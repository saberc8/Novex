# Agent Queue Cancel Sync Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Synchronize queued Agent run cancellation with the durable queue so `pending` and `retrying` queue rows become terminal `cancelled` before a worker claims them.

**Progress 2026-06-17:** Implemented `cancel_agent_run_queue_for_run`, wired `AgentService::cancel_run` to call it, and added source-contract tests covering the queue SQL and service orchestration.

**Architecture:** Keep worker ownership of `running` queue rows. Add a narrow repository method that terminalizes not-yet-running queue rows by `tenant_id + run_id`, and call it from `AgentService::cancel_run` after the cancel request event is written.

**Tech Stack:** Rust, SQLx/Postgres, existing Agent service cancellation flow, existing `ai_agent_run_queue`.

---

### Task 1: Source Contract And Red Tests

Status: Completed.

**Files:**
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add backend tests proving:

- repository exposes `cancel_agent_run_queue_for_run`,
- the SQL updates `queue_status = $3`,
- the SQL scopes to `queue_status IN ('pending', 'retrying')`,
- the SQL sets `AGENT_RUN_QUEUE_STATUS_CANCELLED`,
- the SQL clears `locked_by` and `locked_until`,
- the SQL writes `finished_at`,
- `cancel_run` calls `cancel_agent_run_queue_for_run`.

Run:

```bash
cargo test -p backend-rust agent_queue_cancel_sync --offline
```

Expected: FAIL until the repository method and service wiring exist.

### Task 2: Implement Queue Cancel Repository Method

Status: Completed.

**Files:**
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`

**Step 1: Add method**

Add:

```rust
pub async fn cancel_agent_run_queue_for_run(
    &self,
    tenant_id: i64,
    run_id: i64,
    user_id: i64,
    now: NaiveDateTime,
) -> Result<u64, AppError>
```

The method updates only `pending` and `retrying` rows, clears locks, sets `finished_at`, preserves existing `last_error`, and returns `rows_affected`.

**Step 2: Verify**

```bash
cargo test -p backend-rust agent_queue_cancel_sync --offline
cargo test -p backend-rust agent_run_queue --offline
```

### Task 3: Wire Cancel API To Queue Sync

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Call repository method**

Inside `cancel_run`, after `CancelRequested` is appended and active pauses are cancelled, call:

```rust
self.repo
    .cancel_agent_run_queue_for_run(self.tenant_id, run_id, user_id, now)
    .await?;
```

Do not require affected rows to be non-zero.

**Step 2: Verify**

```bash
cargo test -p backend-rust agent_queue_cancel_sync --offline
cargo test -p backend-rust external_cancel --offline
cargo test -p backend-rust runtime_supervisor --offline
```

### Task 4: Update Docs, Verify, Merge

Status: Completed.

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-background-run-queue.md`
- Modify: `docs/plans/2026-06-17-agent-queue-cancel-sync.md`

**Step 1: Update progress docs**

Record that queued runs now synchronize cancellation for not-yet-claimed queue rows. Keep cross-process active provider abort and broker-backed wake-up listed as remaining gaps.

**Step 2: Verify feature branch**

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd apps/agent-workspace && pnpm test -- src/api/agent.test.ts
cd apps/codex-app-poc && pnpm test -- src/api/agent.test.ts
```

**Step 3: Merge and verify main**

Merge `feat/enterprise-agent-foundation` into `main`, then rerun the same verification commands on `main`.
