# Agent Provider Call Lease Controls Design

## Goal

Turn the durable provider-call lease table into an operator-visible control-plane surface.

The previous slice records tenant-bound chat/model provider calls in `ai_model_provider_call_lease` and links Agent `model_inference` events to `providerCallLeaseId`. The remaining immediate gap is visibility and stale lease recovery: operators need to list active/recent provider calls and mark expired `running` leases terminal when a worker dies or a provider await is abandoned by cancellation.

## Scope

This slice adds backend-only controls:

- `GET /ai/models/provider-call-leases`
  - Lists tenant-scoped provider-call leases.
  - Supports optional `status`, `runId`, and bounded `limit`.
  - Defaults to active/recent rows ordered by `started_at DESC`.
- `POST /ai/models/provider-call-leases/expire-stale`
  - Marks current-tenant `running` leases with `lease_expires_at < now()` as `expired`.
  - Sets completion fields, `error_kind = lease_expired`, and an audit `update_user`.
- Permission seeds:
  - `ai:model:providerCallLease:list`
  - `ai:model:providerCallLease:expire`

## Non-Goals

- Provider-native cancellation API calls.
- Streaming heartbeat refresh.
- Embedding/rerank/media provider leases.
- UI screens.

## Acceptance

- Source-contract tests prove service methods query/update `ai_model_provider_call_lease` tenant-scoped rows.
- HTTP tests prove routes are registered and permission-gated before DB/network work.
- Mapping tests prove response DTOs expose lifecycle fields without prompt/answer payload content.
- Migration matrix moves lease list/expire controls from remaining runtime-loop work into implemented evidence while leaving provider-native cancel and heartbeat refresh as next.
