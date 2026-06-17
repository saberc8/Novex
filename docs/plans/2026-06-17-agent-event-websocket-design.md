# Agent Event WebSocket Transport Design

## Context

The migration matrix still lists streaming transport as a Runtime loop gap. Novex already has a durable agent run-event stream over SSE:

- Events are stored with monotonic `sequence_no`.
- `AgentService::list_events_after_sequence` enforces tenant scoping and cursor paging.
- `AgentService::is_run_terminal` lets the stream close once the run is complete.
- `AgentRunEventStreamQuery` clamps cursor, batch, poll, and idle settings.

This gives Novex the right application boundary for a second realtime transport. WebSocket should be an HTTP adapter over the same cursor-based event stream, not a second event store or provider-specific streaming path.

## Selected Slice

Add an authenticated WebSocket endpoint for durable agent run events:

1. Register `GET /ai/agents/runs/:run_id/events/ws`.
2. Reuse `CurrentUser` and `ai:agent:event:list`.
3. Reuse `AgentRunEventStreamQuery` settings.
4. Reuse `AgentService::list_events_after_sequence` and `AgentService::is_run_terminal`.
5. Emit text JSON frames shaped for clients and SDKs:

```json
{
  "type": "agent_run_event",
  "sequenceNo": 9,
  "event": {}
}
```

6. Emit typed error frames before closing on service errors:

```json
{
  "type": "error",
  "message": "..."
}
```

The endpoint is intentionally server/SDK friendly in this slice because the existing `CurrentUser` extractor authenticates via the `Authorization` header. Browser-native `WebSocket` cannot set that header, so browser token handoff via query token, cookie, or subprotocol remains a separate security design.

## Non-Goals

- No provider token-delta streaming in this slice.
- No query-token or subprotocol authentication in this slice.
- No binary WebSocket protocol.
- No frontend WebSocket helper.
- No new event persistence model.
- No second runtime loop.

## Interfaces

- `stream_events_ws(...) -> Result<Response, AppError>`
- `agent_run_event_ws_loop(socket, service, run_id, settings)`
- `agent_run_event_ws_message(event) -> String`
- `agent_run_event_ws_error_message(err) -> String`

## Validation

- Unit/source-contract tests prove the route is registered and uses WebSocket upgrade.
- Unit/source-contract tests prove the route keeps `ai:agent:event:list` permission.
- Unit tests prove event frames include `type`, `sequenceNo`, and the original event payload.
- Unit tests prove error frames are typed.
- Existing SSE event stream tests remain green.
- Workspace tests remain green.
