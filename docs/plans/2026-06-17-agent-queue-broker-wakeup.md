# Agent Queue Broker Wake-Up Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an Agent-specific RabbitMQ wake-up message contract and publisher abstraction while keeping Postgres as the Agent queue source of truth.

**Progress 2026-06-17:** Implemented Agent RabbitMQ config/message/client types, AppConfig topology mapping, Agent queue wake-up message builder, publisher trait, fake-publisher tests, and local env/compose wiring for the Agent topology.

**Architecture:** Reuse the existing RabbitMQ infrastructure shape from scheduler/parser. Add Agent RabbitMQ config/message/client types, map app config into that topology from `agent_queue_runtime`, and add a fake-publisher-tested wake-up message builder for queued run rows.

**Tech Stack:** Rust, lapin/RabbitMQ, SQLx/Postgres queue records, existing backend unit tests.

---

### Task 1: RabbitMQ Agent Topology Contract

Status: Completed.

**Files:**
- Modify: `backend/src/infrastructure/mq/rabbitmq.rs`

**Step 1: Write failing tests**

Add tests proving:

- `AgentRabbitMqConfig::default()` uses `novex.agent` topology,
- `AgentQueueMessage` serializes camelCase fields,
- RabbitMQ module exposes `AgentRabbitMqClient`, `publish_agent_execute`, and `declare_agent_topology`,
- publisher confirms are enabled for scheduler, parser, and agent clients.

Run:

```bash
cargo test -p backend-rust agent_queue_broker_wakeup --offline
```

Expected: FAIL until Agent RabbitMQ types exist.

### Task 2: Runtime Config And Publisher Abstraction

Status: Completed.

**Files:**
- Modify: `backend/src/shared/config.rs`
- Modify: `backend/src/application/ai/agent_queue_runtime.rs`

**Step 1: Write failing tests**

Add tests proving:

- `AppConfig` has Agent RabbitMQ topology fields,
- `agent_rabbitmq_from_config` maps env config into `AgentRabbitMqConfig`,
- `agent_queue_message_from_save_record` produces stable wake-up metadata,
- `AgentQueueMessagePublisher` can publish through a fake publisher.

Run:

```bash
cargo test -p backend-rust agent_queue_broker_wakeup --offline
```

Expected: FAIL until config, mapper, message builder, and trait exist.

### Task 3: Implement Minimal Broker Contract

Status: Completed.

**Files:**
- Modify: `backend/src/infrastructure/mq/rabbitmq.rs`
- Modify: `backend/src/shared/config.rs`
- Modify: `backend/src/application/ai/agent_queue_runtime.rs`

**Step 1: Add RabbitMQ types**

Add:

- `AgentRabbitMqConfig`,
- `AgentQueueMessage`,
- `AgentRabbitMqClient`,
- `publish_agent_execute`,
- `publish_agent_retry`,
- `publish_agent_dead`,
- `declare_agent_topology`.

**Step 2: Add app config fields**

Add env-backed fields:

- `RABBITMQ_AGENT_EXCHANGE`,
- `RABBITMQ_AGENT_EXECUTE_QUEUE`,
- `RABBITMQ_AGENT_RETRY_QUEUE`,
- `RABBITMQ_AGENT_DEAD_QUEUE`,
- `RABBITMQ_AGENT_EXECUTE_ROUTING_KEY`,
- `RABBITMQ_AGENT_RETRY_ROUTING_KEY`,
- `RABBITMQ_AGENT_DEAD_ROUTING_KEY`,
- `RABBITMQ_AGENT_RETRY_TTL_MS`.

**Step 3: Add runtime helper**

Add:

- `agent_rabbitmq_from_config`,
- `AgentQueueMessagePublisher`,
- `agent_queue_message_from_save_record`,
- fake-publisher test.

**Step 4: Verify focused**

```bash
cargo fmt -- --check
cargo test -p backend-rust agent_queue_broker_wakeup --offline
cargo test -p backend-rust agent_queue_runtime --offline
cargo test -p backend-rust rabbitmq --offline
```

### Task 4: Docs, Full Verification, Merge

Status: In progress.

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-background-run-queue.md`
- Modify: `docs/plans/2026-06-17-agent-queue-broker-wakeup.md`

**Step 1: Update docs**

Record broker wake-up message/topology as implemented and keep live consumer/outbox integration listed as remaining work.

**Step 2: Verify feature branch**

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd apps/agent-workspace && pnpm test -- src/api/agent.test.ts
cd apps/codex-app-poc && pnpm test -- src/api/agent.test.ts
```

**Step 3: Merge and verify main**

Merge `feat/enterprise-agent-foundation` into `main`, then rerun the same verification commands on `main`.
