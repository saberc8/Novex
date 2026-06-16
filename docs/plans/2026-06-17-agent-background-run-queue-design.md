# Agent Background Run Queue Design

## Goal

Move Novex Agent execution toward Codex-grade runtime infrastructure by decoupling run creation from run execution. HTTP should be able to create a durable queued run quickly, clients should follow it through the event stream, and a worker should claim and execute it with retry/lease metadata.

## Current State

Novex already has:

- Run Graph tables: `ai_run`, `ai_agent_run`, `ai_run_event`, `ai_run_step`, `ai_run_pause`, `ai_agent_trace`.
- Inline deterministic and model-loop execution in `AgentService::create_run`.
- Runtime cancellation registry and replayable SSE event stream.
- Scheduler and parser queue infrastructure.

The missing enterprise runtime piece is a durable Agent-specific execution queue. `create_run` currently executes inside the HTTP request lifecycle.

## Codex Reference

Codex uses a session/task boundary: a submission enters the runtime, the turn runs asynchronously, and clients consume events as the task proceeds. Novex should adapt that shape to Run Graph:

- `ai_run` is the durable turn/session record.
- `ai_run_event` is the event queue.
- `ai_agent_run_queue` becomes the durable submission execution queue.

## Selected Approach

Add an Agent-specific database queue:

```text
ai_agent_run_queue
  id
  tenant_id
  run_id
  queue_status
  priority
  attempt_count
  max_attempts
  locked_by
  locked_until
  last_error
  payload
  queued_at
  started_at
  finished_at
```

Queue statuses:

- `pending`
- `running`
- `retrying`
- `succeeded`
- `failed`
- `cancelled`

Claiming uses `FOR UPDATE SKIP LOCKED` so multiple workers can safely compete. The first slice uses Postgres polling because it is reliable, inspectable, and already available. RabbitMQ/NATS can later become the wake-up transport while Postgres remains the source of truth.

## API Shape

Extend `AgentRunCommand` with:

```json
{
  "executionMode": "inline" | "queued"
}
```

Default remains `inline` to preserve current behavior and tests. `queued` creates:

- `ai_run.status = queued`
- `ai_agent_run.status = queued`
- initial `input_received` and `status_changed` events
- one `ai_agent_run_queue` row with the normalized command payload

The response remains `AgentRunResp`, so existing UI can immediately open `/events/stream`.

## Worker Flow

1. Claim pending/retryable queue rows with an expired or empty lease.
2. Mark the run and agent run `running`.
3. Execute the normalized command against the existing run id.
4. Mark queue `succeeded` on terminal success/waiting-approval.
5. Mark queue `retrying` or `failed` on infrastructure errors.
6. If a run is already terminal/cancelled before execution, mark queue `cancelled`.

Waiting for human approval is not a worker failure. The queue item is done once the runtime reaches a durable pause state; resume creates another foreground action for now.

## Code Structure

- `AiAgentRepository`: queue table records, enqueue, claim, complete/fail/cancel methods.
- `AgentService`: create queued run, execute existing queued run, command normalization.
- `agent_queue_runtime.rs`: config, worker loop, tick helper.
- `main.rs`: optional embedded worker controlled by env.

## Error Handling

- Claim lease defaults to short finite duration.
- Retry count increments on each claim.
- Retryable failures re-enter `retrying` until `max_attempts`.
- Final failure updates both queue and run status, and emits an error/status event where possible.
- Worker id is recorded for audit.

## Acceptance

- Migration creates `ai_agent_run_queue` with status, lease, run id uniqueness, and queue indexes.
- `AgentRunCommand` accepts `executionMode = queued` and defaults to inline.
- Queued creation returns an Agent run in `queued` status and records an enqueue payload.
- Worker claim SQL uses `FOR UPDATE SKIP LOCKED`.
- Worker execution path calls an existing-run execution function, not `create_run`.
- Event stream remains the client-facing progress surface.
- Config can enable an embedded worker without requiring RabbitMQ.
- Full workspace and focused agent queue tests pass before merging to `main`.
