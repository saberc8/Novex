# Agent Provider Client HTTP Primitives Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move reusable provider HTTP client construction and generic POST dispatch primitives from backend-local transport into `novex-provider-client`.

**Architecture:** Keep `backend/src/application/ai/model_provider_transport.rs` as the compatibility facade used by `model_service.rs`. `novex-provider-client` owns provider-neutral HTTP primitives: request DTO, reqwest client construction, POST + bearer auth + JSON payload submission, and provider HTTP status classification. Backend adapter code maps `ModelProviderClientError` into `AppError`, preserving current customer-facing messages and internal-error behavior.

**Tech Stack:** Rust 2021, Cargo workspace, `reqwest`, `serde_json`, backend source-contract tests, provider-client crate unit tests.

## Global Constraints

- Preserve existing `model_service.rs` imports and call sites through the `model_provider_transport` facade.
- Do not change provider payload shapes, timeouts, bearer-auth behavior, HTTP status messages, or response parsing behavior.
- `novex-provider-client` must not depend on `backend`, backend `AppError`, SQL, tenant context, provider-call leases, or run-event persistence.
- Backend remains responsible for converting provider-client errors into `AppError`.
- Use a RED source-contract test before moving production code.
- Verify with focused provider transport tests, provider-client crate tests, formatting, diff checks, and the offline workspace test suite.

---

### Task 1: Provider Client HTTP Primitives

**Files:**
- Modify: `crates/novex-provider-client/Cargo.toml`
- Modify: `crates/novex-provider-client/src/lib.rs`
- Modify: `backend/src/application/ai/model_provider_transport/http.rs`
- Modify: `backend/src/application/ai/model_provider_transport/native_cancel.rs`
- Modify: `backend/src/application/ai/model_provider_transport/rag.rs`
- Modify: `backend/src/application/ai/model_provider_transport/media.rs`
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Test: `crates/novex-provider-client/src/lib.rs`
- Test: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Consumes: existing backend facade names `ModelProviderHttpRequest`, `send_model_provider_http_request`, and shared helper use sites for `model_provider_http_client(request.timeout)`.
- Produces: `novex_provider_client::{ModelProviderHttpRequest, ModelProviderClientError, model_provider_http_client, send_model_provider_http_request}`.

- [ ] **Step 1: Write the failing source-contract test**

Add a backend test named `provider_client_http_primitives_live_in_provider_client_crate` that reads `crates/novex-provider-client/src/lib.rs`, `crates/novex-provider-client/Cargo.toml`, and the backend transport module files at runtime. The test must assert:

```rust
assert!(provider_cargo_source.contains("reqwest = { version = \"0.12\""));
assert!(provider_client_source.contains("pub struct ModelProviderHttpRequest"));
assert!(provider_client_source.contains("pub enum ModelProviderClientError"));
assert!(provider_client_source.contains("pub fn model_provider_http_client"));
assert!(provider_client_source.contains("pub async fn send_model_provider_http_request"));
assert!(provider_client_source.contains(".bearer_auth(request.api_key)"));
assert!(provider_client_source.contains(".json(request.payload)"));
assert!(backend_http_source.contains("use novex_provider_client::{"));
assert!(backend_http_source.contains("ModelProviderClientError"));
assert!(backend_http_source.contains("model_provider_client_error_to_app_error"));
assert!(backend_http_source.contains("pub(in crate::application::ai) use novex_provider_client::ModelProviderHttpRequest"));
assert!(backend_http_source.contains("novex_provider_client::send_model_provider_http_request(request)"));
assert!(!backend_http_source.contains("reqwest::Client::builder()"));
assert!(!backend_http_source.contains(".post(request.endpoint)"));
assert!(native_cancel_source.contains("model_provider_http_client(request.timeout)"));
assert!(rag_source.contains("model_provider_http_client(request.timeout)"));
assert!(media_source.contains("model_provider_http_client(request.timeout)"));
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p backend provider_client_http_primitives_live_in_provider_client_crate --offline`

Expected: FAIL because `novex-provider-client` does not yet own `ModelProviderHttpRequest`, `ModelProviderClientError`, `model_provider_http_client`, or generic HTTP dispatch.

- [ ] **Step 3: Add provider-client reqwest dependency**

Add to `crates/novex-provider-client/Cargo.toml`:

```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

- [ ] **Step 4: Move generic HTTP primitive API into provider-client**

Add to `crates/novex-provider-client/src/lib.rs`:

```rust
#[derive(Debug)]
pub enum ModelProviderClientError {
    Transport(reqwest::Error),
    HttpStatus { failure_message: String, status: u16 },
}

pub struct ModelProviderHttpRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub payload: &'a Value,
    pub timeout: Duration,
    pub failure_message: &'a str,
}

pub fn model_provider_http_client(timeout: Duration) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder().timeout(timeout).build()
}

pub async fn send_model_provider_http_request(
    request: ModelProviderHttpRequest<'_>,
) -> Result<reqwest::Response, ModelProviderClientError> {
    let client =
        model_provider_http_client(request.timeout).map_err(ModelProviderClientError::Transport)?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .json(request.payload)
        .send()
        .await
        .map_err(ModelProviderClientError::Transport)?;
    let status = response.status();

    if !status.is_success() {
        return Err(ModelProviderClientError::HttpStatus {
            failure_message: request.failure_message.to_owned(),
            status: status.as_u16(),
        });
    }

    Ok(response)
}
```

Include crate unit tests for `ModelProviderClientError::HttpStatus` preserving the `failure_message: HTTP status` message shape and `ModelProviderHttpRequest` carrying endpoint, API key, payload, timeout, and failure message.

- [ ] **Step 5: Keep backend facade and error mapping stable**

Replace backend-local generic HTTP implementation in `backend/src/application/ai/model_provider_transport/http.rs` with:

```rust
pub(in crate::application::ai) use novex_provider_client::ModelProviderHttpRequest;
use novex_provider_client::ModelProviderClientError;
pub(super) use novex_provider_client::model_provider_http_client;

pub(in crate::application::ai) async fn send_model_provider_http_request(
    request: ModelProviderHttpRequest<'_>,
) -> Result<reqwest::Response, AppError> {
    novex_provider_client::send_model_provider_http_request(request)
        .await
        .map_err(model_provider_client_error_to_app_error)
}
```

Map `Transport(err)` to `AppError::Anyhow(err.into())` and `HttpStatus { failure_message, status }` to `AppError::bad_request(format!("{failure_message}: HTTP {status}"))`.

- [ ] **Step 6: Update dependent transport modules**

In `native_cancel.rs`, `rag.rs`, and `media.rs`, keep the `model_provider_http_client(request.timeout)` call sites but import the helper through the backend HTTP facade, which now re-exports the provider-client primitive.

- [ ] **Step 7: Update migration matrix**

Change the runtime-loop notes to say `novex-provider-client` now owns provider-neutral RAG response parsers plus reusable HTTP client/POST primitives; RAG/native-cancel/media dispatch extraction remains next.

- [ ] **Step 8: Run focused verification**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend provider_client_http_primitives_live_in_provider_client_crate --offline
cargo test -p backend model_provider_http_transport_adapter --offline
cargo test -p backend model_provider_transport_splits_provider_client_modules --offline
cargo test -p backend model_provider_native_cancel_transport_adapter --offline
cargo test -p backend model_provider_rag_transport_adapter --offline
cargo test -p backend model_provider_media_transport_adapter --offline
cargo test -p novex-provider-client --offline
```

- [ ] **Step 9: Run full workspace verification**

Run: `cargo test --workspace --offline`

- [ ] **Step 10: Commit**

Commit docs and code with:

```bash
git add crates/novex-provider-client backend/src/application/ai/model_provider_transport/http.rs backend/src/application/ai/model_provider_transport/native_cancel.rs backend/src/application/ai/model_provider_transport/rag.rs backend/src/application/ai/model_provider_transport/media.rs backend/src/application/ai/model_service.rs docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-provider-client-http-primitives.md
git commit -m "feat: extract provider client http primitives"
```
