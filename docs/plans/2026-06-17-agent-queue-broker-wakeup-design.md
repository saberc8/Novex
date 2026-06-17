# Agent Queue Broker Wake-Up Design

## Goal

Move the Agent background queue from polling-only toward broker-assisted execution. PostgreSQL remains the source of truth for run state, queue rows, leases, and retries, while RabbitMQ carries lightweight wake-up messages so future workers can react quickly when a run is created or requeued.

## Current State

Novex now has:

- durable `ai_agent_run_queue` rows,
- queued run creation,
- model-loop and deterministic existing-run execution,
- queue cancellation sync,
- approval resume requeue,
- embedded worker polling with `FOR UPDATE SKIP LOCKED`.

The remaining queue gap is transport. The worker currently discovers work only by polling. This is reliable but sluggish at scale and does not expose a clean broker contract for external Agent workers.

## Selected Design

Add an Agent-specific RabbitMQ topology and wake-up message contract:

- exchange: `novex.agent`
- execute queue: `novex.agent.execute`
- retry queue: `novex.agent.retry`
- dead queue: `novex.agent.dead`
- execute routing key: `agent.execute`
- retry routing key: `agent.retry`
- dead routing key: `agent.dead`

Add `AgentQueueMessage` with the smallest stable envelope:

```json
{
  "queueId": 1,
  "tenantId": 1,
  "runId": 2,
  "event": "agent.run.queued",
  "attempt": 0,
  "maxAttempts": 3,
  "source": "agent.create_run"
}
```

The message is a wake-up signal, not the source of execution state. A worker that receives it should claim from Postgres by normal lease rules and ignore stale messages if the run is already terminal or not claimable.

## Scope For This Slice

This slice adds:

- `AgentRabbitMqConfig`,
- `AgentQueueMessage`,
- `AgentRabbitMqClient`,
- `agent_rabbitmq_from_config`,
- `AgentQueueMessagePublisher` trait,
- `agent_queue_message_from_save_record`,
- unit tests and docs proving the topology, serialization, and publisher path.

It does not yet route HTTP creation through a live RabbitMQ publisher because `AgentService` is currently constructed per request without publisher dependencies. The next slice can add an optional application-level publisher/outbox integration once the message and topology contract are stable.

## Why This Shape

Codex-style worker control needs durable state and fast signals. RabbitMQ should wake workers, but it should not own correctness. This design keeps the current DB queue safe under duplicate, delayed, or lost broker messages. The system can continue to work by polling when RabbitMQ is disabled or temporarily unavailable.

## Options Considered

### Option A: Replace polling with RabbitMQ consume now

This is too wide for the current architecture. It would need consumer lifecycle, ack/retry/dead-letter behavior, and a clean worker binary or embedded consumer. It is the right later shape, not the first safe broker slice.

### Option B: Publish wake-up messages directly from `AgentService`

This is operationally attractive, but it would couple request-scoped service construction to a live broker client. Without an outbox or injected publisher in `AppState`, RabbitMQ failure could leak into run creation semantics. That is too risky before the message contract exists.

### Option C: Add broker contract and publisher abstraction first

Selected. It establishes the direct adapter port from existing RabbitMQ infrastructure to Agent queue messages while preserving the DB-backed queue invariants already implemented.

## Non-Goals

- No live RabbitMQ consumer in this slice.
- No Agent queue outbox table in this slice.
- No replacement of polling worker.
- No provider-native abort.
- No UI changes.

## Verification

- RabbitMQ module tests prove Agent topology defaults and message camelCase serialization.
- Runtime tests prove Agent queue config maps to dedicated RabbitMQ topology.
- Publisher abstraction tests prove wake-up messages can be built from queue save records and published through a fake publisher.
- Existing queue/runtime/workspace tests remain green.

