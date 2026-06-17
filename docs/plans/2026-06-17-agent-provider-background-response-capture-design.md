# Agent Provider Background Response Capture Design

## Context

The migration matrix still lists background Responses response-id capture as a Runtime loop gap. The previous native cancel slice can dispatch `POST /responses/{response_id}/cancel`, but it only works when Novex has captured a provider response id from the original provider call.

OpenAI background mode starts a Responses request with `background=true`; clients then track the returned `id` and `status` for polling, cancellation, and resumable streaming. The official background guide also states that background streaming uses both `background=true` and `stream=true`, and that background sampling requires `store=true`.

## Selected Slice

Add provider response metadata capture to the existing Responses compaction transport:

1. For compatible Responses compaction v2 requests, include `background=true`, `store=true`, and `stream=true`.
2. Parse provider response `id` and `status` from JSON Responses bodies.
3. Parse provider response `id` and terminal `status` from SSE `response.*` events, especially `response.completed`.
4. Carry the captured metadata on `ModelChatResp` as optional fields.
5. Persist the metadata into `ai_model_provider_call_lease.response_payload` as `providerResponseId` and `providerResponseStatus`.
6. Keep public answer content out of the lease payload.

This is an adapter port. Codex's runtime can interrupt supervised tasks directly; Novex must additionally preserve provider identifiers in the enterprise control plane so cross-request cancellation, lease inspection, trace/eval, and future background polling have a durable join key.

## Non-Goals

- No WebSocket transport.
- No provider polling worker.
- No resumable stream endpoint.
- No change to Chat Completions routes.
- No full background agent task join-handle lifecycle.

## Interfaces

- `ModelChatResp.provider_response_id: Option<String>`
- `ModelChatResp.provider_response_status: Option<String>`
- `model_chat_responses_compaction_payload(route, command)` includes `background=true`, `store=true`, and `stream=true`.
- `model_chat_compaction_provider_output_from_body(body)` returns output with provider id/status.
- `model_chat_compaction_provider_output_from_sse_text(body_text)` returns output with provider id/status.
- `model_provider_call_lease_completion_from_response(response, ...)` persists captured provider metadata.

## Validation

- Unit tests prove Responses compaction v2 payload enables background mode and store.
- Unit tests prove JSON Responses bodies capture `id/status`.
- Unit tests prove SSE `response.completed.response.id/status` captures terminal provider metadata.
- Unit tests prove provider-call lease completion records `providerResponseId/providerResponseStatus` without leaking `answer`.
- Existing provider compaction, provider-call lease, provider abort, and workspace tests remain green.
