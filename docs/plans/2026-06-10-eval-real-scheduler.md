# Eval Real Scheduler Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make Novex eval runs use durable asynchronous task scheduling and a real worker execution path instead of synchronous simulated results.

**Architecture:** Keep the existing eval HTTP API shape but change run creation to enqueue tasks. Add `ai_eval_task` and `ai_eval_outbox`, publish task messages to a dedicated RabbitMQ topology, and add an `eval-worker` binary that executes `live_rag` through the real knowledge path and aggregates the run.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL migrations, RabbitMQ/lapin, Tokio, serde_json, existing Novex eval and knowledge services.

---

### Task 1: Add Eval Task And Outbox Schema

**Files:**
- Create: `backend/migrations/202606100001_create_ai_eval_task_queue.sql`

**Step 1: Write migration assertions**

Add tests in `backend/src/application/ai/eval_service.rs` that include the migration and assert it creates `ai_eval_task`, `ai_eval_outbox`, `idx_ai_eval_task_status`, and `idx_ai_eval_outbox_status`.

**Step 2: Run failing test**

Run: `cargo test -p backend application::ai::eval_service::tests::eval_task_queue_migration_defines_required_tables`

Expected: FAIL because the migration file does not exist.

**Step 3: Add migration**

Create the migration with `ai_eval_task` and `ai_eval_outbox` tables, status indexes, run/task indexes, and a unique outbox key on `(tenant_id, task_id, event_type)`.

**Step 4: Run passing test**

Run: `cargo test -p backend application::ai::eval_service::tests::eval_task_queue_migration_defines_required_tables`

Expected: PASS.

### Task 2: Extend Eval Repository

**Files:**
- Modify: `backend/src/infrastructure/persistence/ai_eval_repository.rs`
- Test: `backend/src/infrastructure/persistence/ai_eval_repository.rs`

**Step 1: Write repository shape tests**

Add source-level tests that assert repository exposes task/outbox methods:

- `create_task`
- `create_outbox`
- `list_pending_eval_outbox`
- `mark_eval_outbox_published`
- `try_start_task`
- `complete_task`
- `fail_task`
- `list_run_tasks`

**Step 2: Run failing test**

Run: `cargo test -p backend infrastructure::persistence::ai_eval_repository::tests::eval_repository_exposes_task_queue_methods`

Expected: FAIL because methods are missing.

**Step 3: Implement repository records and methods**

Add record structs for task save/read, outbox read/save, and terminal task summaries. Keep SQL query-builder style consistent with existing repository code.

**Step 4: Run passing test**

Run: `cargo test -p backend infrastructure::persistence::ai_eval_repository::tests::eval_repository_exposes_task_queue_methods`

Expected: PASS.

### Task 3: Convert Run Creation To Async Scheduling

**Files:**
- Modify: `backend/src/application/ai/eval_service.rs`
- Test: `backend/src/application/ai/eval_service.rs`

**Step 1: Write scheduling tests**

Add tests for pure helpers:

- `normalize_eval_run_command` still accepts `deterministic` and `live_rag`;
- `eval_task_payload` includes `taskId`, `runId`, `tenantId`, `caseId`, `runMode`, `attempt`, and `maxAttempts`;
- `queued_eval_run_payload` reports `status = queued` and zero completed cases.

**Step 2: Run failing test**

Run: `cargo test -p backend application::ai::eval_service::tests::eval_run_creation_builds_task_outbox_payload`

Expected: FAIL because helper does not exist.

**Step 3: Implement async scheduling path**

Change `run_eval` to create a queued run, task rows, and outbox rows. Move old synchronous case execution into a worker helper so HTTP no longer scores inline.

**Step 4: Run passing test**

Run: `cargo test -p backend application::ai::eval_service::tests::eval_run_creation_builds_task_outbox_payload`

Expected: PASS.

### Task 4: Add Eval Queue Runtime

**Files:**
- Create: `backend/src/application/ai/eval_queue_runtime.rs`
- Modify: `backend/src/application/ai/mod.rs`
- Modify: `backend/src/infrastructure/mq/rabbitmq.rs`
- Modify: `backend/src/shared/config.rs`

**Step 1: Write runtime helper tests**

Add tests for config defaults, message conversion from outbox, and publish outcome handling, following parser queue runtime tests.

**Step 2: Run failing test**

Run: `cargo test -p backend application::ai::eval_queue_runtime::tests`

Expected: FAIL because module is missing.

**Step 3: Implement runtime**

Add `EvalTaskMessage`, `EvalRabbitMqConfig`, publisher trait, `publish_pending_eval_tasks`, and `spawn_eval_queue_publisher`.

**Step 4: Run passing test**

Run: `cargo test -p backend application::ai::eval_queue_runtime::tests`

Expected: PASS.

### Task 5: Add Eval Worker Execution

**Files:**
- Create: `backend/src/application/ai/eval_worker_runtime.rs`
- Create: `backend/src/bin/eval_worker.rs`
- Modify: `backend/src/application/ai/mod.rs`
- Modify: `backend/src/application/ai/eval_service.rs`

**Step 1: Write worker tests**

Add tests that verify:

- terminal task messages are ignored;
- `live_rag` requires `knowledgeDatasetId`;
- a successful worker output converts to `EvalResultSaveRecord`;
- aggregation returns `running` until all tasks are terminal.

**Step 2: Run failing test**

Run: `cargo test -p backend application::ai::eval_worker_runtime::tests`

Expected: FAIL because module is missing.

**Step 3: Implement worker runtime**

Implement task consumption, task leasing, execution dispatch, retry/dead handling, result persistence, and run aggregation. Use an executor trait so tests can verify worker behavior without calling external providers.

**Step 4: Run passing test**

Run: `cargo test -p backend application::ai::eval_worker_runtime::tests`

Expected: PASS.

### Task 6: Wire Startup And Local POC Env

**Files:**
- Modify: `backend/src/main.rs`
- Modify: `backend/src/shared/config.rs`
- Modify: `.env.example`
- Modify: `README.md`

**Step 1: Write config tests**

Add tests asserting eval queue env defaults and RabbitMQ topology names.

**Step 2: Run failing test**

Run: `cargo test -p backend shared::config::tests::eval_queue_config_defaults_are_safe`

Expected: FAIL because fields are missing.

**Step 3: Implement wiring**

Start the eval outbox publisher in backend when enabled. Add `eval-worker` service to compose behind the backend dependencies.

**Step 4: Run passing test**

Run: `cargo test -p backend shared::config::tests::eval_queue_config_defaults_are_safe`

Expected: PASS.

### Task 7: Verify

**Files:**
- All touched Rust and infra files

**Step 1: Format**

Run: `cargo fmt -- --check`

Expected: PASS.

**Step 2: Backend targeted tests**

Run: `cargo test -p backend application::ai::eval_service::tests application::ai::eval_queue_runtime::tests application::ai::eval_worker_runtime::tests`

Expected: PASS.

**Step 3: Backend compile tests**

Run: `cargo test -p backend`

Expected: PASS or report pre-existing environment-gated failures.

**Step 4: Commit**

Commit the completed implementation with:

```bash
git add backend infra docs/plans
git commit -m "feat: add real eval scheduler"
```
