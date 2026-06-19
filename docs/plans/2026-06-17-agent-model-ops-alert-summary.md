# Agent Model Ops Alert Summary Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Expose active model ops alerts in the existing model ops summary response.

**Architecture:** Add an alert row DTO and response DTO inside `ModelRuntimeService`. Fetch active `ai_model_ops_alert` rows alongside existing route ops rows, then let the pure summary builder compute top-level alert details, top-level alert count, per-route active alert counts, and route degradation.

**Tech Stack:** Rust, SQLx, Serde, existing Axum model ops endpoint, PostgreSQL `ai_model_ops_alert`.

---

### Task 1: Response Contract and Pure Summary Tests

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write the failing tests**

Add tests in the existing `model_service.rs` test module:

```rust
#[test]
fn model_ops_summary_includes_active_alerts_and_route_counts() {
    let now = NaiveDateTime::parse_from_str("2026-06-17 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let summary = model_ops_summary_from_rows(
        vec![model_ops_route_row("runtime.llm.chat", "chat", 1, None, Some("ok"))],
        vec![model_ops_alert_row(
            "model_health:llm:route:11",
            Some("runtime.llm.chat"),
            Some("chat"),
            "provider unavailable",
            now,
        )],
        now,
    );

    assert_eq!(summary.active_alert_count, 1);
    assert_eq!(summary.alerts.len(), 1);
    assert_eq!(summary.alerts[0].alert_key, "model_health:llm:route:11");
    assert_eq!(summary.alerts[0].route_id.as_deref(), Some("runtime.llm.chat"));
    assert_eq!(summary.alerts[0].message, "provider unavailable");
    assert_eq!(summary.routes[0].active_alert_count, 1);
}

#[test]
fn model_ops_summary_marks_route_degraded_when_active_alert_exists() {
    let now = NaiveDateTime::parse_from_str("2026-06-17 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let summary = model_ops_summary_from_rows(
        vec![model_ops_route_row("runtime.llm.chat", "chat", 1, None, Some("ok"))],
        vec![model_ops_alert_row(
            "model_health:llm:route:11",
            Some("runtime.llm.chat"),
            Some("chat"),
            "provider unavailable",
            now,
        )],
        now,
    );

    assert!(summary.routes[0].degraded);
    assert_eq!(summary.degraded_route_count, 1);
}
```

Add helper builders near the existing ops summary tests:

```rust
fn model_ops_route_row(
    route_code: &str,
    route_purpose: &str,
    status: i16,
    breaker_opened_until: Option<NaiveDateTime>,
    last_health_status: Option<&str>,
) -> ModelRouteOpsSummaryRow {
    ModelRouteOpsSummaryRow {
        route_code: route_code.to_owned(),
        route_purpose: route_purpose.to_owned(),
        provider_code: "deepseek".to_owned(),
        provider_type: "deep-seek".to_owned(),
        model_name: "deepseek-v4-flash".to_owned(),
        network_zone: "public".to_owned(),
        status,
        breaker_opened_until,
        last_health_status: last_health_status.map(str::to_owned),
        last_health_checked_at: None,
        last_health_latency_ms: None,
        request_count_24h: 0,
        total_tokens_24h: 0,
        cost_cents_24h: 0.0,
        avg_latency_ms_24h: None,
    }
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend model_ops_summary_includes_active_alerts --offline
```

Expected: FAIL because the summary function does not accept alert rows and response fields do not exist.

**Step 3: Implement minimal response and pure builder support**

Add:

- `ModelOpsAlertResp`
- `ModelOpsAlertRow`
- `active_alert_count` to `ModelOpsSummaryResp`
- `alerts` to `ModelOpsSummaryResp`
- `active_alert_count` to `ModelRouteOpsSummaryResp`
- Update `model_ops_summary_from_rows(rows, alert_rows, now)`

The alert response message should come from `event_payload["message"]` when it is a string, otherwise default to an empty string.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p backend model_ops_summary_includes_active_alerts --offline
cargo test -p backend model_ops_summary_marks_route_degraded --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: expose model ops alert summary shape"
```

### Task 2: SQL Query Integration

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write the failing source-contract test**

Add:

```rust
#[test]
fn model_ops_summary_source_contract_reads_active_model_ops_alerts() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("ai_model_ops_alert alert"));
    assert!(source.contains("alert.resolved_at IS NULL"));
    assert!(source.contains("ORDER BY alert.last_seen_at DESC"));
    assert!(source.contains("model_ops_summary_from_rows(rows, alert_rows"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend model_ops_summary_source_contract_reads_active_model_ops_alerts --offline
```

Expected: FAIL because the SQL query and call site are not wired.

**Step 3: Implement SQL integration**

In `model_ops_summary`:

1. Keep existing route query.
2. Add a second `sqlx::query_as::<_, ModelOpsAlertRow>` selecting active alerts for the tenant.
3. Left join `ai_model_route`, `ai_model_profile`, `ai_model_deployment`, and `ai_model_provider`.
4. Order by `alert.last_seen_at DESC, alert.id DESC`.
5. Pass `rows, alert_rows, now` to the pure builder.

**Step 4: Run focused tests**

Run:

```bash
cargo test -p backend model_ops_summary --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: read active model ops alerts"
```

### Task 3: Migration Matrix and Full Verification

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-model-ops-alert-summary.md`

**Step 1: Update migration matrix**

Change `slice-14 implemented` to `slice-15 implemented`. Update notes to include ops summary active alert exposure and narrow remaining work to frontend dashboard rendering and external alert delivery.

Add `cargo test -p backend model_ops_summary --offline` remains the focused acceptance command for this slice.

**Step 2: Run verification**

Run:

```bash
cargo fmt -- --check
cargo test -p backend model_ops_summary --offline
cargo test -p backend model_health_alert --offline
cargo test --workspace --offline
```

Expected: all pass.

**Step 3: Commit**

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-model-ops-alert-summary.md
git commit -m "docs: record model ops alert summary progress"
```

**Step 4: Merge to main**

After feature verification:

```bash
cd /path/to/Novex
git merge --no-ff feat/enterprise-agent-foundation -m "merge: enterprise agent foundation model ops alert summary"
cargo fmt -- --check
cargo test --workspace --offline
cd /path/to/Novex/.worktrees/enterprise-agent-foundation
git merge --ff-only main
```
