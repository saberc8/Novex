# Agent Guardian Model Review Design

## Goal

Move Novex Guardian from a static approval-review contract to a real model-backed reviewer path. This is the next adapter port of Codex `codex-rs/core/src/guardian`: reconstruct a compact transcript, ask a dedicated Guardian reviewer model route for strict JSON, parse explicit allow/deny/needs-human output, and fail closed on timeout, provider failure, or malformed output.

## Current Gap

`crates/novex-approval-review` now owns risk/policy/user-authorization vocabulary, fail-closed policy decisions, and denial breaker behavior. Backend approval pauses include `guardianReview` evidence, and eval tags it.

The remaining gap is runtime review:

- no dedicated model route purpose for Guardian review,
- no Guardian prompt/request shape,
- no strict model-assessment parser,
- no timeout/failure classification,
- no model reviewer metadata in approval-pause evidence,
- no path for future automatic approval decisions.

## Codex Reference

Codex Guardian has a dedicated reviewer session named `guardian`. It builds a prompt from recent transcript entries plus the exact planned action, asks for structured assessment, and fails closed if the session times out, fails, or returns malformed JSON. It distinguishes:

- risk level,
- user authorization,
- outcome,
- rationale,
- reviewer/session failure reason.

Novex does not need Codex's full subagent session reuse in this slice. It does need the same contracts and failure semantics.

## Options

### Option A: Reuse `ModelRoutePurpose::CodeAgent`

This is the smallest transport change. The reviewer prompt would run through the same configured code-agent route.

Trade-off: easy, but weak control-plane semantics. Ops, rollout, cost, and eval cannot distinguish actual agent generation from approval review.

### Option B: Add `ModelRoutePurpose::GuardianReview` and default it to the existing LLM route

Selected. The model control plane gets a distinct `guardian_review` purpose while the POC still works with existing LLM configuration. Operators can later bind a smaller/stricter reviewer model without changing AgentService.

### Option C: Add a full Codex-style reusable review session manager

This is closest to Codex, but too broad for this slice because Novex does not yet have a generic subagent session runtime. The slice should expose the transport and prompt boundary first.

## Architecture

`novex-model` adds `ModelRoutePurpose::GuardianReview`. It maps to `ModelRuntimeTarget::Llm`, parses `guardian_review`, and is included in default LLM runtime purposes. Existing env-based LLM config can therefore serve Guardian review immediately.

`novex-approval-review` adds:

- `GuardianTranscriptEntry`
- `GuardianReviewedAction`
- `GuardianModelReviewRequest`
- `GuardianPromptMessage`
- `GuardianModelAssessment`
- `GuardianReviewStatus`
- `build_guardian_model_review_prompt`
- `parse_guardian_model_assessment`
- `review_tool_approval_with_model_assessment`
- `guardian_review_failure_decision`

`backend` adapts Agent runtime state into transcript entries, builds a reviewed action from the tool and proposed arguments, calls `chat_completion_for_purpose(ModelRoutePurpose::GuardianReview, ...)` with a 90-second timeout, and serializes the resulting decision into `guardianReview`.

## Runtime Semantics

The reviewer is attempted only when `auto_approve = true` and an existing tool policy would otherwise require approval. This keeps Novex non-permissive by default.

Decision behavior:

- model `approved` means the action can be automatically allowed unless the local policy is `always`,
- model `rejected` means the action is not executable and should not be silently retried,
- model `needs_human` means pause for user approval,
- timeout/provider failure/parse failure means `needs_human` with failure metadata.

This gives Novex an actual Guardian reviewer path without forcing every existing approval pause to spend model tokens.

## Trace And Eval

Approval payloads already preserve `guardianReview`. This slice enriches the payload with:

- `reviewStatus`
- `failureReason`
- `reviewerName`
- `modelRouteId`
- `modelProvider`
- `modelName`
- `reviewLatencyMs`

Eval should add:

- `guardianReviewStatus`
- `guardianReviewFailureReason`
- `guardianReviewModelRouteId`

These tags let rollout gates separate static policy pauses from real Guardian reviewer failures.

## Non-Goals

- No persistent reusable subagent session manager.
- No UI changes.
- No new database table.
- No Codex network-access request subtype.
- No cross-turn Guardian denial breaker persistence beyond the crate contract already in place.

## Acceptance

- `novex-model` tests prove `guardian_review` parses, maps to LLM, and is included in default LLM purposes.
- `novex-approval-review` tests prove prompt shape, strict JSON parsing, model assessment decisions, and failure decisions.
- Backend tests prove approval review uses `ModelRoutePurpose::GuardianReview`, includes timeout, builds transcript/action payload, and preserves model reviewer metadata.
- Eval tests prove Guardian model review status/failure/route tags are extracted from traces.
- Migration matrix records Guardian model reviewer progress and leaves reusable review-session manager as next.
- Feature branch and merged `main` both pass `cargo fmt -- --check` and `cargo test --workspace --offline`.
