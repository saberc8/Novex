# Agent Model Health Automation Design

## Context

`GET /ai/models/ops-summary` can now read persisted health rows, and the scheduler executor can run builtin key `ai.model.health_check`. The missing production loop is automation: a default job must exist after migration, and failed model health checks need an alert-grade record that survives process restarts.

## Goal

Seed a default model-health scheduler job and record active model ops alerts when health checks fail. Resolve those alerts when a later check succeeds for the same target.

## Approaches Considered

1. **Only seed the scheduler job.** This is low risk and immediately refreshes health rows, but operators still have to infer incident state from raw rows.
2. **Add a dedicated `ai_model_ops_alert` table.** This keeps model SLO alerts close to model ops, supports active-alert dedupe, and can feed future dashboards or notification bridges.
3. **Reuse `ai_trigger_event` for alerts.** This would reuse existing event listing, but trigger events require a configured trigger and are shaped around inbound trigger delivery instead of model SLO state.

Use approach 2 plus the default scheduler seed. It is the smallest path that creates a durable ops signal without forcing notification delivery in this slice.

## Design

Add migration `202606170004_create_ai_model_ops_alert.sql`.

The migration creates `ai_model_ops_alert` with tenant, alert key, kind, severity, status, optional route/provider/profile ids, a source ref to the health-check row, JSON payload, first/last seen timestamps, and optional resolution fields. A partial unique index on `(tenant_id, alert_key)` where `resolved_at IS NULL` deduplicates active alerts. The same migration seeds `sys_job` id `3600001` as an enabled builtin job:

- name: `AI Model Health Check`
- group: `ai-ops`
- task type: builtin
- builtin key: `ai.model.health_check`
- cron: every five minutes
- timeout: 120 seconds
- max retry: 1

`ModelRuntimeService::health_check_for_tenant` already persists each result. Extend that persistence path so every failed result upserts one active alert, while every successful result resolves any matching active alert. Alert keys are stable by tenant, target, and route id when present, otherwise by tenant and unconfigured target.

The alert payload should include the health-check id, target, configured flag, route/provider/profile ids, HTTP status, latency, message, and detail. It must not expose raw secrets; it may carry the existing masked API key.

## Data Flow

1. Scheduler enqueues seeded `sys_job`.
2. Executor runs builtin `ai.model.health_check`.
3. Model service checks active tenant routes and inserts `ai_model_health_check` rows.
4. Failed rows upsert active alerts.
5. Successful rows resolve matching active alerts.
6. Future dashboard/notification work reads active alerts by tenant and joins model ops summary.

## Non-Goals

- Sending external notifications.
- Adding new HTTP alert APIs.
- Changing scheduler tenancy.
- Changing the model health response shape.

## Validation

- Migration source contract proves `ai_model_ops_alert` schema, active-alert partial unique index, and seeded `ai.model.health_check` job.
- Unit tests prove alert keys are stable and failure/success outcomes map to upsert/resolve actions.
- Source contract proves the health-check persistence path calls alert recording after saving health rows.
- Existing scheduler builtin tests remain green.
- `cargo fmt -- --check` and `cargo test --workspace --offline` pass on feature and `main`.
