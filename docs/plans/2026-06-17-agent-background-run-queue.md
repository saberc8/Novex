# Agent Background Run Queue Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add durable Agent run queueing so HTTP can create queued Agent runs and a background worker can claim and execute them using the same Agent runtime code.

**Progress 2026-06-17:** Implemented the durable queue table/repository, `executionMode=queued` API contract, queued Run Graph creation, replayable queued events, config-gated embedded worker, Postgres `FOR UPDATE SKIP LOCKED` claim/lease polling, shared HTTP/worker `AgentRuntimeRegistry`, deterministic existing-run execution, model-loop existing-run execution, pending/retrying queue-row cancellation sync, queued approval-resume requeue, Agent RabbitMQ wake-up topology/message publisher contract, and Agent broker execute consumer with exact queue/tenant/run claim plus retry/dead routing.

**Architecture:** A Postgres-backed `ai_agent_run_queue` owns durable queue state and leases. `AgentService` creates queued runs, exposes an existing-run execution entrypoint, terminalizes not-yet-claimed queue rows when a queued run is cancelled, and requeues queued approval resumes instead of executing tools in the HTTP request. `agent_queue_runtime.rs` polls/claims queue rows and can consume Agent RabbitMQ execute messages by exact queue/tenant/run identity; SSE over `ai_run_event` remains the client progress API.

**Tech Stack:** Rust, Axum service layer, SQLx/Postgres, existing Run Graph tables, existing Agent runtime/model/tool crates.

---

### Task 1: Queue Migration And Repository Contract

Status: Completed in `feat: add agent run queue repository`.

**Files:**
- Create: `backend/migrations/202606170006_create_ai_agent_run_queue.sql`
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`

**Step 1: Write failing tests**

Add backend tests proving:

- migration contains `CREATE TABLE IF NOT EXISTS ai_agent_run_queue`,
- migration contains unique `(tenant_id, run_id)`,
- repository has `AgentRunQueueSaveRecord`, `AgentRunQueueClaimRecord`,
- claim query uses `FOR UPDATE SKIP LOCKED`,
- queue statuses include `pending`, `running`, `retrying`, `succeeded`, `failed`, `cancelled`.

Run:

```bash
cargo test -p backend-rust agent_run_queue --offline
```

Expected: FAIL until migration and repository methods exist.

**Step 2: Implement minimal repository**

Add:

- queue status constants,
- save/claim record structs,
- `enqueue_agent_run`,
- `claim_agent_run_queue`,
- `mark_agent_run_queue_succeeded`,
- `mark_agent_run_queue_retrying`,
- `mark_agent_run_queue_failed`,
- `mark_agent_run_queue_cancelled`.

**Step 3: Verify and commit**

```bash
cargo test -p backend-rust agent_run_queue --offline
git add backend/migrations/202606170006_create_ai_agent_run_queue.sql backend/src/infrastructure/persistence/ai_agent_repository.rs
git commit -m "feat: add agent run queue repository"
```

### Task 2: Command Execution Mode And Queued Run Creation

Status: Completed in `feat: create queued agent runs`.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `apps/agent-workspace/src/types/agent.ts`
- Modify: `apps/codex-app-poc/src/types/agent.ts`

**Step 1: Write failing tests**

Add backend tests proving:

- `AgentRunCommand` accepts `executionMode = queued`,
- blank/unknown execution mode is rejected or normalized,
- queued source path creates `RunStatus::Queued`,
- queued path calls `enqueue_agent_run`,
- inline path remains default.

Run:

```bash
cargo test -p backend-rust agent_run_queue --offline
```

Expected: FAIL until command and service wiring exist.

**Step 2: Implement queued creation**

Add:

- `execution_mode: Option<String>` to `AgentRunCommand`,
- `normalize_agent_execution_mode`,
- `create_queued_run`,
- `create_run_records_with_status`,
- queued `InputReceived` and `StatusChanged` events,
- normalized command payload in queue row.

Default `create_run` behavior remains inline.

**Step 3: Verify and commit**

```bash
cargo test -p backend-rust agent_run_queue --offline
cargo test -p backend-rust agent_runtime_low_risk_tool_can_finish_without_approval --offline
git add backend/src/application/ai/agent_service.rs apps/agent-workspace/src/types/agent.ts apps/codex-app-poc/src/types/agent.ts
git commit -m "feat: create queued agent runs"
```

### Task 3: Existing-Run Execution Entry Point And Worker Runtime

Status: Completed in `feat: execute queued agent runs` for deterministic Agent runs, then extended by `feat: execute queued model loop runs` so queued model-loop uses the same existing-run execution shape without creating a second run.

**Files:**
- Create: `backend/src/application/ai/agent_queue_runtime.rs`
- Modify: `backend/src/application/ai/mod.rs`
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `backend/src/shared/config.rs`
- Modify: `backend/src/main.rs`

**Step 1: Write failing tests**

Add tests proving:

- worker config reads env defaults,
- worker tick claims queue rows,
- worker calls `execute_queued_run` and not `create_run`,
- `execute_queued_run` marks run `running` before execution,
- queue success/retry/failure methods are used.

Run:

```bash
cargo test -p backend-rust agent_queue_runtime --offline
cargo test -p backend-rust agent_run_queue --offline
```

Expected: FAIL until runtime exists.

**Step 2: Implement runtime**

Add:

- `AgentQueueRuntimeConfig`,
- `agent_queue_from_config`,
- `spawn_agent_queue_worker`,
- `run_agent_queue_worker`,
- `run_agent_queue_tick`,
- `execute_queued_run` service method.

Refactor existing inline execution enough that the worker executes an existing run id and does not create a second run.

**Step 3: Verify and commit**

```bash
cargo test -p backend-rust agent_queue_runtime --offline
cargo test -p backend-rust agent_run_queue --offline
git add backend/src/application/ai/agent_queue_runtime.rs backend/src/application/ai/mod.rs backend/src/application/ai/agent_service.rs backend/src/shared/config.rs backend/src/main.rs
git commit -m "feat: execute queued agent runs"
```

### Task 4: Matrix And Final Verification

Status: In progress.

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-background-run-queue.md`

**Step 1: Update docs**

Record:

- Runtime loop slice moves to background queue implemented.
- Current evidence includes queue migration, repository, service, runtime worker, and config.
- Current evidence includes pending/retrying queue-row cancellation sync.
- Current evidence includes waiting-approval queue rows and queued approval-resume requeue.
- Current evidence includes Agent RabbitMQ wake-up topology/message publisher contract.
- Current evidence includes live Agent RabbitMQ execute consumer, exact message-row claim, retry routing, and dead-letter routing.
- Remaining gaps: queue outbox integration and active cross-process provider abort.

**Step 2: Verify**

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd apps/agent-workspace && pnpm test -- src/api/agent.test.ts
cd apps/codex-app-poc && pnpm test -- src/api/agent.test.ts
```

**Step 3: Commit and merge**

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-background-run-queue.md
git commit -m "docs: record agent queue progress"
```

Merge `feat/enterprise-agent-foundation` into `main`, then rerun the same verification on `main`.
