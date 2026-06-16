# Agent Route Breaker Controls Design

## Context

Model route circuit breaker state is now persisted in `ai_model_route_circuit_breaker`, and runtime fallback reads/writes that table. The remaining control-plane gap is operational visibility and manual recovery. Operators cannot currently see which routes are open or clear a route after external remediation.

## Goal

Expose minimal enterprise controls for model route circuit breakers:

- list current breaker rows for the tenant,
- clear a breaker row by route id,
- enforce model control-plane permissions,
- keep runtime behavior aligned with DB as the cross-process source of truth.

## API

Add two backend endpoints:

- `GET /ai/models/route-circuit-breakers`
- `DELETE /ai/models/route-circuit-breakers/:route_id`

Use two permissions:

- `ai:model:circuitBreaker:list`
- `ai:model:circuitBreaker:clear`

The list response includes route id, opened-until timestamp, open reason, last error kind/status, whether the row is still open, and remaining milliseconds. The clear response uses the existing empty success envelope.

## Runtime Semantics

Persistent breaker state is the cross-process source of truth. Runtime route execution should check the persistent row before the local process cache. This matters because a manual clear removes the DB row, but cannot directly clear every other backend process's in-memory map.

The process-local map remains useful as a short-lived same-process optimization and as a fallback if the DB row was opened by the same process. Manual clear removes the DB row and clears the current process map. Other processes will stop respecting stale local entries once they observe the missing persistent row first.

## Data Flow

List:

1. HTTP handler checks `ai:model:circuitBreaker:list`.
2. `ModelRuntimeService::list_route_circuit_breakers` queries tenant-scoped rows ordered by `opened_until DESC`.
3. Service maps DB rows into API response fields.

Clear:

1. HTTP handler checks `ai:model:circuitBreaker:clear`.
2. `ModelRuntimeService::clear_route_circuit_breaker` validates route id.
3. Service deletes the tenant-scoped DB row and clears the current process local map.

## Alternatives Considered

1. **Only expose service methods.** Useful internally, but not an enterprise control plane.
2. **Add a full dashboard now.** More user-facing value, but too much surface for this slice.
3. **Minimal API + permissions.** Small, testable, and enough for future UI/dashboard work. This is the chosen path.

## Out Of Scope

- Frontend dashboard page.
- Bulk clear.
- Background cleanup of expired rows.
- Per-provider aggregate metrics.
- Audit log table for manual clear actions.

## Acceptance

- Permission seed contains list/clear breaker permissions.
- HTTP handlers reject missing permissions before DB access.
- Service source contract proves tenant-scoped list/delete queries.
- Runtime checks persistent breaker state before local cache.
- `cargo fmt -- --check`
- `cargo test --workspace --offline`
