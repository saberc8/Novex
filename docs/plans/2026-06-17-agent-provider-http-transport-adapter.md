# Agent Provider HTTP Transport Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the model provider HTTP send/status boundary from `model_service.rs` into a focused backend transport adapter module.

**Architecture:** Keep Novex's enterprise model service in charge of route resolution, fallback, provider-call leases, dispatch mode, usage/cost accounting, SSE parsing, and response normalization. Move only the reusable reqwest client construction, POST request, bearer auth, JSON body submission, and HTTP status mapping into `model_provider_transport.rs`; later slices can move response parsing and provider-specific clients behind this adapter boundary.

**Tech Stack:** Rust, reqwest, serde_json, backend model runtime service, source-contract tests, offline cargo tests.

## Global Constraints

- Do not change provider request payloads, response parsing, stream/unary dispatch semantics, fallback behavior, provider-call lease persistence, cost accounting, or Agent event payloads.
- `execute_normalized_chat_completion_with_route` must keep dispatch-mode selection and response parsing, but must not own reqwest client construction or provider HTTP status mapping.
- New transport adapter must return `reqwest::Response` so existing streaming chunk handling remains unchanged in this slice.
- The adapter must keep the existing timeout and error text for LLM chat calls: `LLM 模型调用失败: HTTP <status>`.
- This slice does not create a new crate. It prepares for a later `novex-model-provider` or `novex-provider-transport` crate split by adding a backend-local module first.

---

### Task 1: Add HTTP Transport Adapter Source Contract

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Production `model_service.rs` imports `send_model_provider_http_request` and `ModelProviderHttpRequest` from `model_provider_transport`.
- `execute_normalized_chat_completion_with_route` calls the adapter and no longer creates a `reqwest::Client` directly.

- [ ] **Step 1: Write failing source-contract test**

Add backend test `model_provider_http_transport_adapter_source_contract` near the existing model provider dispatch tests:

```rust
#[test]
fn model_provider_http_transport_adapter_source_contract() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    let route_path = &source[source
        .find("async fn execute_normalized_chat_completion_with_route")
        .unwrap()
        ..source.find("fn normalize_model_chat_command").unwrap()];

    assert!(source.contains("model_provider_transport::{"));
    assert!(source.contains("ModelProviderHttpRequest"));
    assert!(source.contains("send_model_provider_http_request"));
    assert!(route_path.contains("send_model_provider_http_request(ModelProviderHttpRequest"));
    assert!(!route_path.contains("reqwest::Client::builder()"));
    assert!(!route_path.contains(".post(&provider_request.endpoint)"));
    assert!(!route_path.contains(".bearer_auth(route.api_key())"));
    assert!(!route_path.contains("LLM 模型调用失败: HTTP {}"));
}
```

- [ ] **Step 2: Verify red**

Run: `cargo test -p backend model_provider_http_transport_adapter --offline`

Expected: FAIL because the adapter module and call do not exist yet.

---

### Task 2: Implement Backend HTTP Transport Adapter

**Files:**
- Create: `backend/src/application/ai/model_provider_transport.rs`
- Modify: `backend/src/application/ai/mod.rs`
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Creates `pub(super) struct ModelProviderHttpRequest<'a>`.
- Creates `pub(super) async fn send_model_provider_http_request(request: ModelProviderHttpRequest<'_>) -> Result<reqwest::Response, AppError>`.
- `model_service.rs` passes endpoint, API key, JSON payload, timeout, and failure prefix into the adapter.

- [ ] **Step 1: Add module declaration**

Add to `backend/src/application/ai/mod.rs`:

```rust
pub mod model_provider_transport;
```

- [ ] **Step 2: Create adapter**

Create `backend/src/application/ai/model_provider_transport.rs`:

```rust
use std::time::Duration;

use serde_json::Value;

use crate::shared::error::AppError;

pub(super) struct ModelProviderHttpRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub payload: &'a Value,
    pub timeout: Duration,
    pub failure_message: &'a str,
}

pub(super) async fn send_model_provider_http_request(
    request: ModelProviderHttpRequest<'_>,
) -> Result<reqwest::Response, AppError> {
    let client = reqwest::Client::builder()
        .timeout(request.timeout)
        .build()
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .json(request.payload)
        .send()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let status = response.status();

    if !status.is_success() {
        return Err(AppError::bad_request(format!(
            "{}: HTTP {}",
            request.failure_message,
            status.as_u16()
        )));
    }

    Ok(response)
}
```

- [ ] **Step 3: Wire model service**

Import the adapter in `model_service.rs`:

```rust
use super::model_provider_transport::{
    send_model_provider_http_request, ModelProviderHttpRequest,
};
```

Replace reqwest client construction and status handling in `execute_normalized_chat_completion_with_route` with:

```rust
let response = send_model_provider_http_request(ModelProviderHttpRequest {
    endpoint: &provider_request.endpoint,
    api_key: route.api_key(),
    payload: &provider_request.payload,
    timeout: MODEL_CHAT_TIMEOUT,
    failure_message: "LLM 模型调用失败",
})
.await?;
```

- [ ] **Step 4: Verify green**

Run: `cargo test -p backend model_provider_http_transport_adapter --offline`

Expected: PASS.

---

### Task 3: Update Matrix And Regression Gates

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Runtime loop matrix status advances from `slice-60 implemented` to `slice-61 implemented`.
- Runtime loop notes mention backend-local provider HTTP transport adapter extraction.
- Acceptance evidence includes `model_provider_http_transport_adapter`.
- Remaining gap narrows to moving provider response parsing/SSE builders and native cancel clients behind provider transport modules.

- [ ] **Step 1: Update migration matrix**

Add the provider HTTP adapter boundary to the Runtime loop row and Runtime loop POC evidence. Add this plan to Follow-up Implementation Plans.

- [ ] **Step 2: Verify**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend model_provider_http_transport_adapter --offline
cargo test -p backend model_provider_stream_dispatch_mode --offline
cargo test -p backend model_provider_stream_dispatch_route_path --offline
cargo test -p backend model_stream_transport_executor --offline
cargo test -p backend model_stream_completion_builder --offline
cargo test -p backend streamed_tool_call_early_stop --offline
cargo test -p backend provider_abort --offline
cargo test --workspace --offline
```

Expected: all commands pass.
