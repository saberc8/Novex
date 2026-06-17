# Agent Provider Call Lease Heartbeat Design

## Goal

Keep durable provider-call leases accurate while a model call is still in flight.

The lease table already stores `heartbeat_at` and `lease_expires_at`, and the operator control plane can expire stale `running` rows. Without a heartbeat, a long provider call or future streaming response can exceed the initial lease window and look stale even though the worker is still awaiting the provider.

## Scope

This slice adds a provider-agnostic heartbeat inside `ModelRuntimeService`:

- Start a heartbeat task after `begin_model_provider_call_lease`.
- Refresh `heartbeat_at` every 30 seconds while the provider await is active.
- Extend `lease_expires_at` from each heartbeat by the existing lease TTL.
- Update only the matching `id`, `tenant_id`, and `status = 'running'` row.
- Stop and abort the heartbeat task immediately after the provider await returns, before writing terminal completion.

## Non-Goals

- Provider-native cancel API calls.
- WebSocket/token streaming.
- Embedding/rerank/media leases.
- New operator endpoints; the existing list/expire controls read the refreshed fields.

## Acceptance

- Unit tests prove heartbeat expiry extends from the heartbeat timestamp.
- Source-contract tests prove the model runtime starts/stops heartbeat around provider calls.
- Source-contract tests prove heartbeat SQL is tenant-scoped and only refreshes `running` rows.
- Existing provider-call lease tests continue to pass.
