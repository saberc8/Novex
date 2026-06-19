# Agent Route Breaker Controls Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add tenant-scoped model route circuit breaker list and clear controls to the enterprise control plane.

**Architecture:** Add an additive permission seed, service methods on `ModelRuntimeService`, and HTTP handlers under `/ai/models/route-circuit-breakers`. Keep DB as the breaker source of truth by checking persistent state before local cache during runtime execution.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL migrations, existing `ApiResponse`, existing model runtime service.

---

### Task 1: Commit Design And Plan

**Files:**
- Create: `docs/plans/2026-06-17-agent-route-breaker-controls-design.md`
- Create: `docs/plans/2026-06-17-agent-route-breaker-controls.md`

**Step 1: Commit docs**

Run:

```bash
git add docs/plans/2026-06-17-agent-route-breaker-controls-design.md docs/plans/2026-06-17-agent-route-breaker-controls.md
git commit -m "docs: plan route breaker controls"
```

### Task 2: Add Permission Seed

**Files:**
- Create: `backend/migrations/202606170002_seed_ai_model_circuit_breaker_permissions.sql`
- Modify: `backend/src/interfaces/http/ai/model.rs`

**Step 1: Write failing test**

Add constants:

```rust
pub const MODEL_CIRCUIT_BREAKER_LIST_PERMISSION: &str = "ai:model:circuitBreaker:list";
pub const MODEL_CIRCUIT_BREAKER_CLEAR_PERMISSION: &str = "ai:model:circuitBreaker:clear";
```

Add test:

```rust
#[test]
fn model_circuit_breaker_permission_seed_contains_controls() {
    let seed = include_str!(
        "../../../../migrations/202606170002_seed_ai_model_circuit_breaker_permissions.sql"
    );

    assert!(seed.contains(MODEL_CIRCUIT_BREAKER_LIST_PERMISSION));
    assert!(seed.contains(MODEL_CIRCUIT_BREAKER_CLEAR_PERMISSION));
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend model_circuit_breaker_permission_seed --offline
```

Expected: FAIL because the seed migration does not exist.

**Step 3: Add migration**

Create SQL:

```sql
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3024, '断路器列表', 3020, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:model:circuitBreaker:list', 4, 1, 1, NOW()),
    (3025, '清除断路器', 3020, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:model:circuitBreaker:clear', 5, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
VALUES
    (1, 3024),
    (1, 3025)
ON CONFLICT DO NOTHING;
```

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend model_circuit_breaker_permission_seed --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/migrations/202606170002_seed_ai_model_circuit_breaker_permissions.sql backend/src/interfaces/http/ai/model.rs
git commit -m "feat: seed model breaker control permissions"
```

### Task 3: Add Service List/Clear Methods

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add source-contract tests:

```rust
#[test]
fn route_breaker_controls_source_contract_lists_tenant_breakers() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("pub async fn list_route_circuit_breakers"));
    assert!(source.contains("FROM ai_model_route_circuit_breaker"));
    assert!(source.contains("WHERE tenant_id = $1"));
    assert!(source.contains("ORDER BY opened_until DESC"));
}

#[test]
fn route_breaker_controls_source_contract_clears_tenant_breaker_and_local_cache() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("pub async fn clear_route_circuit_breaker"));
    assert!(source.contains("DELETE FROM ai_model_route_circuit_breaker"));
    assert!(source.contains("WHERE tenant_id = $1"));
    assert!(source.contains("model_circuit_breaker_clear(route_id)"));
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend route_breaker_controls --offline
```

Expected: FAIL because service methods do not exist.

**Step 3: Implement service methods**

Add:

- `ModelRouteCircuitBreakerResp`
- `ModelRouteCircuitBreakerControlRow`
- `list_route_circuit_breakers`
- `clear_route_circuit_breaker`

Use `format_datetime` for timestamp output and compute `is_open`/`remaining_ms` from `Utc::now().naive_utc()`.

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend route_breaker_controls --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: add model route breaker control service"
```

### Task 4: Add HTTP Routes

**Files:**
- Modify: `backend/src/interfaces/http/ai/model.rs`

**Step 1: Write failing tests**

Add tests:

```rust
#[tokio::test]
async fn model_circuit_breaker_list_handler_rejects_missing_permission() {
    let err = list_route_circuit_breakers(State(test_state()), user_with_permissions(vec![]))
        .await
        .unwrap_err();

    assert!(matches!(err, AppError::Forbidden));
}

#[tokio::test]
async fn model_circuit_breaker_clear_handler_rejects_missing_permission() {
    let err = clear_route_circuit_breaker(
        State(test_state()),
        user_with_permissions(vec![]),
        axum::extract::Path("runtime.llm.code_agent".to_owned()),
    )
    .await
    .unwrap_err();

    assert!(matches!(err, AppError::Forbidden));
}

#[test]
fn model_circuit_breaker_routes_are_registered() {
    let source = include_str!("model.rs");

    assert!(source.contains("/ai/models/route-circuit-breakers"));
    assert!(source.contains("/ai/models/route-circuit-breakers/:route_id"));
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend model_circuit_breaker_ --offline
```

Expected: FAIL because handlers/routes are missing.

**Step 3: Implement handlers/routes**

Add:

- `GET /ai/models/route-circuit-breakers`
- `DELETE /ai/models/route-circuit-breakers/:route_id`

Both bind `ModelRuntimeService::for_tenant(state.db, current_user.tenant_id)`.

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend model_circuit_breaker_ --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/interfaces/http/ai/model.rs
git commit -m "feat: expose model route breaker controls"
```

### Task 5: Runtime Source-Of-Truth Ordering

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing source-contract test**

Add:

```rust
#[test]
fn route_breaker_controls_source_contract_checks_persistent_before_local_cache() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    let persistent = source
        .find("self.persistent_model_circuit_breaker_open_attempt(&current_route)")
        .unwrap();
    let local = source
        .find("model_circuit_breaker_open_attempt(&current_route)")
        .unwrap();

    assert!(persistent < local);
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend checks_persistent_before_local_cache --offline
```

Expected: FAIL if runtime still checks local first.

**Step 3: Reorder runtime check**

Check persistent breaker first, then local cache. This makes manual DB clear authoritative across processes.

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend checks_persistent_before_local_cache --offline
cargo test -p backend route_circuit_breaker --offline
cargo test -p backend persistent_route_circuit_breaker --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: prioritize persistent breaker state"
```

### Task 6: Matrix, Verification, Merge

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update matrix**

Change rollout trace status from `slice-10 implemented` to `slice-11 implemented`. Update notes to include breaker list/clear controls; leave dashboards/metrics next.

Add focused command:

```bash
cargo test -p backend route_breaker_controls --offline
```

**Step 2: Commit matrix**

Run:

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: record breaker controls progress"
```

**Step 3: Verify and merge**

Run:

```bash
cargo fmt -- --check
cargo test -p backend model_circuit_breaker_ --offline
cargo test -p backend route_breaker_controls --offline
cargo test -p backend route_circuit_breaker --offline
cargo test -p backend persistent_route_circuit_breaker --offline
cargo test --workspace --offline
```

Then merge feature worktree to local `main`, rerun `cargo fmt -- --check` and `cargo test --workspace --offline` on main, and fast-forward the feature worktree.
