# Agent Queued Model Loop Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable `executionMode=queued` for `runtimeMode=model_loop` by extracting the inline model-loop body into an existing-run execution entrypoint shared by HTTP and the Agent queue worker.

**Progress 2026-06-17:** Implemented `execute_model_loop_existing_run`, optional inline input-event recording, queued model-loop creation, and queued execution dispatch through the shared model-loop executor.

**Architecture:** `create_model_loop_run` creates Run Graph records, then calls `execute_model_loop_existing_run`. `execute_queued_run` marks queued runs running and dispatches model-loop commands to the same helper. The worker remains a thin claim/lease executor and never creates a second run.

**Tech Stack:** Rust, Axum service layer, SQLx/Postgres, existing Novex Agent runtime/model/tool crates, existing `ai_agent_run_queue`.

---

### Task 1: Source Contract And Red Tests

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `backend/src/application/ai/agent_queue_runtime.rs`

**Step 1: Write failing tests**

Add backend tests proving:

- queued model-loop is not rejected by `create_queued_run` or `execute_queued_run`,
- `create_model_loop_run` calls `execute_model_loop_existing_run(..., true)`,
- `execute_queued_run` calls `execute_model_loop_existing_run(..., false)`,
- the worker still calls `execute_queued_run` and not `create_run`,
- the helper registers the active run and owns model-loop cancellation.

Run:

```bash
cargo test -p backend-rust queued_model_loop --offline
```

Expected: FAIL until the helper exists and the rejection strings are removed.

### Task 2: Extract Existing-Run Model Loop

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Refactor inline creation**

Change `create_model_loop_run` so it:

- builds the model-loop plan,
- creates Run Graph records,
- calls `execute_model_loop_existing_run(user_id, run_id, command, true)`.

**Step 2: Create shared helper**

Add:

- `execute_model_loop_existing_run`,
- `record_model_loop_input_event`.

Move the current model-loop body into the helper. The helper should:

- fetch retry policy,
- register the active run in `AgentRuntimeRegistry`,
- create `AgentRuntimeState`,
- optionally record running input event,
- run the existing sampling/tool/Guardian/compaction/cancellation loop unchanged.

**Step 3: Verify**

```bash
cargo test -p backend-rust model_loop --offline
cargo test -p backend-rust queued_model_loop --offline
```

### Task 3: Enable Queued Model Loop Dispatch

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-background-run-queue.md`

**Step 1: Remove rejection**

Allow `create_queued_run` to build a model-loop plan by setting:

- `loop_kind = "model_loop"`,
- `selected_tool_code = None`,
- `requires_approval = false`,
- `pause_reason = None`.

**Step 2: Dispatch from queued execution**

In `execute_queued_run`, after the queued run is moved to `running`, dispatch:

```rust
if command.runtime_mode.as_deref() == Some("model_loop") {
    return self
        .execute_model_loop_existing_run(user_id, run_id, command, false)
        .await;
}
```

**Step 3: Update docs**

Record that background queue now supports model-loop existing-run execution. Remaining gaps are broker wake-up transport, cross-process provider abort, resume requeue, and distributed cancellation.

### Task 4: Verify, Commit, Merge

Status: In progress.

**Step 1: Verify feature branch**

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd apps/agent-workspace && pnpm test -- src/api/agent.test.ts
cd apps/codex-app-poc && pnpm test -- src/api/agent.test.ts
```

**Step 2: Commit**

```bash
git add backend/src/application/ai/agent_service.rs backend/src/application/ai/agent_queue_runtime.rs docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-background-run-queue.md
git commit -m "feat: execute queued model loop runs"
```

**Step 3: Merge and verify main**

Merge `feat/enterprise-agent-foundation` into `main`, then rerun the same verification commands on `main`.
