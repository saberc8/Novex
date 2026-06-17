# Agent Event WebSocket Browser Auth Design

## Context

Novex now exposes an authenticated durable run-event WebSocket transport at `GET /ai/agents/runs/:run_id/events/ws`. That endpoint works for SDKs and non-browser clients that can set the `Authorization` header. Browser-native `WebSocket` cannot set custom headers, and putting the long-lived login JWT directly into a query string would leak the primary credential into browser history, reverse-proxy logs, and observability tools.

The next browser-safe slice is a short-lived WebSocket ticket handoff:

1. The browser calls an authenticated HTTP endpoint with its normal `Authorization` header.
2. The backend validates tenant/user/permission and issues a short-lived, purpose-bound ticket.
3. The browser opens the WebSocket using `?ticket=...`.
4. The WebSocket extractor accepts either normal `Authorization` or a valid ticket, then the existing stream loop runs unchanged.

## Selected Slice

Add a short-lived JWT-backed ticket for agent run-event WebSocket connections:

- `POST /ai/agents/runs/:run_id/events/ws-ticket`
- Response body:

```json
{
  "ticket": "...",
  "expiresInSeconds": 60
}
```

- Ticket claims:
  - `purpose = "agent_run_event_ws"`
  - `user_id`
  - `username`
  - `run_id`
  - `iat`
  - `exp`

The ticket is not persisted in this slice. It is signed by the existing `JwtService`, expires quickly, and is scoped to one run id. This makes it appropriate for browser handoff while avoiding a new operational table.

## Transport Contract

The WebSocket route accepts either:

- `Authorization: Bearer <login-jwt>` for SDK/server clients.
- `?ticket=<short-lived-ticket>` for browser clients.

When both are present, `Authorization` wins. A ticket must match the path `run_id`; otherwise the request is unauthorized.

## Frontend Contract

Add helpers in both browser-facing POC packages:

- `createAgentRunEventWebSocketTicket(runId)`
- `agentRunEventWebSocketUrl(runId, ticket, query)`

The URL helper converts `http` to `ws` and `https` to `wss`, carries existing cursor query fields, and appends the ticket.

## Non-Goals

- No persistent ticket table or one-time-use revocation in this slice.
- No provider token-delta streaming.
- No UI live WebSocket panel.
- No cookie/session auth migration.
- No change to existing SSE endpoint.

## Validation

- JWT unit tests prove ticket claims are purpose-bound, run-bound, and rejected for the wrong run.
- Agent HTTP tests prove the ticket route is registered and requires auth.
- Agent HTTP/source tests prove the WebSocket route accepts a ticket principal before `WebSocketUpgrade`.
- Frontend tests prove ticket request uses normal HTTP auth and browser WebSocket URL uses `ws/wss` plus cursor query and ticket.
- Existing WebSocket and SSE tests remain green.
