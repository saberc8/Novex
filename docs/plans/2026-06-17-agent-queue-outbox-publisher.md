# Agent Queue Outbox Publisher Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a durable Agent queue outbox and publisher loop so queued Agent runs reliably emit RabbitMQ wake-up messages.

**Progress 2026-06-17:** Implemented durable Agent queue outbox migration, repository transaction APIs, outbox-to-message mapping, publish outcome handling, pending outbox publisher loop, queued create/resume service wiring, `AGENT_QUEUE_PUBLISHER_ENABLED`, startup wiring, focused verification, and full feature-branch verification.

**Architecture:** Mirror the parser outbox pattern while using Agent queue identity as the broker consumer's concurrency gate. Queue creation and queued resume write queue state and outbox state in one repository transaction; a config-gated publisher loop emits `AgentQueueMessage` and records publish state.

**Tech Stack:** Rust, SQLx/Postgres, lapin/RabbitMQ, existing Agent queue runtime tests.

---

### Task 1: Durable Agent Queue Outbox Contract

Status: Completed.

**Files:**
- Create: `backend/migrations/202606170007_create_ai_agent_queue_outbox.sql`
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`

**Step 1: Write failing tests**

Add tests proving:

- migration creates `ai_agent_queue_outbox`,
- migration has `queue_id`, `tenant_id`, `run_id`, `event_type`, `max_attempts`, `payload`, `status`, `attempt_count`, `published_time`,
- repository exposes `AgentQueueOutboxSaveRecord` and `AgentQueueOutboxRecord`,
- repository exposes queue/outbox transactional write APIs,
- repository exposes list/mark published/mark failed APIs.

Run:

```bash
cargo test -p backend agent_queue_outbox --offline
```

Expected: FAIL until migration and repository APIs exist.

### Task 2: Outbox Publisher Runtime Contract

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_queue_runtime.rs`

**Step 1: Write failing tests**

Add tests proving:

- `agent_queue_message_from_outbox` builds `AgentQueueMessage`,
- `publish_agent_queue_outbox_records` returns per-record success/failure outcomes,
- `publish_pending_agent_queue_outbox` lists pending records and marks published/failed,
- `spawn_agent_queue_outbox_publisher` and `run_agent_queue_outbox_publisher` exist.

Run:

```bash
cargo test -p backend agent_queue_outbox --offline
```

Expected: FAIL until runtime publisher APIs exist.

### Task 3: Service And Config Wiring

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `backend/src/shared/config.rs`
- Modify: `backend/src/main.rs`
- Modify: `backend/.env.example`
- Modify: `infra/.env.poc.example`

**Step 1: Write failing tests**

Add tests proving:

- queued run creation writes queue outbox,
- queued resume writes queue outbox,
- `AppConfig` exposes `AGENT_QUEUE_PUBLISHER_ENABLED`,
- main starts `spawn_agent_queue_outbox_publisher`,
- local env/docs expose the publisher flag.

Run:

```bash
cargo test -p backend agent_queue_outbox --offline
cargo test -p backend shared::config --offline
cargo test -p backend foundation --offline
```

Expected: FAIL until service/config/main/env are wired.

### Task 4: Implement Minimal Outbox

Status: Completed.

**Files:**
- Create: `backend/migrations/202606170007_create_ai_agent_queue_outbox.sql`
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`
- Modify: `backend/src/application/ai/agent_queue_runtime.rs`
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `backend/src/shared/config.rs`
- Modify: `backend/src/main.rs`
- Modify: `backend/.env.example`
- Modify: `infra/.env.poc.example`

**Step 1: Add migration and repository records**

Add:

- `AgentQueueOutboxSaveRecord`,
- `AgentQueueOutboxRecord`,
- `enqueue_agent_run_with_outbox`,
- `requeue_agent_run_for_resume_with_outbox`,
- `list_pending_agent_queue_outbox`,
- `mark_agent_queue_outbox_published`,
- `mark_agent_queue_outbox_publish_failed`.

**Step 2: Add runtime publisher**

Add:

- `AgentQueueOutboxPublishOutcome`,
- `agent_queue_message_from_outbox`,
- `publish_agent_queue_outbox_records`,
- `publish_pending_agent_queue_outbox`,
- `spawn_agent_queue_outbox_publisher`,
- `run_agent_queue_outbox_publisher`.

**Step 3: Wire service/config/main**

Use queue/outbox transactional APIs in queued creation and queued resume. Add `AGENT_QUEUE_PUBLISHER_ENABLED`, env examples, compose env, and startup wiring.

**Step 4: Verify focused**

```bash
cargo fmt -- --check
cargo test -p backend agent_queue_outbox --offline
cargo test -p backend agent_queue_runtime --offline
cargo test -p backend ai_agent_repository --offline
cargo test -p backend shared::config --offline
cargo test -p backend foundation --offline
```

### Task 5: Docs, Full Verification, Merge, Clean

Status: Completed.

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-background-run-queue.md`
- Modify: `docs/plans/2026-06-17-agent-queue-outbox-publisher.md`

**Step 1: Update docs**

Record Agent queue outbox publishing as implemented and keep cross-process provider abort plus live POC run as remaining work.

**Step 2: Verify feature branch**

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd apps/agent-workspace && pnpm test -- src/api/agent.test.ts
cd apps/codex-app-poc && pnpm test -- src/api/agent.test.ts
```

**Step 3: Merge and verify main**

Merge `feat/enterprise-agent-foundation` into `main`, rerun the same verification commands on `main`, then run `cargo clean` in main and feature worktrees.
