# Agent Guardian Auto Approval Design

## Goal

Close the Codex Guardian execution gap: when the configured Guardian model route reviews an otherwise approval-gated tool call and returns an explicit executable approval, Novex should continue the agent turn without a human pause.

This keeps the enterprise posture fail-closed. Timeout, provider/session failure, parse failure, `needs_human`, `rejected`, policy-only decisions, and `approval_policy = always` must still pause for human approval.

## Current State

Novex already has:

- `novex-approval-review` with Guardian risk, policy, authorization, model assessment, strict parser, fail-closed failure decisions, and denial breaker vocabulary.
- `ModelRoutePurpose::GuardianReview`, routed through the configured LLM model route.
- Backend approval pauses that call `guardian_review` when `autoApprove = true` and attach serialized `guardianReview` payloads.
- Trace and eval tags for Guardian review outcome, source, status, failure reason, and model route.

The remaining gap is that an approved Guardian model decision is only evidence. The backend still calls `pause_for_approval` and sets `waiting_approval`.

## Selected Design

Add a narrow backend execution gate:

```rust
fn guardian_auto_approval_allows_execution(decision: &GuardianReviewDecision) -> bool
```

It returns true only when:

- `source = guardian`
- `review_status = reviewed`
- `can_execute = true`

This makes the executable condition explicit and auditable. Static policy-only approvals do not silently bypass a pause. Failed-closed model decisions remain non-executable because `can_execute = false`.

## Backend Flow

For deterministic runs:

1. Existing tool policy says approval is required.
2. Backend obtains the Guardian decision using the same `guardian_review_decision_for_tool_policy` path.
3. If the decision is executable, append `ActionSelected` with:
   - `guardianReview`
   - `guardianAutoApproved = true`
   - `approvalMode = "guardian_auto_approved"`
4. Execute the tool and finish as the existing non-paused path does.
5. Otherwise call `pause_for_approval` with the precomputed review payload so the model is not called twice.

For model-loop batch runs:

1. Existing per-call policy still runs before any batch execution.
2. If a call requires approval, backend obtains the Guardian decision.
3. If executable, store the review payload on the prepared call metadata and let the normal action/event/execution path continue.
4. If not executable, emit the existing `ActionSelected`, status update, approval pause, and trace refresh with the precomputed payload.
5. Batch execution still happens only after every call has passed policy or Guardian executable review.

## Trace And Eval

Auto-approved runs do not emit `ApprovalRequested`, so eval must also read Guardian evidence from `ActionSelected` events. It should tag:

- `guardianAutoApproved = true` when present on the action event.
- existing Guardian outcome/source/status/failure/route tags from the same `guardianReview` object.

This lets rollout gates measure model-granted auto-approvals separately from human pauses and reviewer failures.

## Rejected Alternatives

### Trust Any `can_execute`

Rejected. A policy-only decision could be executable for low-risk tools, but this slice is about model-backed Guardian auto review. The gate must require `source = guardian` and `review_status = reviewed`.

### Re-run Guardian During Pause

Rejected. A non-executable result should be recorded exactly once. Re-running the reviewer when constructing the approval pause can create inconsistent evidence and wastes model budget.

### Change Tool Policy Itself

Rejected for now. `novex-tools` should remain the deterministic policy layer. Guardian is an adapter/reviewer that can override a required pause only under the explicit executable review contract.

## Acceptance

- Backend tests prove executable reviewed Guardian decisions can continue and failed/policy decisions cannot.
- Backend source guards prove deterministic and model-loop approval branches use the auto-approval gate before pausing and pass precomputed payloads into `pause_for_approval`.
- Eval tests prove `ActionSelected` Guardian payloads are tagged, including `guardianAutoApproved`.
- Migration matrix marks Guardian slice 22 implemented and names remaining gaps.
- `cargo fmt -- --check && cargo test --workspace --offline` passes on feature and main after merge.
