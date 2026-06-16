# Agent Model Ops Summary Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a tenant-scoped model operations summary API for route health, usage, cost, latency, and circuit-breaker state.

**Architecture:** Add an additive permission seed, route-level ops DTOs and aggregation on `ModelRuntimeService`, and an HTTP handler under `/ai/models/ops-summary`. Keep the slice dashboard-ready but UI-neutral by returning normalized route rows plus top-level counts.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL migrations, existing `ApiResponse`, existing model runtime service.

---

### Task 1: Commit Design And Plan

**Files:**
- Create: `docs/plans/2026-06-17-agent-model-ops-summary-design.md`
- Create: `docs/plans/2026-06-17-agent-model-ops-summary.md`

**Step 1: Commit docs**

Run:

```bash
git add docs/plans/2026-06-17-agent-model-ops-summary-design.md docs/plans/2026-06-17-agent-model-ops-summary.md
git commit -m "docs: plan model ops summary"
```

### Task 2: Add Ops Summary Permission Seed

**Files:**
- Create: `backend/migrations/202606170003_seed_ai_model_ops_summary_permission.sql`
- Modify: `backend/src/interfaces/http/ai/model.rs`

**Step 1: Write failing test**

Add constant:

```rust
pub const MODEL_OPS_SUMMARY_PERMISSION: &str = "ai:model:opsSummary";
```

Add test:

```rust
#[test]
fn model_ops_summary_permission_seed_contains_control() {
    let seed =
        include_str!("../../../../migrations/202606170003_seed_ai_model_ops_summary_permission.sql");

    assert!(seed.contains(MODEL_OPS_SUMMARY_PERMISSION));
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend-rust model_ops_summary_permission_seed --offline
```

Expected: FAIL because the migration file does not exist.

**Step 3: Add migration**

Create SQL:

```sql
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3026, '模型运营摘要', 3020, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:model:opsSummary', 6, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
VALUES
    (1, 3026)
ON CONFLICT DO NOTHING;
```

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend-rust model_ops_summary_permission_seed --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/migrations/202606170003_seed_ai_model_ops_summary_permission.sql backend/src/interfaces/http/ai/model.rs
git commit -m "feat: seed model ops summary permission"
```

### Task 3: Add Model Ops Summary Service

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add source-contract test:

```rust
#[test]
fn model_ops_summary_source_contract_reads_route_health_usage_and_breakers() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("pub async fn model_ops_summary"));
    assert!(source.contains("FROM ai_model_route r"));
    assert!(source.contains("ai_model_route_circuit_breaker"));
    assert!(source.contains("ai_model_health_check"));
    assert!(source.contains("ai_model_usage"));
    assert!(source.contains("WHERE r.tenant_id = $1"));
    assert!(source.contains("INTERVAL '24 hours'"));
}
```

Add pure aggregation test:

```rust
#[test]
fn model_ops_summary_from_rows_counts_open_breakers_and_degraded_routes() {
    let now = NaiveDateTime::parse_from_str("2026-06-17 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let summary = model_ops_summary_from_rows(
        vec![
            ModelRouteOpsSummaryRow {
                route_code: "runtime.llm.chat".to_owned(),
                route_purpose: "chat".to_owned(),
                provider_code: "deepseek".to_owned(),
                provider_type: "deep-seek".to_owned(),
                model_name: "deepseek-v4".to_owned(),
                network_zone: "public".to_owned(),
                status: 1,
                breaker_opened_until: Some(now + chrono::Duration::minutes(5)),
                last_health_status: Some("ok".to_owned()),
                last_health_checked_at: Some(now),
                last_health_latency_ms: Some(120),
                request_count_24h: 3,
                total_tokens_24h: 1200,
                cost_cents_24h: 1.5,
                avg_latency_ms_24h: Some(330.0),
            },
            ModelRouteOpsSummaryRow {
                route_code: "runtime.embedding".to_owned(),
                route_purpose: "embedding".to_owned(),
                provider_code: "dashscope".to_owned(),
                provider_type: "dash-scope".to_owned(),
                model_name: "text-embedding-v4".to_owned(),
                network_zone: "public".to_owned(),
                status: 1,
                breaker_opened_until: None,
                last_health_status: Some("provider returned HTTP 500".to_owned()),
                last_health_checked_at: Some(now),
                last_health_latency_ms: Some(800),
                request_count_24h: 2,
                total_tokens_24h: 500,
                cost_cents_24h: 0.25,
                avg_latency_ms_24h: Some(90.0),
            },
        ],
        now,
    );

    assert_eq!(summary.route_count, 2);
    assert_eq!(summary.active_route_count, 2);
    assert_eq!(summary.open_breaker_count, 1);
    assert_eq!(summary.degraded_route_count, 2);
    assert_eq!(summary.usage_24h.request_count, 5);
    assert_eq!(summary.usage_24h.total_tokens, 1700);
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend-rust model_ops_summary --offline
```

Expected: FAIL because service types/functions do not exist.

**Step 3: Implement DTOs, row struct, query, and builder**

Add:

- `ModelOpsSummaryResp`
- `ModelRouteOpsSummaryResp`
- `ModelOpsUsageSummaryResp`
- `ModelRouteOpsSummaryRow`
- `ModelRuntimeService::model_ops_summary`
- `model_ops_summary_from_rows`

Use a single tenant-scoped SQL query joining route metadata, current breaker rows, latest health row, and 24-hour usage aggregates.

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend-rust model_ops_summary --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: add model ops summary service"
```

### Task 4: Add HTTP Route

**Files:**
- Modify: `backend/src/interfaces/http/ai/model.rs`

**Step 1: Write failing tests**

Add tests:

```rust
#[tokio::test]
async fn model_ops_summary_handler_rejects_missing_permission() {
    let err = model_ops_summary(State(test_state()), user_with_permissions(vec![]))
        .await
        .unwrap_err();

    assert!(matches!(err, AppError::Forbidden));
}

#[test]
fn model_ops_summary_route_is_registered() {
    let source = include_str!("model.rs");

    assert!(source.contains("/ai/models/ops-summary"));
    assert!(source.contains("MODEL_OPS_SUMMARY_PERMISSION"));
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend-rust model_ops_summary --offline
```

Expected: FAIL because the handler/route is missing.

**Step 3: Implement handler and route**

Import `ModelOpsSummaryResp`, add:

```rust
.route("/ai/models/ops-summary", get(model_ops_summary))
```

Handler:

```rust
async fn model_ops_summary(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<ModelOpsSummaryResp>>, AppError> {
    require_permission(&current_user, MODEL_OPS_SUMMARY_PERMISSION)?;
    let service = ModelRuntimeService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.model_ops_summary().await?)))
}
```

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend-rust model_ops_summary --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/interfaces/http/ai/model.rs
git commit -m "feat: expose model ops summary"
```

### Task 5: Matrix, Verification, Merge

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update matrix**

Change rollout trace status from `slice-11 implemented` to `slice-12 implemented`. Update notes to include model ops summary route/provider/usage/health/breaker aggregation, and leave frontend dashboard, alerting, and scheduler-driven health persistence next.

Add focused command:

```bash
cargo test -p backend-rust model_ops_summary --offline
```

**Step 2: Commit matrix**

Run:

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: record model ops summary progress"
```

**Step 3: Verify and merge**

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust model_ops_summary --offline
cargo test -p backend-rust model_circuit_breaker_ --offline
cargo test -p backend-rust route_breaker_controls --offline
cargo test --workspace --offline
```

Then merge feature worktree to local `main`, rerun `cargo fmt -- --check` and `cargo test --workspace --offline` on main, and fast-forward the feature worktree.
