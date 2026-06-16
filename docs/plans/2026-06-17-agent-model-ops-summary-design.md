# Agent Model Ops Summary Design

## Context

The Codex migration matrix now has route retry, fallback, circuit-breaker, persistent breaker state, and breaker control APIs. The remaining rollout operations gap is observability: operators can list breaker rows, but cannot see route-level health, usage, cost, and degraded status in one stable control-plane response.

This matters for enterprise Agent workloads because Chat Flow, POC, customer service, knowledge-base RAG, and Notebook-style workflows all share the model route registry. When a route degrades, operators need a tenant-scoped summary before opening trace bundles or eval reports.

## Goal

Expose a backend model operations summary API that aggregates:

- configured model routes,
- provider/model/network metadata,
- currently open route circuit breakers,
- latest persisted health-check row when present,
- 24-hour usage, token, cost, and latency aggregates.

## API

Add:

- `GET /ai/models/ops-summary`

Add permission:

- `ai:model:opsSummary`

The response is intentionally dashboard-ready but UI-neutral:

- top-level route counts,
- active route count,
- open breaker count,
- degraded route count,
- tenant-wide 24-hour usage summary,
- route rows with route id, purpose, provider, model, network zone, breaker state, latest health state, and 24-hour usage.

## Data Sources

- `ai_model_route`, `ai_model_profile`, `ai_model_deployment`, `ai_model_provider` for configured route metadata.
- `ai_model_route_circuit_breaker` for open breaker state. Join by tenant and route code.
- `ai_model_health_check` for latest route health if health rows exist. Join by route table id.
- `ai_model_usage` for last-24-hour request, token, cost, and latency aggregates. Join by route table id.

## Degraded Semantics

A route is degraded when:

- the persisted breaker row is currently open, or
- the latest health status exists and is not `ok`.

Missing health data does not make a route degraded. It means the route has not been probed or no probe row has been recorded.

## Alternatives Considered

1. **Frontend dashboard first.** Visually useful, but it would either duplicate SQL logic or rely on unstable ad hoc endpoints.
2. **Trace/eval-only observability.** Strong for postmortems, but too indirect for operators deciding whether to clear breakers or reroute traffic.
3. **Backend ops summary first.** Small, tenant-scoped, testable, and a stable contract for future dashboards and monitors. This is the chosen slice.

## Out Of Scope

- Frontend dashboard page.
- Alerting or SLO notifications.
- Background health-check scheduler.
- Persisting health-check results from the existing manual health-check endpoint.
- Per-provider time-series storage.

## Acceptance

- Permission seed contains `ai:model:opsSummary`.
- `GET /ai/models/ops-summary` is registered and rejects missing permission before DB access.
- Service source contract proves tenant-scoped route metadata, breaker, health, and 24-hour usage joins.
- Pure summary builder test proves open breaker and non-ok health mark routes degraded.
- Matrix records this as the next rollout operations slice.
- `cargo fmt -- --check`
- `cargo test --workspace --offline`
