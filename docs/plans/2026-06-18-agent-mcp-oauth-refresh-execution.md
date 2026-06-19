# Agent MCP OAuth Refresh Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add MCP OAuth refresh-token execution so persisted MCP OAuth sessions can renew access tokens through the shared secretRef boundary without leaking token values.

**Architecture:** `crates/novex-mcp` owns the pure refresh-token grant plan and sanitized evidence. `backend/src/application/ai/mcp_oauth_token_dispatch.rs` owns plaintext refresh-token resolution, token endpoint dispatch, token secret writes, and sanitized outcome assembly. `CapabilityService` owns tenant/session lookup, refresh config recovery from callback metadata, secretRef resolution through `SecretService`, and scoped session upsert.

**Tech Stack:** Rust, Tokio, reqwest, serde_json, sqlx repository contracts, `SecretService`, `novex-mcp` OAuth session contracts.

## Global Constraints

- Keep access and refresh token values out of public DTOs, logs, errors, and evidence.
- Resolve `env:` and `sys:` token refs through a single application boundary before network dispatch.
- Keep `novex-mcp` pure: no database, env, HTTP, or backend service imports.
- Use TDD: add RED tests before production code.
- Preserve existing authorization-code callback behavior.
- Verify offline with focused Rust tests before merging.

---

## File Structure

- Modify: `crates/novex-mcp/src/oauth.rs`
  - Add refresh-token grant vocabulary, config, plan builder, validation, and evidence.
- Modify: `backend/src/application/ai/mcp_oauth_token_dispatch.rs`
  - Add refresh command, resolved refresh-token secret handling, optional PKCE verifier support, and secret-writer dispatch path.
- Modify: `backend/src/application/ai/capability_service.rs`
  - Add refresh command/response service method, callback metadata needed for future refresh, session metadata parsing, and refresh session save helper.
- Modify: `backend/src/interfaces/http/ai/capability.rs`
  - Add authenticated refresh route only if the service method is complete in this slice.
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
  - Update MCP and Secret resolver status/evidence for refresh execution.

## Task 1: Pure Refresh Grant Contract

**Files:**
- Modify: `crates/novex-mcp/src/oauth.rs`

**Interfaces:**
- Produces: `McpOAuthGrantType::RefreshToken`
- Produces: `McpOAuthTokenRefreshConfig { server_code, token_endpoint, client_id, refresh_token, client_auth }`
- Produces: `McpOAuthTokenExchangePlan::refresh_token(config) -> Result<McpOAuthTokenExchangePlan, McpOAuthSessionError>`

- [ ] **Step 1: Write failing tests**

Add tests next to existing `mcp_oauth_session_*` tests:

```rust
#[test]
fn mcp_oauth_session_token_exchange_plan_builds_refresh_token_form_without_leakage() {
    let plan = McpOAuthTokenExchangePlan::refresh_token(McpOAuthTokenRefreshConfig {
        server_code: "docs".to_owned(),
        token_endpoint: "https://mcp.example.com/oauth/token".to_owned(),
        client_id: "client-123".to_owned(),
        refresh_token: "refresh-token-secret-value".to_owned(),
        client_auth: McpOAuthClientAuth::ClientSecretRef("env:MCP_CLIENT_SECRET".to_owned()),
    })
    .unwrap();

    assert_eq!(plan.grant_type, McpOAuthGrantType::RefreshToken);
    assert_eq!(plan.form.get("grant_type").map(String::as_str), Some("refresh_token"));
    assert_eq!(plan.form.get("refresh_token").map(String::as_str), Some("refresh-token-secret-value"));
    assert_eq!(plan.form.get("client_id").map(String::as_str), Some("client-123"));

    let evidence = plan.sanitized_evidence().to_string();
    assert!(evidence.contains("refreshTokenPresent"));
    assert!(!evidence.contains("refresh-token-secret-value"));
}

#[test]
fn mcp_oauth_session_token_exchange_rejects_missing_refresh_token() {
    let err = McpOAuthTokenExchangePlan::refresh_token(McpOAuthTokenRefreshConfig {
        server_code: "docs".to_owned(),
        token_endpoint: "https://mcp.example.com/oauth/token".to_owned(),
        client_id: "client-123".to_owned(),
        refresh_token: " ".to_owned(),
        client_auth: McpOAuthClientAuth::None,
    })
    .unwrap_err();

    assert_eq!(err.field, "refresh_token");
}
```

- [ ] **Step 2: Run RED test**

Run: `cargo test -p novex-mcp mcp_oauth_session_token_exchange --offline`

Expected: FAIL because `McpOAuthTokenRefreshConfig`, `RefreshToken`, and `refresh_token` do not exist.

- [ ] **Step 3: Implement contract**

Add `RefreshToken` to `McpOAuthGrantType`, return `"refresh_token"` from `as_form_value`, add `McpOAuthTokenRefreshConfig`, and implement `McpOAuthTokenExchangePlan::refresh_token`. The refresh plan must set `code_verifier_secret_ref` to an empty string and must omit the raw refresh token from `sanitized_evidence`.

- [ ] **Step 4: Run GREEN test**

Run: `cargo test -p novex-mcp mcp_oauth_session_token_exchange --offline`

Expected: PASS.

## Task 2: Backend Refresh Dispatch

**Files:**
- Modify: `backend/src/application/ai/mcp_oauth_token_dispatch.rs`

**Interfaces:**
- Produces: `McpOAuthRefreshTokenCommand`
- Produces: `refresh_mcp_oauth_session_with_dispatch_and_secret_writer(command, received_at_epoch_seconds, secret_resolve, env_get, dispatch, secret_write)`
- Changes: `McpOAuthTokenDispatchResolvedSecrets.code_verifier: Option<String>`

- [ ] **Step 1: Write failing tests**

Add tests to the existing `mcp_oauth_token_dispatch` test module:

```rust
#[tokio::test]
async fn mcp_oauth_refresh_token_dispatch_resolves_refresh_secret_and_writes_new_tokens_without_leakage() {
    let mut resolved_refs = Vec::new();
    let mut written = Vec::new();
    let outcome = refresh_mcp_oauth_session_with_dispatch_and_secret_writer(
        McpOAuthRefreshTokenCommand {
            server_code: "docs".to_owned(),
            token_endpoint: "https://mcp.example.com/oauth/token".to_owned(),
            client_id: "client-123".to_owned(),
            refresh_token_secret_ref: "sys:tenant:42:mcp.docs.refresh".to_owned(),
            client_auth: McpOAuthClientAuth::ClientSecretRef("env:MCP_CLIENT_SECRET".to_owned()),
            access_token_secret_ref: "sys:tenant:42:mcp.docs.access".to_owned(),
            new_refresh_token_secret_ref: Some("sys:tenant:42:mcp.docs.refresh".to_owned()),
        },
        1_000,
        |secret_ref| {
            resolved_refs.push(secret_ref.to_owned());
            async move { Ok("old-refresh-token".to_owned()) }
        },
        |key| (key == "MCP_CLIENT_SECRET").then(|| "client-secret-value".to_owned()),
        |plan, secrets| async move {
            assert_eq!(secrets.code_verifier, None);
            assert_eq!(secrets.client_secret.as_deref(), Some("client-secret-value"));
            assert_eq!(plan.form.get("refresh_token").map(String::as_str), Some("old-refresh-token"));
            Ok(McpOAuthTokenDispatchHttpResponse {
                http_status: 200,
                content_type: "application/json".to_owned(),
                body: serde_json::json!({
                    "access_token": "new-access-token",
                    "refresh_token": "new-refresh-token",
                    "token_type": "Bearer",
                    "expires_in": 3600,
                    "scope": "tools/read"
                })
                .to_string(),
            })
        },
        |material| {
            written.push(material);
            async { Ok(()) }
        },
    )
    .await
    .unwrap();

    assert_eq!(resolved_refs, vec!["sys:tenant:42:mcp.docs.refresh"]);
    assert_eq!(outcome.session.access_token_secret_ref, "sys:tenant:42:mcp.docs.access");
    assert_eq!(
        outcome.session.refresh_token_secret_ref.as_deref(),
        Some("sys:tenant:42:mcp.docs.refresh")
    );
    assert_eq!(written[0].access_token, "new-access-token");
    assert_eq!(written[0].refresh_token.as_deref(), Some("new-refresh-token"));
    let evidence = outcome.sanitized_evidence().to_string();
    assert!(!evidence.contains("old-refresh-token"));
    assert!(!evidence.contains("new-refresh-token"));
    assert!(!evidence.contains("client-secret-value"));
}
```

- [ ] **Step 2: Run RED test**

Run: `cargo test -p backend mcp_oauth_refresh_token_dispatch --offline`

Expected: FAIL because refresh command and dispatch function do not exist.

- [ ] **Step 3: Implement dispatch**

Resolve `refresh_token_secret_ref` through the injected async resolver, build `McpOAuthTokenExchangePlan::refresh_token`, resolve optional client secret through the existing env resolver, dispatch the plan, parse token response, write new access/refresh token material, and return sanitized evidence.

- [ ] **Step 4: Run focused tests**

Run: `cargo test -p backend mcp_oauth_token_dispatch --offline`

Expected: PASS, including existing authorization-code tests.

## Task 3: CapabilityService Refresh Session Upsert

**Files:**
- Modify: `backend/src/application/ai/capability_service.rs`

**Interfaces:**
- Produces: `McpOAuthRefreshCommand { scope_type, scope_id }`
- Produces: `CapabilityService::refresh_mcp_oauth_session(user_id, server_id, command) -> Result<McpOAuthCallbackResp, AppError>`
- Produces: `mcp_oauth_refresh_token_command_from_session_metadata(record) -> Result<McpOAuthRefreshTokenCommand, AppError>`
- Produces: `mcp_oauth_session_save_record_from_refresh(...) -> Result<McpOAuthSessionSaveRecord, AppError>`

- [ ] **Step 1: Write failing helper tests**

Add tests to `capability_service.rs`:

```rust
#[test]
fn mcp_oauth_callback_session_metadata_keeps_refresh_config_without_token_values() {
    let state = mcp_oauth_state_record_fixture();
    let session = McpOAuthSessionMaterial {
        server_code: "docs".to_owned(),
        access_token_secret_ref: "sys:tenant:42:mcp.docs.access".to_owned(),
        refresh_token_secret_ref: Some("sys:tenant:42:mcp.docs.refresh".to_owned()),
        token_type: "Bearer".to_owned(),
        scopes: vec!["tools/read".to_owned()],
        expires_at_epoch_seconds: Some(1_000),
    };
    let record = mcp_oauth_session_save_record_from_callback(
        42,
        7,
        9,
        &state,
        &session,
        json!({"token":"sanitized"}),
        NaiveDateTime::from_timestamp_opt(0, 0).unwrap(),
    )
    .unwrap();

    assert_eq!(record.metadata["refreshConfig"]["tokenEndpoint"], state.token_endpoint);
    assert_eq!(record.metadata["refreshConfig"]["clientId"], state.client_id);
    assert_eq!(record.metadata["refreshConfig"]["refreshTokenSecretRef"], "sys:tenant:42:mcp.docs.refresh");
    assert!(!record.metadata.to_string().contains("refresh-token-value"));
}
```

- [ ] **Step 2: Write service boundary source test**

Add a source-contract test requiring `find_mcp_oauth_session_for_scope`, `SecretService::resolve_secret_ref`, `refresh_mcp_oauth_session_with_dispatch_and_secret_writer`, and `upsert_mcp_oauth_session` in the service path.

- [ ] **Step 3: Run RED tests**

Run: `cargo test -p backend mcp_oauth_refresh --offline`

Expected: FAIL because refresh metadata parsing and service method do not exist.

- [ ] **Step 4: Implement service method**

Load the scoped session, require a refresh token secret ref, recover refresh config from callback metadata, call `SecretService::resolve_secret_ref`, dispatch the refresh, write refreshed token secrets, build a save record with source `mcp_oauth_refresh`, and upsert the session.

- [ ] **Step 5: Run focused tests**

Run: `cargo test -p backend mcp_oauth_callback --offline && cargo test -p backend mcp_oauth_refresh --offline`

Expected: PASS.

## Task 4: HTTP Route and Matrix

**Files:**
- Modify: `backend/src/interfaces/http/ai/capability.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Produces route: `POST /ai/capabilities/mcp/servers/:server_id/oauth/refresh`

- [ ] **Step 1: Add route test**

Add a source route test requiring the new route, handler, auth extraction, and service method call.

- [ ] **Step 2: Run RED test**

Run: `cargo test -p backend mcp_oauth_refresh_route --offline`

Expected: FAIL until the route is registered.

- [ ] **Step 3: Implement route**

Register the route beside `/oauth/callback`, require the same authenticated user boundary, and call `CapabilityService::refresh_mcp_oauth_session`.

- [ ] **Step 4: Update matrix**

Update the MCP row, Secret resolver row, acceptance evidence, and follow-up list to show refresh-token execution is implemented while automatic scheduler refresh remains next.

- [ ] **Step 5: Verify**

Run:

```bash
cargo fmt --all -- --check
git diff --check
cargo test -p novex-mcp mcp_oauth_session --offline
cargo test -p backend mcp_oauth_token_dispatch --offline
cargo test -p backend mcp_oauth_callback --offline
cargo test -p backend mcp_oauth_refresh --offline
cargo test -p backend mcp_oauth_refresh_route --offline
cargo test -p backend secret --offline
```

Expected: all commands pass.

## Self-Review

- Spec coverage: refresh grant, backend secret resolution, secret writer, session upsert, route, and matrix are covered.
- Placeholder scan: no task depends on undefined future work; scheduler refresh is explicitly out of scope for this slice.
- Type consistency: the service method consumes the backend refresh dispatch command and returns the existing OAuth session response DTO.
