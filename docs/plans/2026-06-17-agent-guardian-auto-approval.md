# Agent Guardian Auto Approval Implementation Plan

**Goal:** Let Codex-style Guardian model approvals continue Novex agent execution without human pause while keeping timeout/session/parse/rejection outcomes fail-closed.

**Architecture:** `backend` treats `GuardianReviewDecision` as an explicit execution gate after deterministic tool policy requires approval. Approved model-reviewed decisions continue through normal tool execution with traceable `guardianReview` evidence. Non-executable decisions reuse the precomputed review payload for the existing approval pause. `novex-eval` reads Guardian evidence from both `ApprovalRequested` and `ActionSelected`.

## Task 1: Backend Auto-Approval Gate Tests

Files:

- Modify: `backend/src/application/ai/agent_service.rs`

Add tests:

- reviewed Guardian `approved` decision allows execution,
- policy-only or failed-closed decisions do not allow execution,
- source guard confirms deterministic required-approval branch calls the gate before `pause_for_approval`,
- source guard confirms model-loop batch required-approval branch calls the gate before batch execution and passes a precomputed payload to `pause_for_approval`.

Run:

```bash
cargo test -p backend guardian_auto_approval --offline
```

Expected: FAIL until the gate and branch wiring exist.

## Task 2: Backend Continuation Wiring

Files:

- Modify: `backend/src/application/ai/agent_service.rs`

Implement:

- `guardian_auto_approval_allows_execution(decision: &GuardianReviewDecision) -> bool`
- `guardian_review_payload_from_decision(decision: &GuardianReviewDecision) -> Value`
- `pause_for_approval(..., guardian_review_override: Option<Value>, ...)`
- deterministic continuation when Guardian approved:
  - append `ActionSelected` with `guardianAutoApproved`, `approvalMode`, and `guardianReview`,
  - call existing `execute_tool_and_finish`.
- model-loop batch continuation when Guardian approved:
  - keep approval checks before `execute_agent_tool_io_batch`,
  - store Guardian payload by `call_id`,
  - include payload and auto-approval metadata in the normal action event.

Run:

```bash
cargo test -p backend guardian_auto_approval --offline
cargo test -p backend guardian_model_review --offline
cargo test -p backend guardian_review --offline
```

Expected: PASS.

## Task 3: Eval Reads Auto-Approved Action Evidence

Files:

- Modify: `crates/novex-eval/src/lib.rs`

Add test:

- an `ActionSelected` event with `guardianReview` and `guardianAutoApproved = true` produces:
  - `guardianAutoApproved = true`
  - existing Guardian review tags.

Implement:

- extend `TraceGuardianReviewSummary` with `auto_approved`,
- make `trace_guardian_review_summary` inspect the first `ApprovalRequested` or `ActionSelected` event containing `guardianReview`.

Run:

```bash
cargo test -p novex-eval guardian_auto_approval --offline
cargo test -p novex-eval guardian_model_review --offline
cargo test -p novex-eval guardian_review --offline
```

Expected: PASS.

## Task 4: Migration Matrix And Verification

Files:

- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: this plan with actual verification notes if needed.

Update:

- Guardian row to `slice-22 implemented`.
- Guardian approval review capability row to include auto-continue-on-approval evidence.
- Keep remaining gaps explicit: reusable Codex-style Guardian review session manager, delta transcript cursor, persistent denial breaker integration.

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
```

Then merge feature into `main` and rerun both commands on `main`.
