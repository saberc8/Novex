# Agent Model Ops Alert Delivery Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deliver active model ops alerts through a scheduler-driven Feishu/dry-run bridge with durable delivery records and tool audits.

**Architecture:** Add `ai_model_ops_alert_delivery` and seed builtin scheduler job `ai.model.alert_delivery`. Extend `ModelRuntimeService` with a delivery scanner, Feishu/dry-run delivery execution, tool-call audit persistence, and delivery-attempt persistence. Wire the builtin into the scheduler executor.

**Tech Stack:** Rust, SQLx, PostgreSQL migrations, Reqwest, `novex-connectors::FeishuTextMessage`, scheduler builtin jobs, existing `ai_tool_call_audit`.

---

### Task 1: Migration Contract and Scheduler Seed

**Files:**
- Create: `backend/migrations/202606170005_create_ai_model_ops_alert_delivery.sql`
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write the failing test**

Add in `model_service.rs` tests:

```rust
#[test]
fn model_ops_alert_delivery_migration_defines_table_and_seed_job() {
    let migration = include_str!(
        "../../../migrations/202606170005_create_ai_model_ops_alert_delivery.sql"
    );

    assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_model_ops_alert_delivery"));
    assert!(migration.contains("idx_ai_model_ops_alert_delivery_alert_id"));
    assert!(migration.contains("idx_ai_model_ops_alert_delivery_channel_status"));
    assert!(migration.contains("INSERT INTO sys_job"));
    assert!(migration.contains("'ai.model.alert_delivery'"));
    assert!(migration.contains("'AI Model Alert Delivery'"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend model_ops_alert_delivery_migration --offline
```

Expected: FAIL because the migration file does not exist.

**Step 3: Create migration**

Create `backend/migrations/202606170005_create_ai_model_ops_alert_delivery.sql`:

```sql
CREATE TABLE IF NOT EXISTS ai_model_ops_alert_delivery (
    id BIGINT NOT NULL,
    tenant_id BIGINT NOT NULL DEFAULT 1,
    alert_id BIGINT NOT NULL,
    alert_key VARCHAR(160) NOT NULL,
    channel VARCHAR(64) NOT NULL,
    status VARCHAR(32) NOT NULL,
    dry_run BOOLEAN NOT NULL DEFAULT TRUE,
    tool_call_audit_id BIGINT DEFAULT NULL,
    request_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    response_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_message TEXT DEFAULT NULL,
    create_user BIGINT NOT NULL,
    create_time TIMESTAMP NOT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_model_ops_alert_delivery_alert_id
    ON ai_model_ops_alert_delivery (alert_id);
CREATE INDEX IF NOT EXISTS idx_ai_model_ops_alert_delivery_channel_status
    ON ai_model_ops_alert_delivery (tenant_id, channel, status, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_ai_model_ops_alert_delivery_audit_id
    ON ai_model_ops_alert_delivery (tool_call_audit_id);

INSERT INTO sys_job (
    id, name, group_name, task_type, cron_expression, status, concurrent,
    misfire_policy, max_retry, timeout_seconds, http_method, http_url,
    http_headers, http_body, builtin_key, description, next_trigger_time,
    create_user, create_time
) VALUES (
    3600002, 'AI Model Alert Delivery', 'ai-ops', 2, '*/5 * * * * *', 1, FALSE,
    1, 1, 120, NULL, NULL,
    '{}'::jsonb, NULL, 'ai.model.alert_delivery',
    'Deliver active model ops alerts through the configured notification bridge.',
    NOW(), 1, NOW()
)
ON CONFLICT DO NOTHING;
```

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p backend model_ops_alert_delivery_migration --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/migrations/202606170005_create_ai_model_ops_alert_delivery.sql backend/src/application/ai/model_service.rs
git commit -m "feat: seed model ops alert delivery"
```

### Task 2: Delivery Message and Result Mapping

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing pure tests**

Add:

```rust
#[test]
fn model_ops_alert_delivery_message_contains_operational_context() {
    let now = NaiveDateTime::parse_from_str("2026-06-17 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let alert = model_ops_alert_delivery_candidate("model_health:llm:route:11", now);

    let message = model_ops_alert_delivery_message(&alert);

    assert!(message.contains("Novex Model Alert"));
    assert!(message.contains("critical"));
    assert!(message.contains("runtime.llm.chat"));
    assert!(message.contains("provider unavailable"));
}

#[test]
fn model_ops_alert_delivery_dry_run_result_preserves_feishu_payload() {
    let now = NaiveDateTime::parse_from_str("2026-06-17 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let alert = model_ops_alert_delivery_candidate("model_health:llm:route:11", now);

    let result = model_ops_alert_delivery_dry_run_result(&alert);

    assert_eq!(result.status, "dry_run");
    assert!(result.dry_run);
    assert_eq!(result.request_payload["toolCode"], "feishu.message.send");
    assert_eq!(result.response_payload["status"], "dry_run");
    assert!(result.error_message.is_none());
}
```

Add helper:

```rust
fn model_ops_alert_delivery_candidate(alert_key: &str, now: NaiveDateTime) -> ModelOpsAlertDeliveryCandidateRow {
    ModelOpsAlertDeliveryCandidateRow {
        alert_id: 42,
        tenant_id: 1,
        alert_key: alert_key.to_owned(),
        alert_kind: "model_health".to_owned(),
        severity: "critical".to_owned(),
        route_code: Some("runtime.llm.chat".to_owned()),
        route_purpose: Some("chat".to_owned()),
        provider_code: Some("deepseek".to_owned()),
        model_name: Some("deepseek-v4".to_owned()),
        source_ref: "health_check:99".to_owned(),
        event_payload: json!({"message":"provider unavailable"}),
        first_seen_at: now,
        last_seen_at: now,
    }
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p backend model_ops_alert_delivery_message --offline
```

Expected: FAIL because candidate/result helpers do not exist.

**Step 3: Implement minimal structs/helpers**

Add:

- `MODEL_ALERT_DELIVERY_TOOL_CODE`
- `MODEL_ALERT_DELIVERY_CHANNEL_FEISHU`
- `ModelOpsAlertDeliveryCandidateRow`
- `ModelOpsAlertDeliveryResult`
- `model_ops_alert_delivery_message`
- `model_ops_alert_delivery_request_payload`
- `model_ops_alert_delivery_dry_run_result`

Use `FeishuTextMessage::new(message).to_webhook_payload()`.

**Step 4: Run tests**

Run:

```bash
cargo test -p backend model_ops_alert_delivery_message --offline
cargo test -p backend model_ops_alert_delivery_dry_run --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: map model ops alert deliveries"
```

### Task 3: Delivery Persistence and Scanner Source Contract

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing source-contract test**

Add:

```rust
#[test]
fn model_ops_alert_delivery_source_contract_scans_audits_and_records_delivery() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("pub async fn deliver_active_model_ops_alerts"));
    assert!(source.contains("FROM ai_model_ops_alert alert"));
    assert!(source.contains("NOT EXISTS"));
    assert!(source.contains("ai_model_ops_alert_delivery delivery"));
    assert!(source.contains("create_tool_call_audit"));
    assert!(source.contains("INSERT INTO ai_model_ops_alert_delivery"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend model_ops_alert_delivery_source_contract --offline
```

Expected: FAIL because scanner/persistence are not wired.

**Step 3: Implement scanner and persistence**

Add:

- `ModelOpsAlertDeliverySummary`
- `ModelOpsAlertDeliverySaveRecord`
- `ModelRuntimeService::deliver_active_model_ops_alerts(db: &PgPool) -> Result<ModelOpsAlertDeliverySummary, AppError>`
- `model_ops_alert_delivery_candidates`
- `deliver_model_ops_alert_candidate`
- `execute_model_ops_alert_feishu_delivery`
- `persist_model_ops_alert_delivery`
- `record_model_ops_alert_delivery_audit`

The scanner selects active unresolved alerts with no `sent` or `dry_run` Feishu delivery. The execution function sends Feishu only when webhook env is configured; otherwise it returns the dry-run result.

**Step 4: Run focused tests**

Run:

```bash
cargo test -p backend model_ops_alert_delivery --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: persist model ops alert deliveries"
```

### Task 4: Scheduler Builtin and Matrix

**Files:**
- Modify: `backend/src/application/scheduler/executor.rs`
- Modify: `backend/src/application/scheduler/service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Write failing scheduler tests**

In `executor.rs` tests add:

```rust
#[test]
fn model_alert_delivery_key_source_contract_routes_scheduler_builtin() {
    let source = include_str!("executor.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("ai.model.alert_delivery"));
    assert!(source.contains("ModelRuntimeService::deliver_active_model_ops_alerts"));
}
```

In `service.rs` tests add:

```rust
#[test]
fn model_alert_delivery_key_builtin_job_is_accepted() {
    let mut command = base_command();
    command.task_type = JOB_TYPE_BUILTIN;
    command.builtin_key = "ai.model.alert_delivery".to_owned();

    let command = normalize_job_command(command, &safety_config()).unwrap();

    assert_eq!(command.builtin_key, "ai.model.alert_delivery");
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p backend model_alert_delivery_key --offline
```

Expected: FAIL because executor has no dispatch arm.

**Step 3: Implement scheduler builtin**

Add executor arm:

```rust
"ai.model.alert_delivery" => {
    let summary = ModelRuntimeService::deliver_active_model_ops_alerts(db).await?;
    Ok(HttpOutput {
        status: Some(200),
        body: serde_json::to_string(&summary).unwrap_or_else(|_| "{}".to_owned()),
    })
}
```

Update matrix from `slice-15 implemented` to `slice-16 implemented`, noting scheduler-driven Feishu/dry-run alert delivery and durable delivery audit. Add the new plan link.

**Step 4: Run verification**

Run:

```bash
cargo fmt -- --check
cargo test -p backend model_ops_alert_delivery --offline
cargo test -p backend model_alert_delivery_key --offline
cargo test -p backend model_health_alert --offline
cargo test --workspace --offline
```

Expected: all pass.

**Step 5: Commit**

```bash
git add backend/src/application/scheduler/executor.rs backend/src/application/scheduler/service.rs docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-model-ops-alert-delivery.md
git commit -m "docs: record model ops alert delivery progress"
```

**Step 6: Merge to main**

After feature verification:

```bash
cd /path/to/Novex
git merge --no-ff feat/enterprise-agent-foundation -m "merge: enterprise agent foundation model ops alert delivery"
cargo fmt -- --check
cargo test --workspace --offline
cd /path/to/Novex/.worktrees/enterprise-agent-foundation
git merge --ff-only main
```
