# Agent Model Ops Alert Summary Design

## Context

Model health automation now persists `ai_model_health_check` rows and maintains active rows in `ai_model_ops_alert`. The existing `/ai/models/ops-summary` endpoint already aggregates routes, breaker state, latest health, and 24-hour usage. Dashboard consumers still cannot see active model ops alerts without reading the database directly.

## Goal

Expose active model ops alerts through the existing model ops summary response so the enterprise control plane has one model-ops read path for route health, fallback/breaker state, usage, cost, and current incidents.

## Approaches Considered

1. **Extend `/ai/models/ops-summary` with alert counts and active alert details.** This keeps dashboard reads simple and preserves the current permission boundary.
2. **Add `/ai/models/ops-alerts` as a new paginated endpoint.** This is better for a full alert workbench, but it requires new permission seed and UI/API surface.
3. **Join only active alert counts, not details.** This is small but forces a second future pass before a dashboard can show what is wrong.

Use approach 1 now. It is the smallest change that turns the previous alert table into useful control-plane data.

## API Shape

Extend `ModelOpsSummaryResp` with:

- `activeAlertCount: usize`
- `alerts: Vec<ModelOpsAlertResp>`

Add `ModelOpsAlertResp`:

- `alertKey`
- `alertKind`
- `severity`
- `status`
- `routeId` as route code when the alert is route-bound
- `routePurpose`
- `provider`
- `model`
- `sourceRef`
- `message`
- `firstSeenAt`
- `lastSeenAt`
- `eventPayload`

Extend `ModelRouteOpsSummaryResp` with:

- `activeAlertCount`

Routes with active route-bound alerts should be marked `degraded`, even if the latest health row is stale or unavailable.

## Data Flow

1. `model_ops_summary` fetches existing route rows.
2. It fetches active alert rows from `ai_model_ops_alert` where `resolved_at IS NULL`.
3. The alert query left joins route/profile/deployment/provider by tenant and route DB id to enrich dashboard labels.
4. The pure summary builder receives route rows plus alert rows and computes top-level and per-route counts.
5. The HTTP handler keeps using the existing `ai:model:opsSummary` permission.

## Non-Goals

- Paginated alert history.
- Alert acknowledgement or assignment.
- External notification delivery.
- Frontend rendering.

## Validation

- Source contract proves `model_ops_summary` reads `ai_model_ops_alert` active rows.
- Pure unit test proves active alerts appear in the response and increment per-route counts.
- Pure unit test proves route-bound active alerts mark routes degraded.
- Existing model ops summary and model health automation tests remain green.
- `cargo fmt -- --check` and `cargo test --workspace --offline` pass on feature and on `main`.
