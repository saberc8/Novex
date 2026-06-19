# Agent Provider Client Native Cancel Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move provider-native Responses cancel HTTP dispatch from backend-local transport into `novex-provider-client`.

**Architecture:** `novex-provider-client` owns the native-cancel request DTO, bearer-auth POST dispatch, reqwest client construction, transport error wrapping, and provider HTTP status mapping. Backend `model_provider_transport/native_cancel.rs` remains the compatibility adapter that maps `ModelProviderClientError` into `AppError`; response-id planning, route resolution, provider-call leases, durable cancellation evidence, trace/eval metadata, tenant context, and public service APIs stay in backend.

**Tech Stack:** Rust 2021, Cargo workspace, `reqwest`, backend source-contract tests, provider-client crate unit tests.

## Global Constraints

- Preserve existing `model_service.rs` imports and call sites through `model_provider_transport`.
- Do not change native-cancel request method, bearer-auth behavior, timeout use, returned HTTP status, or error message text.
- `novex-provider-client` must not depend on `backend`, backend `AppError`, SQL, tenant context, provider-call leases, run-event persistence, or trace/eval crates.
- Backend remains responsible for converting `ModelProviderClientError` into `AppError`.
- Use a RED source-contract test before moving production code.
- Verify with focused native-cancel/provider-client tests, formatting, diff checks, and the offline workspace test suite.
- Merge this phase back to `main`, sync the feature worktree, and run `cargo clean` in both worktrees.

---

### Task 1: Provider Client Native Cancel Dispatch

**Files:**
- Modify: `crates/novex-provider-client/src/lib.rs`
- Modify: `backend/src/application/ai/model_provider_transport/native_cancel.rs`
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Test: `crates/novex-provider-client/src/lib.rs`
- Test: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Consumes: existing provider-client `model_provider_http_client(timeout)` and `ModelProviderClientError`.
- Produces: `novex_provider_client::{ModelProviderNativeCancelRequest, send_model_provider_native_cancel_request}` returning `Result<u16, ModelProviderClientError>`.

- [ ] **Step 1: Write the failing source-contract test**

Add a backend test named `provider_client_native_cancel_dispatch_lives_in_provider_client_crate` that reads `crates/novex-provider-client/src/lib.rs`, `backend/src/application/ai/model_provider_transport/http.rs`, and `backend/src/application/ai/model_provider_transport/native_cancel.rs`. The test must assert:

```rust
assert!(provider_client_source.contains("pub struct ModelProviderNativeCancelRequest"));
assert!(provider_client_source.contains("pub async fn send_model_provider_native_cancel_request"));
assert!(provider_client_source.contains(".post(request.endpoint)"));
assert!(provider_client_source.contains(".bearer_auth(request.api_key)"));
assert!(provider_client_source.contains("model_provider_http_client(request.timeout)"));
assert!(provider_client_source.contains("Provider native cancel failed"));
assert!(provider_client_source.contains("ModelProviderClientError::HttpStatus"));
assert!(backend_http_source.contains("pub(super) fn model_provider_client_error_to_app_error"));
assert!(backend_native_source.contains("pub(in crate::application::ai) use novex_provider_client::ModelProviderNativeCancelRequest"));
assert!(backend_native_source.contains("novex_provider_client::send_model_provider_native_cancel_request(request)"));
assert!(backend_native_source.contains("model_provider_client_error_to_app_error"));
assert!(!backend_native_source.contains("model_provider_http_client(request.timeout)"));
assert!(!backend_native_source.contains(".post(request.endpoint)"));
assert!(!backend_native_source.contains(".bearer_auth(request.api_key)"));
assert!(!backend_native_source.contains("Provider native cancel failed: HTTP"));
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p backend provider_client_native_cancel_dispatch_lives_in_provider_client_crate --offline`

Expected: FAIL because backend `native_cancel.rs` still owns the request DTO, provider POST dispatch, bearer auth, status mapping, and native-cancel failure message.

- [ ] **Step 3: Move native-cancel DTO and dispatch into provider-client**

Add to `crates/novex-provider-client/src/lib.rs`:

```rust
pub struct ModelProviderNativeCancelRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub timeout: Duration,
}

pub async fn send_model_provider_native_cancel_request(
    request: ModelProviderNativeCancelRequest<'_>,
) -> Result<u16, ModelProviderClientError> {
    let client =
        model_provider_http_client(request.timeout).map_err(ModelProviderClientError::Transport)?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .send()
        .await
        .map_err(ModelProviderClientError::Transport)?;
    let status = response.status();

    if !status.is_success() {
        return Err(ModelProviderClientError::HttpStatus {
            failure_message: "Provider native cancel failed".to_owned(),
            status: status.as_u16(),
        });
    }

    Ok(status.as_u16())
}
```

- [ ] **Step 4: Add provider-client unit test for request shape and failure display**

Add tests in `crates/novex-provider-client/src/lib.rs`:

```rust
#[test]
fn native_cancel_request_carries_provider_dispatch_inputs() {
    let request = ModelProviderNativeCancelRequest {
        endpoint: "https://provider.example/v1/responses/resp_123/cancel",
        api_key: "secret",
        timeout: Duration::from_secs(8),
    };

    assert_eq!(
        request.endpoint,
        "https://provider.example/v1/responses/resp_123/cancel"
    );
    assert_eq!(request.api_key, "secret");
    assert_eq!(request.timeout, Duration::from_secs(8));
}

#[test]
fn native_cancel_http_status_error_preserves_backend_message_shape() {
    let error = ModelProviderClientError::HttpStatus {
        failure_message: "Provider native cancel failed".to_owned(),
        status: 409,
    };

    assert_eq!(error.to_string(), "Provider native cancel failed: HTTP 409");
}
```

- [ ] **Step 5: Replace backend native-cancel transport with thin adapter**

Replace `backend/src/application/ai/model_provider_transport/native_cancel.rs` with:

```rust
pub(in crate::application::ai) use novex_provider_client::ModelProviderNativeCancelRequest;

use super::http::model_provider_client_error_to_app_error;
use crate::shared::error::AppError;

pub(in crate::application::ai) async fn send_model_provider_native_cancel_request(
    request: ModelProviderNativeCancelRequest<'_>,
) -> Result<u16, AppError> {
    novex_provider_client::send_model_provider_native_cancel_request(request)
        .await
        .map_err(model_provider_client_error_to_app_error)
}
```

- [ ] **Step 6: Update existing source-contract tests and migration matrix**

Update existing provider transport source-contract tests so they expect backend `native_cancel.rs` to call the provider-client dispatch and no longer own `model_provider_http_client(request.timeout)`. Update the migration matrix runtime-loop notes and acceptance evidence to say `novex-provider-client` owns native-cancel dispatch APIs alongside HTTP primitives and RAG dispatch.

- [ ] **Step 7: Verify focused and full workspace**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend provider_client_native_cancel_dispatch_lives_in_provider_client_crate --offline
cargo test -p backend provider_client_http_primitives_live_in_provider_client_crate --offline
cargo test -p backend model_provider_native_cancel_transport_adapter --offline
cargo test -p backend provider_call_lease_native_cancel --offline
cargo test -p backend provider_call_lease_cancel --offline
cargo test -p backend provider_stream_native_cancel --offline
cargo test -p novex-provider-client --offline
cargo test --workspace --offline
```

Expected: all commands pass.

- [ ] **Step 8: Commit, merge, sync, clean**

Commit the plan before production changes:

```bash
git add docs/plans/2026-06-17-agent-provider-client-native-cancel-dispatch.md
git commit -m "docs: plan provider client native cancel dispatch"
```

Commit implementation after verification:

```bash
git add crates/novex-provider-client/src/lib.rs backend/src/application/ai/model_provider_transport/native_cancel.rs backend/src/application/ai/model_service.rs docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-provider-client-native-cancel-dispatch.md
git commit -m "feat: extract provider client native cancel dispatch"
```

Merge the phase into `main`, fast-forward the feature worktree from `main`, then run `cargo clean` in both worktrees.
