# Agent Queue Resume Requeue Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Route queued Agent approval resume through `ai_agent_run_queue` so HTTP approval unblocks the run and the worker performs the resumed tool execution.

**Architecture:** Add a non-claimable `waiting_approval` queue status, mark queue rows with that status when queued execution pauses, and requeue the existing row to `pending` from `resume_run`. `execute_queued_run` gets a resume-payload branch that runs the approved tool input using a shared helper.

**Tech Stack:** Rust, SQLx/Postgres, existing Agent service, existing embedded Agent queue worker.

---

### Task 1: Source Contract And Red Tests

Status: Planned.

**Files:**
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`
- Modify: `backend/src/application/ai/agent_queue_runtime.rs`
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add backend tests proving:

- repository exposes `AGENT_RUN_QUEUE_STATUS_WAITING_APPROVAL`,
- repository exposes `mark_agent_run_queue_waiting_approval`,
- repository exposes `requeue_agent_run_for_resume`,
- requeue SQL updates `queue_status = $3`, resets `attempt_count = 0`, clears locks, clears `finished_at`, and scopes to `queue_status IN ('waiting_approval', 'succeeded')`,
- worker handles `run.status == "waiting_approval"` with `mark_agent_run_queue_waiting_approval`,
- `resume_run` calls `requeue_agent_run_for_resume` and returns without inline tool execution when requeued,
- `execute_queued_run` detects resume payloads and calls the shared resumed-tool helper.

Run:

```bash
cargo test -p backend-rust agent_queue_resume_requeue --offline
```

Expected: FAIL until the status, repository methods, worker branch, and service branch exist.

### Task 2: Repository Queue Status And Requeue

Status: Planned.

**Files:**
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`

**Step 1: Add waiting-approval status**

Add:

```rust
pub const AGENT_RUN_QUEUE_STATUS_WAITING_APPROVAL: &str = "waiting_approval";
```

**Step 2: Add status marker**

Add `mark_agent_run_queue_waiting_approval(queue_id, user_id, now)` that updates the row to `waiting_approval`, clears `locked_by` and `locked_until`, and updates audit fields without setting terminal `finished_at`.

**Step 3: Add resume requeue**

Add `requeue_agent_run_for_resume(tenant_id, run_id, payload, user_id, now)` that updates the existing queue row to `pending`, resets attempt counters and locks, clears terminal fields, replaces payload, and only matches `waiting_approval` or compatibility `succeeded` rows.

**Step 4: Verify**

```bash
cargo test -p backend-rust agent_queue_resume_requeue --offline
cargo test -p backend-rust agent_run_queue --offline
```

### Task 3: Worker Waiting-Approval Handling

Status: Planned.

**Files:**
- Modify: `backend/src/application/ai/agent_queue_runtime.rs`

**Step 1: Update worker terminalization**

In `run_agent_queue_tick`, add a branch before generic success:

```rust
Ok(run) if run.status == "waiting_approval" => {
    repo.mark_agent_run_queue_waiting_approval(...)
}
```

**Step 2: Verify**

```bash
cargo test -p backend-rust agent_queue_resume_requeue --offline
cargo test -p backend-rust agent_queue_runtime --offline
```

### Task 4: Resume API Requeue And Queued Resume Execution

Status: Planned.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Add resume payload helpers**

Add:

```rust
fn agent_resume_queue_payload(command: &AgentRunResumeCommand) -> Value
fn agent_resume_input_from_queue_payload(payload: &Value) -> Result<Option<Value>, AppError>
```

**Step 2: Extract resumed tool execution**

Move the tool lookup and `execute_tool_and_finish` part of `resume_run` into `execute_resumed_tool_and_finish(user_id, run_id, resume_input)`.

**Step 3: Requeue from resume API**

After pause completion and `Resumed` event, call `requeue_agent_run_for_resume`. If it returns a non-zero row count, append a queued resume status event, refresh trace, and return `get_run` without inline tool execution.

**Step 4: Execute resume payload in worker path**

At the top of `execute_queued_run`, detect resume payloads, transition `resuming -> running` through the existing status code, call `execute_resumed_tool_and_finish`, and return `get_run`.

**Step 5: Verify**

```bash
cargo test -p backend-rust agent_queue_resume_requeue --offline
cargo test -p backend-rust guardian_review --offline
cargo test -p backend-rust guardian_auto_approval --offline
cargo test -p backend-rust agent_queue_runtime --offline
```

### Task 5: Docs, Full Verification, Merge

Status: Planned.

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-background-run-queue.md`
- Modify: `docs/plans/2026-06-17-agent-queue-resume-requeue.md`

**Step 1: Update docs**

Record queued approval resume requeue as implemented. Keep broker-backed wake-up and active cross-process provider abort listed as remaining gaps.

**Step 2: Verify feature branch**

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd apps/agent-workspace && pnpm test -- src/api/agent.test.ts
cd apps/codex-app-poc && pnpm test -- src/api/agent.test.ts
```

**Step 3: Merge and verify main**

Merge `feat/enterprise-agent-foundation` into `main`, then rerun the same verification commands on `main`.

