# Agent Guardian Review Design

## Goal

Close the next Codex approval-safety gap by adding a Novex Guardian review contract for tool approvals. This is an adapter port of Codex `codex-rs/core/src/guardian`: Novex should have a reusable approval review crate, Codex-style denial circuit breaker semantics, backend approval-pause evidence, and eval tags for rollout gates.

## Codex Reference

Codex Guardian protects automatic approval by:

- reconstructing the recent conversation and proposed command/tool action,
- asking a dedicated Guardian reviewer session for a structured assessment,
- failing closed when the review times out, fails, or returns malformed output,
- applying explicit allow/deny decisions, and
- interrupting a turn after repeated Guardian denials.

The reusable part Novex should port first is not provider transport. It is the stable review vocabulary and safety state machine: risk level, user authorization, review outcome, rationale, human-approval requirement, and repeated-denial breaker.

## Current Novex Gap

Novex already has `novex-tools::evaluate_tool_execution_policy`, and backend pauses high-risk or non-auto-approved risky tools before execution. The gaps are:

- no `crates/novex-approval-review` target crate even though the migration matrix names it,
- no structured Guardian outcome attached to approval pauses,
- no Codex-style denial circuit breaker,
- trace conversion drops approval payload details and keeps only `toolCode`,
- eval cannot tag approval-review evidence.

## Options

### Option A: Full model-based Guardian reviewer now

Add a new model route purpose, transcript reconstruction, timeout handling, JSON parsing, and reviewer prompts immediately.

Trade-off: closest to Codex runtime behavior, but too wide for this slice because Novex model-loop safety still needs a stable internal review contract and observable acceptance points first.

### Option B: Only annotate existing approval pauses

Add `guardianReview` JSON directly in backend without a crate.

Trade-off: fast, but it keeps the matrix target empty and makes later customer-service, NotebookLM, and enterprise KB write-policy checks reimplement review semantics.

### Option C: Add the reusable review crate and backend adapter

Selected. `novex-approval-review` owns the Guardian review decision vocabulary and denial breaker. Backend uses it when emitting approval-pause events. Trace preserves the approval payload, and eval reads the Guardian tags.

## Review Contract

`crates/novex-approval-review` owns:

- `GuardianRiskLevel`: low, medium, high
- `GuardianApprovalPolicy`: never, on_risk, always
- `GuardianUserAuthorization`: explicit, implicit, missing
- `GuardianReviewOutcome`: approved, needs_human, rejected
- `GuardianDecisionSource`: policy, guardian
- `GuardianReviewInput`
- `GuardianReviewDecision`
- `GuardianRejectionCircuitBreaker`

Initial adapter behavior is fail-closed:

- high-risk tools always require human approval,
- `approval_policy = always` always requires human approval,
- medium-risk tools require human approval unless auto-approved or explicitly authorized,
- low-risk tools can be approved by policy when policy permits,
- disabled/missing model reviewer does not reduce the current backend approval requirement.

This means Guardian review evidence can be introduced without making Novex more permissive than the existing policy engine.

## Backend Adapter

`AgentService` continues to use the existing tool policy decision to decide whether a tool can execute. When an approval pause is needed, it adds a serialized `guardianReview` object to:

- the approval `ActionSelected` event,
- the `ApprovalRequested` event.

Trace conversion should preserve the full approval event payload instead of rebuilding a minimal `toolCode` payload. This makes rollout replay and eval carry the same evidence users and operators see in run events.

## Trace And Eval

`novex-eval` should tag the first approval review seen in a trace:

- `guardianReviewOutcome`
- `guardianReviewSource`
- `guardianReviewRequiresHumanApproval`

These tags are intentionally low-cardinality so future eval suites can gate customer service sends, enterprise-KB writes, NotebookLM notebook mutations, and POC tool execution against safety behavior.

## Non-Goals

- No dedicated model route purpose for Guardian yet.
- No model reviewer prompt or timeout worker yet.
- No background approval queue.
- No policy relaxation beyond the current backend safety behavior.
- No UI changes.

## Acceptance

- `novex-approval-review` tests prove fail-closed decisions and Codex-style repeated-denial breaker behavior.
- Backend tests prove approval pauses include `guardianReview`, high-risk auto-approval still pauses, and trace conversion preserves the payload.
- Eval tests prove Guardian tags are extracted from approval-requested trace events.
- Migration matrix marks Guardian as implemented for the review contract while leaving model reviewer sessions as next.
- Feature branch and merged `main` both pass `cargo fmt -- --check` and `cargo test --workspace --offline`.
