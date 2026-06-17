# Agent Queue Broker Consumer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an Agent RabbitMQ execute consumer that wakes queued Agent runs while Postgres remains the durable execution queue.

**Architecture:** Reuse scheduler's RabbitMQ consumer style, but claim Agent work through `ai_agent_run_queue` by message identity before execution. Keep the existing polling worker as fallback, and route retry/dead outcomes through the Agent RabbitMQ topology.

**Tech Stack:** Rust, lapin/RabbitMQ, SQLx/Postgres, existing Agent queue runtime tests.

---

### Task 1: Claim Durable Queue Row By Broker Message

Status: Pending.

**Files:**
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`

**Step 1: Write failing tests**

Add tests proving:

- repository exposes `claim_agent_run_queue_by_message`,
- SQL filters by `id`, `tenant_id`, and `run_id`,
- claim uses `FOR UPDATE SKIP LOCKED`,
- claim only accepts `pending` and `retrying` rows.

Run:

```bash
cargo test -p backend-rust agent_queue_broker_consumer --offline
```

Expected: FAIL until the repository claim function exists.

### Task 2: Add Broker Consumer Runtime Contract

Status: Pending.

**Files:**
- Modify: `backend/src/application/ai/agent_queue_runtime.rs`
- Modify: `backend/src/main.rs`

**Step 1: Write failing tests**

Add tests proving:

- Agent queue runtime exposes `spawn_agent_queue_broker_consumer`,
- consumer uses `AgentRabbitMqClient::connect`,
- consumer uses `basic_consume` on Agent execute queue,
- consumer decodes `AgentQueueMessage`,
- consumer calls `claim_agent_run_queue_by_message`,
- consumer publishes `publish_agent_retry` and `publish_agent_dead`,
- consumer acks delivery after durable handling.

Run:

```bash
cargo test -p backend-rust agent_queue_broker_consumer --offline
```

Expected: FAIL until the runtime consumer exists.

### Task 3: Implement Minimal Consumer

Status: Pending.

**Files:**
- Modify: `backend/src/application/ai/agent_queue_runtime.rs`
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`
- Modify: `backend/src/main.rs`

**Step 1: Add repository claim helper**

Implement:

- `claim_agent_run_queue_by_message(queue_id, tenant_id, run_id, worker_id, lease_until, user_id, now)`.

**Step 2: Reuse execution completion logic**

Extract a helper that executes one `AgentRunQueueClaimRecord` and returns whether it completed, should retry, or should dead-letter.

**Step 3: Add consumer loop**

Implement:

- `spawn_agent_queue_broker_consumer`,
- `run_agent_queue_broker_consumer`,
- `consume_agent_execute_queue`.

**Step 4: Wire app startup**

Start the broker consumer when Agent queue is enabled, while leaving polling fallback active.

**Step 5: Verify focused**

```bash
cargo fmt -- --check
cargo test -p backend-rust agent_queue_broker_consumer --offline
cargo test -p backend-rust agent_queue_runtime --offline
cargo test -p backend-rust ai_agent_repository --offline
```

### Task 4: Docs, Full Verification, Merge

Status: Pending.

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-background-run-queue.md`
- Modify: `docs/plans/2026-06-17-agent-queue-broker-consumer.md`

**Step 1: Update docs**

Record broker consumer as implemented and keep Agent queue outbox publishing listed as remaining work.

**Step 2: Verify feature branch**

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd apps/agent-workspace && pnpm test -- src/api/agent.test.ts
cd apps/codex-app-poc && pnpm test -- src/api/agent.test.ts
```

**Step 3: Merge and verify main**

Merge `feat/enterprise-agent-foundation` into `main`, then rerun the same verification commands on `main`.
