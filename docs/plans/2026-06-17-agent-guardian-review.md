# Agent Guardian Review Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add the Codex Guardian approval-review foundation to Novex: reusable review crate, denial circuit breaker, backend approval-pause evidence, trace payload preservation, and eval tags.

**Architecture:** `novex-approval-review` owns Guardian review vocabulary and state-machine behavior. `backend-rust` adapts existing tool risk/policy records into Guardian review payloads when approval pauses are emitted. `novex-eval` extracts Guardian review tags from trace bundles.

**Tech Stack:** Rust, serde, serde_json, existing backend AgentService, Cargo offline tests.

---

### Task 1: Guardian Review Crate Contract

**Files:**
- Add: `crates/novex-approval-review/Cargo.toml`
- Add: `crates/novex-approval-review/src/lib.rs`
- Modify: `Cargo.toml`

**Step 1: Write failing tests**

Create the crate skeleton and tests for:

- high-risk tools require human approval even when auto-approved,
- medium-risk tools can be approved when auto-approved,
- `approval_policy = always` requires human approval,
- denial circuit breaker interrupts after three consecutive denials,
- denial circuit breaker interrupts after ten recent denials in a fifty-review window,
- non-denials reset the consecutive-denial counter.

**Step 2: Verify RED**

Run:

```bash
cargo test -p novex-approval-review guardian --offline
```

Expected: FAIL until the contract types and functions are implemented.

**Step 3: Implement minimal contract**

Add:

- `GuardianRiskLevel`
- `GuardianApprovalPolicy`
- `GuardianUserAuthorization`
- `GuardianReviewOutcome`
- `GuardianDecisionSource`
- `GuardianReviewInput`
- `GuardianReviewDecision`
- `review_tool_approval`
- `GuardianRejectionCircuitBreaker`

Keep behavior fail-closed and non-permissive relative to the existing backend policy.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p novex-approval-review guardian --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add Cargo.toml crates/novex-approval-review
git commit -m "feat: add guardian approval review contract"
```

### Task 2: Backend Approval Evidence And Trace Preservation

**Files:**
- Modify: `backend/Cargo.toml`
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add tests for:

- high-risk auto-approved tools still require approval and expose a Guardian decision requiring human approval,
- approval-pause source contains `guardianReview` payload wiring,
- `approval_requested` trace conversion preserves the full original payload.

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend-rust guardian_review --offline
```

Expected: FAIL until backend maps Guardian review decisions and preserves approval payloads in traces.

**Step 3: Implement backend adapter**

- Add `novex-approval-review` dependency.
- Map backend `ToolLookupRecord` risk and approval policy into Guardian enums.
- Add `guardian_review_for_tool_policy(...)`.
- Add serialized `guardianReview` to approval `ActionSelected` and `ApprovalRequested` event payloads.
- Preserve full `approval_requested` payload when converting run events to trace events.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p backend-rust guardian_review --offline
cargo test -p backend-rust agent_tool_policy_requires_manual_approval_for_high_risk_even_when_auto_approved --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/Cargo.toml backend/src/application/ai/agent_service.rs
git commit -m "feat: attach guardian review to approval pauses"
```

### Task 3: Eval Tags And Migration Matrix

**Files:**
- Modify: `crates/novex-eval/src/lib.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Write failing tests**

Add an eval test proving an approval-requested trace with `guardianReview` yields:

- `guardianReviewOutcome`
- `guardianReviewSource`
- `guardianReviewRequiresHumanApproval`

**Step 2: Verify RED**

Run:

```bash
cargo test -p novex-eval guardian_review --offline
```

Expected: FAIL until eval extracts Guardian review tags.

**Step 3: Implement eval extraction and docs update**

- Add a small helper that reads the first `ApprovalRequested` event with `guardianReview`.
- Insert low-cardinality Guardian tags into eval candidate tags.
- Update Guardian row and acceptance evidence in the migration matrix.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p novex-eval guardian_review --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-eval/src/lib.rs docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "feat: tag guardian review evidence"
```

### Task 4: Final Verification And Merge

**Files:**
- All modified files

**Step 1: Verify feature branch**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
```

Expected: PASS.

**Step 2: Merge to main**

Fast-forward or merge `feat/enterprise-agent-foundation` into `main`.

**Step 3: Verify main**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
```

Expected: PASS.

**Step 4: Report**

Summarize implemented Guardian foundation, accepted remaining gaps, exact verification commands, and current commit.
