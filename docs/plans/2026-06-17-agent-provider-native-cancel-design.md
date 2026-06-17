# Agent Provider Native Cancel Design

## Context

The Runtime loop matrix now has a durable provider-call lease table, active run cancellation tokens, persistent run-status abort watchers, and provider-call lease heartbeats. The remaining cancellation gap is provider-native cancellation: Novex can stop its own model loop, but operators do not yet have a control-plane action that targets an in-flight provider call.

Codex treats interruption as a first-class runtime boundary: active tasks receive cancellation, app-server request ids can be cancelled, and interrupted turns leave model-visible history markers. For OpenAI Responses, the provider-native HTTP shape is `POST /responses/{response_id}/cancel`, and it only applies to background Responses that already have a response id. Novex should not fake this for synchronous chat calls that never received a provider response id.

## Selected Approach

Add a narrow provider-call lease cancel control:

1. `POST /ai/models/provider-call-leases/:lease_id/cancel` requires a new permission.
2. The service loads the tenant-bound lease row including request and response payloads.
3. If the row is already terminal, return an idempotent response without mutating it.
4. If the row is running, derive a provider-native cancel plan.
5. When a supported route and provider response id are present, issue `POST {baseUrl}/responses/{responseId}/cancel`.
6. Whether native cancel is supported or not, mark the lease `cancelled` locally with structured evidence so the control plane stops showing it as active.
7. Completion updates must only update `running` rows so a late provider future cannot overwrite a cancelled or expired lease.

This is an adapter port rather than a direct port. Codex's local interrupt path owns task cancellation; Novex already has that. The new piece maps provider cancellation into the enterprise control plane through durable lease ids, tenant authorization, route resolution, and audit-friendly response payloads.

## Native Cancel Plan

Supported in this slice:

- `openai-compatible` and `local-runtime` routes.
- Provider response id found in `request_payload` or `response_payload` at one of:
  - `providerResponseId`
  - `responseId`
  - `id`

Endpoint:

- `join_model_endpoint(route.base_url(), Some(&format!("responses/{response_id}/cancel")))`

Unsupported responses return a plan with `attempted=false`, `supported=false`, and a reason such as `missing_provider_response_id` or `unsupported_provider`.

## API Response

`ModelProviderCallLeaseCancelResp`:

- `leaseId`
- `status`
- `nativeCancel`

`nativeCancel`:

- `attempted`
- `supported`
- `provider`
- `providerResponseId`
- `endpoint`
- `httpStatus`
- `message`

The public response does not expose API keys or prompt/answer payloads.

## Non-Goals

- Do not add WebSocket streaming transport in this slice.
- Do not introduce background Responses sampling yet.
- Do not store provider request ids from headers yet.
- Do not cancel provider calls for every provider family; unsupported providers get a durable local cancellation record.

## Acceptance

- Unit tests prove cancel plan endpoint construction for OpenAI-compatible routes.
- Unit tests prove unsupported plans do not attempt network cancellation.
- Unit tests prove cancellation completion payloads include native cancel evidence.
- Source-contract tests prove the HTTP route, permission, and migration exist.
- Source-contract tests prove lease completion only updates `status = 'running'` rows.
- Existing provider abort, provider-call lease, and compaction tests stay green.
