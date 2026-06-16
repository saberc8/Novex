# Agent Model Ops Alert Delivery Design

## Context

Model ops alerts are now durable in `ai_model_ops_alert` and visible from `/ai/models/ops-summary`. They still stop at the control-plane API. Enterprise operators need a delivery bridge that can send or dry-run alert notifications without blocking the model health-check loop.

## Goal

Add a scheduler-driven model ops alert delivery bridge. It should deliver active model ops alerts through the existing Feishu message tool contract, fall back to audited dry-run when no webhook is configured, and persist both delivery attempts and tool-level audits.

## Approaches Considered

1. **Deliver during health-check persistence.** This is immediate, but it couples external network I/O to health checks and can slow or fail the health loop.
2. **Add a scheduler builtin delivery bridge.** This keeps health checks fast, gives retry cadence to the scheduler, and matches the existing `ai.model.health_check` operational pattern.
3. **Only expose alerts and leave delivery to frontend.** This avoids backend delivery code, but does not provide a server-side enterprise notification base.

Use approach 2. Scheduler-driven delivery is safer and gives us a reusable bridge for future Slack, PagerDuty, enterprise WeChat, or webhook targets.

## Data Model

Add `ai_model_ops_alert_delivery`:

- `alert_id`, `alert_key`, `tenant_id`
- `channel`, initially `feishu`
- `status`: `sent`, `dry_run`, or `failed`
- `dry_run`
- `tool_call_audit_id`
- `request_payload`, `response_payload`, `error_message`
- `create_user`, `create_time`

The delivery query selects active alerts with no successful or dry-run Feishu delivery record yet. Failed attempts remain retryable on the next scheduler run.

## Runtime Flow

1. Seed scheduler job `AI Model Alert Delivery` with builtin key `ai.model.alert_delivery`.
2. Scheduler executor calls `ModelRuntimeService::deliver_active_model_ops_alerts`.
3. Service queries unresolved active alerts not yet delivered through Feishu.
4. Each alert is converted to a Feishu text message payload.
5. If `FEISHU_WEBHOOK_URL` or `NOVEX_FEISHU_WEBHOOK_URL` is configured, send the webhook.
6. If not configured, record a dry-run delivery with the same request payload.
7. Every attempt writes `ai_tool_call_audit`.
8. Every attempt writes `ai_model_ops_alert_delivery` with the audit id.

## Error Handling

External send failures become `failed` delivery records and failed tool audits. The scheduler builtin still returns a compact summary of attempted, sent, dry-run, and failed counts. Database failures still fail the scheduler execution.

## Non-Goals

- New public HTTP APIs.
- Alert acknowledgement or assignment.
- Multiple delivery channels in this slice.
- Repeated reminder/escalation policies.

## Validation

- Migration contract proves delivery table, indexes, and seeded `ai.model.alert_delivery` job.
- Pure tests prove alert delivery message and dry-run payload shape.
- Source contract proves scheduler builtin dispatches `ai.model.alert_delivery`.
- Source contract proves delivery writes both `ai_tool_call_audit` and `ai_model_ops_alert_delivery`.
- Existing model health/ops summary tests remain green.
- `cargo fmt -- --check` and `cargo test --workspace --offline` pass on feature and on `main`.
