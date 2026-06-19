# Agent Persistent Route Circuit Breaker Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Persist model route circuit breaker state so multiple backend instances share route cooldown decisions.

**Architecture:** Add an additive SQL migration for `ai_model_route_circuit_breaker`. Extend `ModelRuntimeService` with DB-backed open/read helpers, keep the existing process-local breaker as a fast path, and wire both helpers into the existing fallback chain.

**Tech Stack:** Rust, SQLx, PostgreSQL migrations, `backend`, `chrono`, existing `ModelProviderAttempt`.

---

### Task 1: Commit Design And Plan

**Files:**
- Create: `docs/plans/2026-06-17-agent-persistent-route-circuit-breaker-design.md`
- Create: `docs/plans/2026-06-17-agent-persistent-route-circuit-breaker.md`

**Step 1: Review docs**

Run:

```bash
git diff -- docs/plans/2026-06-17-agent-persistent-route-circuit-breaker-design.md docs/plans/2026-06-17-agent-persistent-route-circuit-breaker.md
```

Expected: docs describe the new runtime table, DB-backed open/read helpers, and verification commands.

**Step 2: Commit**

Run:

```bash
git add docs/plans/2026-06-17-agent-persistent-route-circuit-breaker-design.md docs/plans/2026-06-17-agent-persistent-route-circuit-breaker.md
git commit -m "docs: plan persistent route circuit breaker"
```

### Task 2: Add Persistent Breaker Migration

**Files:**
- Create: `backend/migrations/202606170001_create_ai_model_route_circuit_breaker.sql`
- Modify: `backend/src/interfaces/http/ai/model.rs`

**Step 1: Write failing migration test**

Add:

```rust
#[test]
fn model_route_circuit_breaker_migration_defines_runtime_state_table() {
    let migration = include_str!(
        "../../../../migrations/202606170001_create_ai_model_route_circuit_breaker.sql"
    );

    for required in [
        "CREATE TABLE IF NOT EXISTS ai_model_route_circuit_breaker",
        "tenant_id",
        "route_id",
        "opened_until",
        "open_reason",
        "last_error_kind",
        "last_http_status",
        "uk_ai_model_route_circuit_breaker_tenant_route",
        "idx_ai_model_route_circuit_breaker_opened_until",
    ] {
        assert!(migration.contains(required), "missing {required}");
    }
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend model_route_circuit_breaker_migration --offline
```

Expected: FAIL because the migration file does not exist.

**Step 3: Add migration**

Create SQL:

```sql
CREATE TABLE IF NOT EXISTS ai_model_route_circuit_breaker (
    id               BIGINT       NOT NULL,
    tenant_id        BIGINT       NOT NULL DEFAULT 1,
    route_id         VARCHAR(128) NOT NULL,
    opened_until     TIMESTAMP    NOT NULL,
    open_reason      VARCHAR(64)  NOT NULL DEFAULT 'provider_failure',
    last_error_kind  VARCHAR(64)  DEFAULT NULL,
    last_http_status INTEGER      DEFAULT NULL,
    create_user      BIGINT       NOT NULL,
    create_time      TIMESTAMP    NOT NULL,
    update_user      BIGINT       DEFAULT NULL,
    update_time      TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_model_route_circuit_breaker_tenant_route
    ON ai_model_route_circuit_breaker (tenant_id, route_id);

CREATE INDEX IF NOT EXISTS idx_ai_model_route_circuit_breaker_opened_until
    ON ai_model_route_circuit_breaker (opened_until);
```

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend model_route_circuit_breaker_migration --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/migrations/202606170001_create_ai_model_route_circuit_breaker.sql backend/src/interfaces/http/ai/model.rs
git commit -m "feat: add model route circuit breaker migration"
```

### Task 3: Add Runtime DB Helpers

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing source-contract tests**

Add tests:

```rust
#[test]
fn persistent_route_circuit_breaker_source_contract_opens_runtime_state() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("async fn persistent_model_circuit_breaker_open"));
    assert!(source.contains("INSERT INTO ai_model_route_circuit_breaker"));
    assert!(source.contains("ON CONFLICT (tenant_id, route_id) DO UPDATE"));
}

#[test]
fn persistent_route_circuit_breaker_source_contract_reads_runtime_state() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("async fn persistent_model_circuit_breaker_open_attempt"));
    assert!(source.contains("FROM ai_model_route_circuit_breaker"));
    assert!(source.contains("opened_until > NOW()"));
    assert!(source.contains("model_provider_attempt_circuit_open"));
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend persistent_route_circuit_breaker_source --offline
```

Expected: FAIL because the helper methods do not exist.

**Step 3: Implement helpers**

Add row struct:

```rust
#[derive(Debug, FromRow)]
struct ModelRouteCircuitBreakerRow {
    opened_until: NaiveDateTime,
}
```

Add methods on `ModelRuntimeService`:

- `persistent_model_circuit_breaker_open(&self, route_id, cooldown_seconds, attempt)`
- `persistent_model_circuit_breaker_open_attempt(&self, route)`

Use `next_id()` for `id`, `self.tenant_id` for tenant isolation, and `DEFAULT_TENANT_ID` as the system user for create/update audit fields.

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend persistent_route_circuit_breaker_source --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: persist model route circuit breakers"
```

### Task 4: Wire Persistent Breaker Into Fallback Chain

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing source-contract test**

Add:

```rust
#[test]
fn persistent_route_circuit_breaker_source_contract_wires_runtime_chain() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains(".persistent_model_circuit_breaker_open_attempt(&current_route)"));
    assert!(source.contains(".persistent_model_circuit_breaker_open("));
    assert!(source.contains("model_circuit_breaker_open(current_route.route_id(), cooldown_seconds)"));
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend persistent_route_circuit_breaker_source_contract_wires_runtime_chain --offline
```

Expected: FAIL because the route chain still uses only process-local helpers.

**Step 3: Wire runtime**

Before route execution, check:

1. `model_circuit_breaker_open_attempt(&current_route)`
2. `self.persistent_model_circuit_breaker_open_attempt(&current_route).await?`

After fallback-eligible failure, call both:

1. `model_circuit_breaker_open(current_route.route_id(), cooldown_seconds)`
2. `self.persistent_model_circuit_breaker_open(current_route.route_id(), cooldown_seconds, failed_attempt).await?`

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend persistent_route_circuit_breaker_source --offline
cargo test -p backend route_circuit_breaker --offline
cargo test -p backend multi_hop_fallback --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: wire persistent route circuit breakers"
```

### Task 5: Update Matrix And Verify

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update matrix**

Change rollout trace status from `slice-9 implemented` to `slice-10 implemented`. Update notes to include `persistent cross-process route circuit breaker state`; leave manual breaker controls and operational dashboards as future work.

Add focused command:

```bash
cargo test -p backend persistent_route_circuit_breaker --offline
```

Add this implementation plan under follow-ups.

**Step 2: Commit**

Run:

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: record persistent breaker progress"
```

**Step 3: Final verification**

Run:

```bash
cargo fmt -- --check
cargo test -p backend persistent_route_circuit_breaker --offline
cargo test -p backend route_circuit_breaker --offline
cargo test -p backend multi_hop_fallback --offline
cargo test -p backend provider_lifecycle --offline
cargo test -p backend route_circuit_breaker_trace --offline
cargo test -p novex-eval circuit_breaker --offline
cargo test --workspace --offline
```

Expected: PASS, with `live_rag_e2e` ignored unless infra is available.

**Step 4: Merge back to main**

Run the usual clean-status, `git merge --no-ff`, main verification, and worktree fast-forward sequence.
