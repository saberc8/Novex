# Agent Provider Native Cancel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a tenant-bound provider-call lease cancel control that can dispatch provider-native Responses cancellation when a response id exists and always leaves durable local cancellation evidence.

**Architecture:** Extend `ModelRuntimeService` provider-call lease controls with a cancel command, native cancel plan helpers, and a running-row-only completion guard. Expose the command through the existing model HTTP router with a new permission seed migration.

**Tech Stack:** Rust, Axum, SQLx/Postgres, reqwest, existing `ai_model_provider_call_lease` table.

## Global Constraints

- TDD: write failing tests first and verify red before production code.
- Keep API keys and prompt/answer payloads out of public responses.
- Do not add WebSocket transport in this slice.
- Do not mark the whole persistent goal complete after this slice.
- Merge the feature worktree back to main after verification.
- Run `cargo clean` in both worktrees after the stage completes.

---

### Task 1: Service Contract And Native Cancel Plan

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `docs/plans/2026-06-17-agent-provider-native-cancel.md`

**Interfaces:**
- Produces: `ModelProviderCallLeaseCancelResp`, `ModelProviderNativeCancelResp`
- Produces: `model_provider_native_cancel_plan(row, route) -> ModelProviderNativeCancelPlan`

- [ ] **Step 1: Write failing tests**

Add tests named:

```rust
#[test]
fn provider_call_lease_native_cancel_plan_uses_responses_cancel_endpoint() {
    let route = openai_compatible_route();
    let row = test_provider_call_lease_control_row(
        "running",
        json!({"providerResponseId": "resp_123"}),
        json!({}),
    );

    let plan = model_provider_native_cancel_plan(&row, Some(&route));

    assert!(plan.supported);
    assert_eq!(plan.provider_response_id.as_deref(), Some("resp_123"));
    assert_eq!(
        plan.endpoint.as_deref(),
        Some("https://llm.internal/v1/responses/resp_123/cancel")
    );
}

#[test]
fn provider_call_lease_native_cancel_plan_requires_provider_response_id() {
    let route = openai_compatible_route();
    let row = test_provider_call_lease_control_row("running", json!({}), json!({}));

    let plan = model_provider_native_cancel_plan(&row, Some(&route));

    assert!(!plan.supported);
    assert_eq!(plan.message, "missing_provider_response_id");
    assert!(plan.endpoint.is_none());
}

#[test]
fn provider_call_lease_cancel_completion_records_native_cancel_evidence() {
    let now =
        NaiveDateTime::parse_from_str("2026-06-17 10:05:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let native = ModelProviderNativeCancelResp {
        attempted: true,
        supported: true,
        provider: "openai-compatible".to_owned(),
        provider_response_id: Some("resp_123".to_owned()),
        endpoint: Some("https://llm.internal/v1/responses/resp_123/cancel".to_owned()),
        http_status: Some(200),
        message: "native_cancel_sent".to_owned(),
    };

    let completion = model_provider_call_lease_completion_from_native_cancel(&native, 32, now);

    assert_eq!(completion.status, "cancelled");
    assert_eq!(completion.error_kind.as_deref(), Some("provider_native_cancel"));
    assert_eq!(completion.response_payload["nativeCancel"]["providerResponseId"], "resp_123");
}
```

- [ ] **Step 2: Run red test**

Run:

```bash
cargo test -p backend-rust provider_call_lease_native_cancel --offline
```

Expected: FAIL because the cancel response structs and plan helpers do not exist.

- [ ] **Step 3: Implement helpers**

Add response structs, native cancel plan structs, provider response id extraction, endpoint construction, and completion payload helper.

- [ ] **Step 4: Run green test**

Run:

```bash
cargo test -p backend-rust provider_call_lease_native_cancel --offline
```

Expected: PASS.

### Task 2: Service Method And Safe Row Completion

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Produces: `ModelRuntimeService::cancel_provider_call_lease(user_id, lease_id)`
- Modifies: `complete_model_provider_call_lease` to update only running rows.

- [ ] **Step 1: Write failing tests**

Add source-contract tests:

```rust
#[test]
fn provider_call_lease_cancel_source_contract_loads_tenant_running_row_and_marks_cancelled() {
    let source = include_str!("model_service.rs").split("#[cfg(test)]").next().unwrap();

    assert!(source.contains("pub async fn cancel_provider_call_lease"));
    assert!(source.contains("WHERE tenant_id = $1"));
    assert!(source.contains("AND id = $2"));
    assert!(source.contains("model_provider_call_lease_completion_from_native_cancel"));
}

#[test]
fn provider_call_lease_completion_only_updates_running_rows() {
    let source = include_str!("model_service.rs").split("#[cfg(test)]").next().unwrap();
    let complete_fn = &source[source.find("async fn complete_model_provider_call_lease").unwrap()
        ..source.find("fn normalize_provider_call_lease_query").unwrap()];

    assert!(complete_fn.contains("AND status = 'running'"));
}
```

- [ ] **Step 2: Run red test**

Run:

```bash
cargo test -p backend-rust provider_call_lease_cancel --offline
```

Expected: FAIL because the service method and running-row guard do not exist.

- [ ] **Step 3: Implement method**

Fetch the lease row, resolve route by route purpose and code, build/send native cancel when supported, mark the lease cancelled, and return the public response.

- [ ] **Step 4: Run green test**

Run:

```bash
cargo test -p backend-rust provider_call_lease_cancel --offline
```

Expected: PASS.

### Task 3: HTTP Route, Permission Seed, Matrix

**Files:**
- Modify: `backend/src/interfaces/http/ai/model.rs`
- Create: `backend/migrations/202606170010_seed_ai_model_provider_call_lease_cancel_permission.sql`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-provider-native-cancel.md`

**Interfaces:**
- Produces: `POST /ai/models/provider-call-leases/:lease_id/cancel`
- Produces: `ai:model:providerCallLease:cancel`

- [ ] **Step 1: Write failing tests**

Add tests named:

```rust
#[test]
fn provider_call_lease_cancel_permission_seed_contains_control() {
    let seed = include_str!(
        "../../../../migrations/202606170010_seed_ai_model_provider_call_lease_cancel_permission.sql"
    );

    assert!(seed.contains(MODEL_PROVIDER_CALL_LEASE_CANCEL_PERMISSION));
}

#[tokio::test]
async fn provider_call_lease_cancel_handler_rejects_missing_permission() {
    let err = cancel_provider_call_lease(
        State(test_state()),
        user_with_permissions(vec![]),
        axum::extract::Path(123),
    )
    .await
    .unwrap_err();

    assert!(matches!(err, AppError::Forbidden));
}

#[test]
fn provider_call_lease_cancel_route_is_registered() {
    let source = include_str!("model.rs");

    assert!(source.contains("/ai/models/provider-call-leases/:lease_id/cancel"));
    assert!(source.contains("MODEL_PROVIDER_CALL_LEASE_CANCEL_PERMISSION"));
}
```

- [ ] **Step 2: Run red test**

Run:

```bash
cargo test -p backend-rust provider_call_lease_cancel --offline
```

Expected: FAIL because route, permission, and migration do not exist.

- [ ] **Step 3: Implement HTTP and migration**

Wire the route and seed permission id `3029`.

- [ ] **Step 4: Update matrix**

Move provider-native cancel controls into Runtime loop evidence while keeping WebSocket streaming and background Responses response-id capture as remaining work.

- [ ] **Step 5: Verify**

Run:

```bash
cargo test -p backend-rust provider_call_lease_cancel --offline
cargo test -p backend-rust provider_call_lease --offline
cargo test -p backend-rust provider_abort --offline
cargo fmt -- --check
cargo test --workspace --offline
git diff --check
```

Expected: all pass with exit code 0.

### Task 4: Commit, Merge, Clean

**Files:**
- No code edits unless verification exposes a defect.

- [ ] **Step 1: Commit feature branch**

```bash
git add backend/src/application/ai/model_service.rs backend/src/interfaces/http/ai/model.rs backend/migrations/202606170010_seed_ai_model_provider_call_lease_cancel_permission.sql docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-provider-native-cancel-design.md docs/plans/2026-06-17-agent-provider-native-cancel.md
git commit -m "feat: add provider call lease cancellation control"
```

- [ ] **Step 2: Merge to main**

```bash
cd /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex
git merge --no-ff feat/enterprise-agent-foundation -m "merge: enterprise agent foundation provider native cancel"
```

- [ ] **Step 3: Verify on main**

```bash
cargo fmt -- --check
cargo test --workspace --offline
git diff --check
```

- [ ] **Step 4: Clean and align**

```bash
cargo clean
cd /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex/.worktrees/enterprise-agent-foundation
cargo clean
git merge --ff-only main
```
