# Agent MCP OAuth Refresh Scheduler Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a scheduler builtin job that automatically refreshes due MCP OAuth sessions before access tokens expire.

**Architecture:** `ai_mcp_oauth_session.refresh_needed_after` remains the persisted readiness signal. `AiCapabilityRepository` provides a bounded due-session query across tenants. `CapabilityService` owns the batch refresh loop by reusing the existing tenant-bound refresh service path. `application::scheduler::executor` exposes the builtin key `ai.mcp.oauth_refresh`, and a migration seeds a default enabled job.

**Tech Stack:** Rust, Tokio, sqlx, PostgreSQL migrations, existing scheduler builtin executor, `SecretService`, MCP OAuth refresh dispatch.

## Global Constraints

- Do not expose access-token or refresh-token plaintext in scheduler logs, response bodies, errors, or evidence.
- Reuse the existing `refresh_mcp_oauth_session` service path instead of adding a second token refresh implementation.
- Bound each scheduler run to a small deterministic batch to avoid long-running scheduler workers.
- Keep tenant isolation: each due session refresh must run through a `CapabilityService::for_tenant` instance for that session tenant.
- Use TDD: add RED tests before production code.
- After completion, merge into `main`, run `cargo clean`, and remove the temporary worktree/branch.

---

## Task 1: Due Session Query Contract

**Files:**
- Modify: `backend/src/infrastructure/persistence/ai_capability_repository.rs`

**Interfaces:**
- Produces: `AiCapabilityRepository::list_due_mcp_oauth_sessions(now: NaiveDateTime, limit: i64) -> Result<Vec<McpOAuthSessionRecord>, AppError>`

- [ ] **Step 1: Write failing repository test**

Add a source-contract test:

```rust
#[test]
fn mcp_oauth_persistence_repository_lists_due_refresh_sessions() {
    let source = include_str!("ai_capability_repository.rs");

    assert!(source.contains("list_due_mcp_oauth_sessions"));
    assert!(source.contains("refresh_needed_after <= "));
    assert!(source.contains("refresh_token_secret_ref IS NOT NULL"));
    assert!(source.contains("revoked_at IS NULL"));
    assert!(source.contains("ORDER BY refresh_needed_after ASC, id ASC LIMIT"));
}
```

- [ ] **Step 2: Run RED test**

Run: `cargo test -p backend mcp_oauth_persistence_repository_lists_due_refresh_sessions --offline`

Expected: FAIL because the method and query contract do not exist.

- [ ] **Step 3: Implement query**

Add `MCP_OAUTH_SESSION_DUE_REFRESH_SQL` using the same selected columns as `MCP_OAUTH_SESSION_LOOKUP_SQL`, filtered by `status = 1`, `revoked_at IS NULL`, `refresh_token_secret_ref IS NOT NULL`, and `refresh_needed_after <= $1`, ordered by oldest refresh need first.

- [ ] **Step 4: Run GREEN test**

Run: `cargo test -p backend mcp_oauth_persistence --offline`

Expected: PASS.

## Task 2: CapabilityService Batch Refresh

**Files:**
- Modify: `backend/src/application/ai/capability_service.rs`

**Interfaces:**
- Produces: `McpOAuthRefreshBatchSummary { attempted, refreshed, failed }`
- Produces: `CapabilityService::refresh_due_mcp_oauth_sessions(db: PgPool, limit: i64) -> Result<McpOAuthRefreshBatchSummary, AppError>`

- [ ] **Step 1: Write failing source-contract test**

Add a test requiring the batch service to query due sessions, instantiate tenant-bound services, call existing refresh path, and keep batch count bounded:

```rust
#[test]
fn mcp_oauth_refresh_scheduler_service_uses_due_query_and_existing_refresh_path() {
    let source = include_str!("capability_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("pub async fn refresh_due_mcp_oauth_sessions"));
    assert!(source.contains("list_due_mcp_oauth_sessions"));
    assert!(source.contains("CapabilityService::for_tenant"));
    assert!(source.contains(".refresh_mcp_oauth_session("));
    assert!(source.contains("MCP_OAUTH_REFRESH_SCHEDULER_USER_ID"));
}
```

- [ ] **Step 2: Run RED test**

Run: `cargo test -p backend mcp_oauth_refresh_scheduler --offline`

Expected: FAIL because the batch summary and service method do not exist.

- [ ] **Step 3: Implement batch service**

Add a bounded limit normalizer, query due sessions through `AiCapabilityRepository`, and for each session call `CapabilityService::for_tenant(db.clone(), session.tenant_id).refresh_mcp_oauth_session(MCP_OAUTH_REFRESH_SCHEDULER_USER_ID, session.server_id, McpOAuthRefreshCommand { scope_type, scope_id })`. Count attempted/refreshed/failed and continue after individual failures.

- [ ] **Step 4: Run GREEN test**

Run: `cargo test -p backend mcp_oauth_refresh_scheduler --offline`

Expected: PASS.

## Task 3: Scheduler Builtin Key

**Files:**
- Modify: `backend/src/application/scheduler/executor.rs`
- Modify: `backend/src/application/scheduler/service.rs`

**Interfaces:**
- Produces builtin key: `ai.mcp.oauth_refresh`

- [ ] **Step 1: Write failing scheduler tests**

Add tests requiring the builtin key to be accepted and routed:

```rust
#[test]
fn mcp_oauth_refresh_key_source_contract_routes_scheduler_builtin() {
    let source = include_str!("executor.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("ai.mcp.oauth_refresh"));
    assert!(source.contains("CapabilityService::refresh_due_mcp_oauth_sessions"));
}

#[test]
fn mcp_oauth_refresh_key_builtin_job_is_accepted() {
    let mut command = base_command();
    command.task_type = JOB_TYPE_BUILTIN;
    command.builtin_key = "ai.mcp.oauth_refresh".to_owned();

    let command = normalize_job_command(command, &HttpSafetyConfig::default()).unwrap();

    assert_eq!(command.builtin_key, "ai.mcp.oauth_refresh");
}
```

- [ ] **Step 2: Run RED test**

Run: `cargo test -p backend mcp_oauth_refresh_key --offline`

Expected: FAIL because executor does not route the key and service test is missing.

- [ ] **Step 3: Implement builtin routing**

Import `CapabilityService` in `scheduler/executor.rs`, add the match branch for `ai.mcp.oauth_refresh`, call the batch service with a small limit such as `50`, and serialize the summary in the response body.

- [ ] **Step 4: Run GREEN test**

Run: `cargo test -p backend scheduler --offline`

Expected: PASS.

## Task 4: Seed Job and Matrix

**Files:**
- Create: `backend/migrations/202606180002_seed_mcp_oauth_refresh_scheduler.sql`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Seeds `sys_job` id `3600003`, group `ai-ops`, builtin key `ai.mcp.oauth_refresh`.

- [ ] **Step 1: Write failing migration test**

Add a test that includes the migration text and requires the seeded key, job id, and enabled status.

- [ ] **Step 2: Run RED test**

Run: `cargo test -p backend mcp_oauth_refresh_scheduler_seed --offline`

Expected: FAIL because the migration file does not exist.

- [ ] **Step 3: Add migration**

Create an idempotent `INSERT INTO sys_job ... ON CONFLICT DO NOTHING` migration using a 60-second cron (`*/60 * * * * *`), `status = 1`, `max_retry = 1`, `timeout_seconds = 300`, and description `Refresh due MCP OAuth sessions before token expiry.`

- [ ] **Step 4: Update matrix and verify**

Update MCP row, acceptance evidence, and follow-up list to show automatic refresh scheduler is implemented while deployed external MCP OAuth smoke remains next.

Run:

```bash
cargo fmt --all -- --check
git diff --check
cargo test -p backend mcp_oauth_persistence --offline
cargo test -p backend mcp_oauth_refresh_scheduler --offline
cargo test -p backend scheduler --offline
cargo test -p backend secret --offline
```

Expected: all commands pass.

## Self-Review

- Spec coverage: due query, batch service, scheduler builtin key, default seed job, matrix evidence, and verification are covered.
- Placeholder scan: no placeholder work remains; deployed external smoke is explicitly outside this slice.
- Type consistency: the scheduler uses `McpOAuthRefreshBatchSummary` returned by `CapabilityService::refresh_due_mcp_oauth_sessions`.
