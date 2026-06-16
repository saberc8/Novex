# Agent Event Stream Design

## Goal

Port the Codex event-queue interaction shape into Novex Agent runs. Clients should be able to follow a run as events are produced instead of repeatedly polling snapshots.

This is a first production-safe streaming slice, not the final transport. It should expose a replayable cursor over the existing `ai_run_event` table and leave room for replacing the polling loop with Postgres `LISTEN/NOTIFY`, Redis Streams, NATS, or a runtime event bus later.

## Codex Reference

Codex protocol is built around Submission Queue and Event Queue:

- `Submission` carries a unique id and an operation.
- `Event` carries the same id plus an `EventMsg` payload.
- `Session::send_event` records rollout/event state and forwards the event to clients.
- Clients filter notifications by thread and turn id and can recover from stream lag.

Novex already has the durable event log:

- `ai_run_event.run_id`
- `sequence_no`
- `event_type`
- `status`
- `payload`

The missing piece is the streaming facade over this event log.

## Selected Design

Add:

```text
GET /ai/agents/runs/:run_id/events/stream
```

Query:

- `afterSequenceNo`: replay cursor. Default `0`.
- `batchSize`: number of events fetched per poll. Default `50`, max `200`.
- `pollMs`: database polling interval. Default `1000`, clamped to `250..5000`.
- `maxIdleMs`: idle stream lifetime. Default `30000`, max `300000`.

SSE event shape:

```text
event: agent_run_event
id: <sequence_no>
data: <AgentRunEventResp JSON>
```

When the run reaches a terminal status and no newer events exist, the stream closes. If backend reading fails, the stream emits one `error` SSE event and closes.

## Layering

- `AiAgentRepository` gets a cursor query for events where `sequence_no > afterSequenceNo`.
- `AgentService` exposes cursor-based event listing and terminal status checks.
- HTTP layer owns SSE transport and retry/keepalive concerns.
- Frontend API helpers use `fetch` rather than native `EventSource`, because current authentication uses bearer headers and native `EventSource` cannot set custom headers.

## Security

The stream endpoint uses the same `ai:agent:event:list` permission as paginated event listing. It must bind to the current tenant through `AgentService::for_tenant_with_runtime`, so `run_id` alone cannot cross tenants.

## Acceptance

- Backend route is registered at `/ai/agents/runs/:run_id/events/stream`.
- Missing `ai:agent:event:list` permission is rejected before stream construction.
- Query normalization clamps cursor, batch size, poll interval, and idle timeout.
- SSE payload helper uses `sequence_no` as SSE id and `agent_run_event` as event name.
- Frontend API exposes a bearer-authenticated `fetchAgentRunEventStream` helper and URL/query tests.
- Migration matrix records Codex-style event stream slice progress.
