# Agent Model Health Automation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Seed automatic model health checks and persist active model ops alerts for failed health probes.

**Architecture:** Add one migration that creates `ai_model_ops_alert` and seeds a default `ai.model.health_check` builtin scheduler job. Extend `ModelRuntimeService` so health-check persistence upserts an active alert for failed probes and resolves that alert when the next probe succeeds.

**Tech Stack:** Rust, SQLx, PostgreSQL migrations, scheduler builtin jobs, existing model runtime service tests.

---

### Task 1: Migration Contract and Default Job Seed

**Files:**
- Create: `backend/migrations/202606170004_create_ai_model_ops_alert.sql`
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write the failing test**

Add this source-contract test in `model_service.rs` tests:

```rust
#[test]
fn model_health_automation_migration_defines_alert_table_and_seed_job() {
    let migration = include_str!("../../../migrations/202606170004_create_ai_model_ops_alert.sql");

    assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_model_ops_alert"));
    assert!(migration.contains("uk_ai_model_ops_alert_active_key"));
    assert!(migration.contains("WHERE resolved_at IS NULL"));
    assert!(migration.contains("INSERT INTO sys_job"));
    assert!(migration.contains("'ai.model.health_check'"));
    assert!(migration.contains("'*/5 * * * * *'"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend-rust model_health_automation_migration --offline
```

Expected: FAIL because the migration file does not exist or does not contain the required schema/job seed.

**Step 3: Write minimal migration**

Create `backend/migrations/202606170004_create_ai_model_ops_alert.sql` with:

```sql
CREATE TABLE IF NOT EXISTS ai_model_ops_alert (
    id BIGINT NOT NULL,
    tenant_id BIGINT NOT NULL DEFAULT 1,
    alert_key VARCHAR(160) NOT NULL,
    alert_kind VARCHAR(64) NOT NULL,
    severity VARCHAR(32) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    route_id BIGINT DEFAULT NULL,
    provider_id BIGINT DEFAULT NULL,
    model_profile_id BIGINT DEFAULT NULL,
    source_ref VARCHAR(128) DEFAULT NULL,
    event_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    first_seen_at TIMESTAMP NOT NULL,
    last_seen_at TIMESTAMP NOT NULL,
    resolved_at TIMESTAMP DEFAULT NULL,
    resolve_message TEXT DEFAULT NULL,
    create_user BIGINT NOT NULL,
    create_time TIMESTAMP NOT NULL,
    update_user BIGINT DEFAULT NULL,
    update_time TIMESTAMP DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_model_ops_alert_tenant_status
    ON ai_model_ops_alert (tenant_id, status, last_seen_at DESC);
CREATE INDEX IF NOT EXISTS idx_ai_model_ops_alert_route_id
    ON ai_model_ops_alert (route_id);
CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_model_ops_alert_active_key
    ON ai_model_ops_alert (tenant_id, alert_key)
    WHERE resolved_at IS NULL;

INSERT INTO sys_job (
    id, name, group_name, task_type, cron_expression, status, concurrent,
    misfire_policy, max_retry, timeout_seconds, http_method, http_url,
    http_headers, http_body, builtin_key, description, next_trigger_time,
    create_user, create_time
) VALUES (
    3600001, 'AI Model Health Check', 'ai-ops', 2, '*/5 * * * * *', 1, FALSE,
    1, 1, 120, NULL, NULL,
    '{}'::jsonb, NULL, 'ai.model.health_check',
    'Refresh persisted model health rows and active model ops alerts.',
    NOW(), 1, NOW()
)
ON CONFLICT DO NOTHING;
```

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p backend-rust model_health_automation_migration --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/migrations/202606170004_create_ai_model_ops_alert.sql backend/src/application/ai/model_service.rs
git commit -m "feat: seed model health automation"
```

### Task 2: Alert Mapping Pure Helpers

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write the failing tests**

Add tests:

```rust
#[test]
fn model_health_alert_record_from_failure_uses_stable_key_and_payload() {
    let now = chrono::NaiveDate::from_ymd_opt(2026, 6, 17)
        .unwrap()
        .and_hms_opt(10, 0, 0)
        .unwrap();
    let result = ModelHealthCheckResult {
        target: ModelRuntimeTarget::Chat,
        configured: true,
        ok: false,
        endpoint: Some("https://api.example.test".to_owned()),
        masked_api_key: Some("sk-***1234".to_owned()),
        http_status: Some(503),
        latency_ms: 123,
        message: "provider unavailable".to_owned(),
        detail: Some(json!({"provider":"example"})),
    };

    let record = model_ops_alert_record_from_health_check(
        1,
        7,
        Some((11, 22, 33)),
        &result,
        99,
        now,
    );

    assert_eq!(record.tenant_id, 1);
    assert_eq!(record.alert_key, "model_health:chat:route:11");
    assert_eq!(record.alert_kind, "model_health");
    assert_eq!(record.severity, "critical");
    assert_eq!(record.status, "active");
    assert_eq!(record.route_id, Some(11));
    assert_eq!(record.provider_id, Some(22));
    assert_eq!(record.model_profile_id, Some(33));
    assert_eq!(record.source_ref, "health_check:99");
    assert_eq!(record.event_payload["message"], "provider unavailable");
    assert_eq!(record.event_payload["maskedApiKey"], "sk-***1234");
}

#[test]
fn model_health_alert_key_uses_target_when_route_is_missing() {
    let result = ModelHealthCheckResult {
        target: ModelRuntimeTarget::Embedding,
        configured: false,
        ok: false,
        endpoint: None,
        masked_api_key: None,
        http_status: None,
        latency_ms: 0,
        message: "missing route".to_owned(),
        detail: None,
    };

    assert_eq!(
        model_ops_alert_key_from_health_check(1, None, &result),
        "model_health:embedding:tenant:1"
    );
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p backend-rust model_health_alert --offline
```

Expected: FAIL because the helper types and functions do not exist.

**Step 3: Implement minimal helpers**

Add:
- `ModelOpsAlertSaveRecord`
- `model_ops_alert_key_from_health_check`
- `model_ops_alert_record_from_health_check`

Keep these helpers pure and near existing model-health persistence helpers.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p backend-rust model_health_alert --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: map model health alerts"
```

### Task 3: Persist and Resolve Active Alerts

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write the failing source-contract test**

Add:

```rust
#[test]
fn model_health_alert_persistence_source_contract_upserts_and_resolves_active_alerts() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("record_model_ops_alert_for_health_check"));
    assert!(source.contains("upsert_model_ops_alert"));
    assert!(source.contains("resolve_model_ops_alert"));
    assert!(source.contains("ON CONFLICT (tenant_id, alert_key) WHERE resolved_at IS NULL"));
    assert!(source.contains("resolved_at = $4"));
    assert!(source.contains("persist_model_health_check_record(&self.db, &record).await?"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend-rust model_health_alert_persistence --offline
```

Expected: FAIL because alert persistence is not wired.

**Step 3: Implement persistence**

After each health row insert, call `record_model_ops_alert_for_health_check`. It should:

- upsert active alert for failed checks
- resolve the active alert for successful checks
- use the same route ids and saved health-check record id

Add DB helpers:

- `upsert_model_ops_alert`
- `resolve_model_ops_alert`

**Step 4: Run focused tests**

Run:

```bash
cargo test -p backend-rust model_health_alert --offline
cargo test -p backend-rust model_health_persistence --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: persist model health alerts"
```

### Task 4: Migration Matrix and Verification

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update matrix**

Change rollout trace status from `slice-13 implemented` to `slice-14 implemented`. Update notes to mention seeded model health scheduler job and active model ops alerts. Add focused commands:

```bash
cargo test -p backend-rust model_health_automation --offline
cargo test -p backend-rust model_health_alert --offline
```

**Step 2: Run verification**

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust model_health_automation --offline
cargo test -p backend-rust model_health_alert --offline
cargo test -p backend-rust model_health_persistence --offline
cargo test -p backend-rust model_health_check_key --offline
cargo test --workspace --offline
```

Expected: all pass.

**Step 3: Commit**

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-model-health-automation.md
git commit -m "docs: record model health automation progress"
```

**Step 4: Merge to main**

After feature verification:

```bash
cd /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex
git merge --no-ff feat/enterprise-agent-foundation -m "merge: enterprise agent foundation model health automation"
cargo fmt -- --check
cargo test --workspace --offline
cd /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex/.worktrees/enterprise-agent-foundation
git merge --ff-only main
```
