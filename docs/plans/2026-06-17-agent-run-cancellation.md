# Agent Run Cancellation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add DB-backed external cancellation checkpoints to the model-loop runtime so `POST /cancel` can stop future model/tool work and produce a stable cancelled trace contract.

**Architecture:** Keep the current synchronous request flow. Add small private helpers in `AgentService` that inspect persisted run status at safe runtime boundaries and finalize/preserve cancellation with a structured payload. Future in-memory task tokens can reuse the same cancellation payload and event semantics.

**Tech Stack:** Rust, Tokio async service methods, SQLx repository reads, serde_json payloads, Cargo offline tests.

---

### Task 1: Add Cancellation Checkpoint Contract

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add tests near the model-loop tests:

```rust
#[test]
fn agent_service_model_loop_checks_external_cancel_before_model_call() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("before_model_call"));
    assert!(source.contains("check_model_loop_cancelled"));
}

#[test]
fn agent_service_model_loop_records_external_cancel_reason() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("\"cancelReason\""));
    assert!(source.contains("external_cancel"));
    assert!(source.contains("\"cancelStage\""));
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend-rust external_cancel --offline
```

Expected: FAIL because `external_cancel` checkpoint helpers are not implemented.

**Step 3: Implement minimal helpers**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelLoopCancelCheck {
    Continue,
    Cancelled,
}
```

Add private helper methods:

```rust
async fn check_model_loop_cancelled(
    &self,
    user_id: i64,
    run_id: i64,
    stage: &str,
) -> Result<ModelLoopCancelCheck, AppError>

async fn finish_model_loop_cancelled(
    &self,
    user_id: i64,
    run_id: i64,
    stage: &str,
) -> Result<(), AppError>
```

`check_model_loop_cancelled` reads `repo.find_run`. If status is `cancelling` or `cancelled`, call `finish_model_loop_cancelled` and return `Cancelled`; otherwise return `Continue`.

`finish_model_loop_cancelled` updates status to cancelled only when not already cancelled, appends a `Cancelled` event, and refreshes trace snapshot with:

```json
{
  "cancelled": true,
  "cancelReason": "external_cancel",
  "cancelStage": "<stage>",
  "runtimeMode": "model_loop"
}
```

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p backend-rust external_cancel --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: add agent run cancellation checkpoints"
```

### Task 2: Wire Checkpoints Into Model Loop

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing test**

Add a source-level guard:

```rust
#[test]
fn agent_service_model_loop_checks_cancel_around_tool_batches() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("before_tool_batch"));
    assert!(source.contains("after_tool_batch"));
    assert!(source.contains("before_next_turn"));
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend-rust external_cancel --offline
```

Expected: FAIL until the checkpoint calls are present.

**Step 3: Add checkpoint calls**

Inside `create_model_loop_run`:

- At the top of every turn, call `check_model_loop_cancelled(..., "before_model_call")`.
- After model response parsing and before final/tool dispatch, call `check_model_loop_cancelled(..., "after_model_call")`.
- Before `execute_agent_tool_io_batch`, call `check_model_loop_cancelled(..., "before_tool_batch")`.
- After tool execution returns and before observations are recorded for the next turn, call `check_model_loop_cancelled(..., "after_tool_batch")`.
- Before pushing the follow-up prompt and continuing, call `check_model_loop_cancelled(..., "before_next_turn")`.

If any checkpoint returns `Cancelled`, return `self.get_run(run_id).await`.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p backend-rust external_cancel --offline
cargo test -p backend-rust model_loop --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: stop model loop on external cancellation"
```

### Task 3: Update Migration Matrix And Verify

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update docs**

Change Parallel tools / Runtime loop notes to mention DB-backed external cancellation checkpoints are implemented, while in-memory task registry and provider abort remain next.

**Step 2: Verify**

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust external_cancel --offline
cargo test -p backend-rust model_loop --offline
cargo test -p backend-rust agent_service --offline
cargo test --workspace --offline
```

Expected: all pass; `live_rag_e2e` may remain ignored unless POC infra is configured.

**Step 3: Commit**

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: record agent cancellation checkpoint progress"
```

