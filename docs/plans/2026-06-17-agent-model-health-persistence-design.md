# Agent Model Health Persistence Design

## Context

`GET /ai/models/ops-summary` now reads the latest `ai_model_health_check` row, but the model health-check path only returns live probe results. The table exists, and the scheduler runtime has builtin jobs, but no production path currently writes model health status into the table.

This leaves the enterprise control plane one step short: operators can inspect route usage and breakers, but health state remains empty unless another future component writes it.

## Goal

Persist model health-check results and expose a scheduler builtin that can refresh health state without an operator calling the HTTP endpoint.

## Design

1. `ModelRuntimeService::health_check_for_tenant` keeps returning the existing response shape, but also records every target result into `ai_model_health_check`.
2. Health persistence is tenant-scoped and best-effort per result. The service maps a result target to the selected route purpose, then looks up the active route row to attach `route_id`, `provider_id`, and `model_profile_id`.
3. Add a service method that selects all tenants with active model routes and runs persisted health checks for each tenant.
4. Add scheduler builtin key `ai.model.health_check`. The scheduler executor invokes the all-tenant model health refresh and records a compact response body.

## Tenant Behavior

The HTTP model health-check endpoint is already tenant-bound through `CurrentUser`. It will persist rows only for that tenant.

The scheduler job table is not tenant-scoped. The builtin therefore scans active tenants from `ai_model_route` and runs the same persisted health-check path for each tenant.

## Out Of Scope

- Adding tenant columns to scheduler jobs.
- Adding alerting rules or notification delivery.
- Creating a default scheduled job seed.
- Changing the `ai_model_health_check` schema.

## Acceptance

- Source contract proves `health_check_for_tenant` persists `ai_model_health_check`.
- Source contract proves active model tenants are selected from `ai_model_route`.
- Pure builder test proves persisted health records map result status, target, latency, HTTP status, and detail safely.
- Scheduler executor supports builtin key `ai.model.health_check`.
- Matrix records this as the health persistence slice after ops summary.
- `cargo fmt -- --check`
- `cargo test --workspace --offline`
