# Agent Model Health Persistence Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Persist model health-check results and add a scheduler builtin that refreshes model health state for active tenants.

**Architecture:** Extend `ModelRuntimeService` with health-check persistence helpers and an all-active-tenants refresh method. Wire the existing HTTP health-check path through persistence, then add scheduler builtin key `ai.model.health_check` that calls the same service path.

**Tech Stack:** Rust, SQLx, Axum service layer, scheduler builtin executor, existing `ai_model_health_check` table.

---

### Task 1: Commit Design And Plan

**Files:**
- Create: `docs/plans/2026-06-17-agent-model-health-persistence-design.md`
- Create: `docs/plans/2026-06-17-agent-model-health-persistence.md`

**Step 1: Commit docs**

Run:

```bash
git add docs/plans/2026-06-17-agent-model-health-persistence-design.md docs/plans/2026-06-17-agent-model-health-persistence.md
git commit -m "docs: plan model health persistence"
```

### Task 2: Add Health Persistence Service

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add source-contract test:

```rust
#[test]
fn model_health_persistence_source_contract_records_tenant_health_rows() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("persist_model_health_check_results"));
    assert!(source.contains("INSERT INTO ai_model_health_check"));
    assert!(source.contains("WHERE r.tenant_id = $1"));
    assert!(source.contains("default_purpose_for_target(result.target)"));
    assert!(source.contains("health_check_for_tenant"));
}
```

Add pure record builder test:

```rust
#[test]
fn model_health_check_record_from_result_maps_status_and_metadata() {
    let now = NaiveDateTime::parse_from_str("2026-06-17 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let result = ModelHealthCheckResult {
        target: ModelRuntimeTarget::Llm,
        configured: true,
        ok: false,
        endpoint: Some("https://llm.example.com/v1/chat/completions".to_owned()),
        masked_api_key: Some("sk-****0001".to_owned()),
        http_status: Some(502),
        latency_ms: 123,
        message: "provider returned HTTP 502".to_owned(),
        detail: Some(json!({"choiceCount": 0})),
    };

    let record = model_health_check_record_from_result(1, 7, Some((11, 22, 33)), &result, now);

    assert_eq!(record.status, "provider returned HTTP 502");
    assert_eq!(record.http_status, Some(502));
    assert_eq!(record.latency_ms, Some(123));
    assert_eq!(record.detail["target"], "llm");
    assert_eq!(record.route_id, Some(11));
    assert_eq!(record.provider_id, Some(22));
    assert_eq!(record.model_profile_id, Some(33));
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend model_health_persistence --offline
```

Expected: FAIL because helper types/functions do not exist.

**Step 3: Implement minimal service persistence**

Add:

- `ModelHealthCheckRouteIdsRow`
- `ModelHealthCheckSaveRecord`
- `model_health_check_record_from_result`
- `persist_model_health_check_results`

Modify `health_check_for_tenant` to persist the finished result list before returning.

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend model_health_persistence --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: persist model health checks"
```

### Task 3: Add All-Tenant Health Refresh

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add source-contract test:

```rust
#[test]
fn model_health_persistence_source_contract_refreshes_active_tenants() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("pub async fn refresh_active_tenant_model_health"));
    assert!(source.contains("SELECT DISTINCT tenant_id"));
    assert!(source.contains("FROM ai_model_route"));
    assert!(source.contains("WHERE status = 1"));
    assert!(source.contains("health_check_for_tenant(ModelHealthCheckCommand"));
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend refresh_active_tenant_model_health --offline
```

Expected: FAIL because method is missing.

**Step 3: Implement method**

Add:

```rust
pub async fn refresh_active_tenant_model_health(db: &PgPool) -> Result<usize, AppError>
```

It selects distinct tenant IDs from active `ai_model_route`, runs `health_check_for_tenant(ModelHealthCheckCommand { target: Some("all".to_owned()) })`, and returns the number of persisted result rows.

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend refresh_active_tenant_model_health --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: refresh active tenant model health"
```

### Task 4: Add Scheduler Builtin

**Files:**
- Modify: `backend/src/application/scheduler/executor.rs`
- Modify: `backend/src/application/scheduler/service.rs`

**Step 1: Write failing tests**

In executor tests add:

```rust
#[test]
fn scheduler_builtin_source_contract_routes_model_health_check() {
    let source = include_str!("executor.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("ai.model.health_check"));
    assert!(source.contains("ModelRuntimeService::refresh_active_tenant_model_health"));
    assert!(source.contains("execute_builtin_job(&repo.db()"));
}
```

In scheduler service tests add:

```rust
#[test]
fn normalize_builtin_job_accepts_model_health_check_key() {
    let mut command = base_command();
    command.task_type = JOB_TYPE_BUILTIN;
    command.builtin_key = "ai.model.health_check".to_owned();

    let command = normalize_job_command(command, &safety_config()).unwrap();

    assert_eq!(command.builtin_key, "ai.model.health_check");
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend model_health_check_key --offline
```

Expected: FAIL because executor does not support the builtin.

**Step 3: Implement scheduler builtin**

Change executor builtin dispatch to pass the DB pool into `execute_builtin_job`.

Add match arm:

```rust
"ai.model.health_check" => {
    let rows = ModelRuntimeService::refresh_active_tenant_model_health(db).await?;
    Ok(HttpOutput {
        status: Some(200),
        body: json!({"status": "ok", "healthRows": rows}).to_string(),
    })
}
```

Expose a `db()` accessor on `SchedulerRepository` if needed, or pass the cloned `PgPool` directly before constructing the repository.

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend model_health_check_key --offline
cargo test -p backend scheduler_builtin --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/application/scheduler/executor.rs backend/src/application/scheduler/service.rs
git commit -m "feat: add model health scheduler builtin"
```

### Task 5: Matrix, Verification, Merge

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update matrix**

Change rollout trace status from `slice-12 implemented` to `slice-13 implemented`. Update notes to include persisted model health checks and scheduler builtin health refresh. Leave alerting, dashboards, and seeded default job next.

Add focused command:

```bash
cargo test -p backend model_health_persistence --offline
```

**Step 2: Commit matrix**

Run:

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: record model health persistence progress"
```

**Step 3: Verify and merge**

Run:

```bash
cargo fmt -- --check
cargo test -p backend model_health_persistence --offline
cargo test -p backend model_ops_summary --offline
cargo test -p backend model_health_check_key --offline
cargo test --workspace --offline
```

Then merge feature worktree to local `main`, rerun `cargo fmt -- --check` and `cargo test --workspace --offline` on main, and fast-forward the feature worktree.
