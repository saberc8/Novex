# Agent Provider Native Cancel Transport Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move provider-native cancel HTTP dispatch from `model_service.rs` into the backend-local provider transport adapter.

**Architecture:** Keep `model_service.rs` responsible for provider-call lease lookup, provider support checks, cancel endpoint construction, durable cancellation evidence, and `ModelProviderNativeCancelResp` shaping. Add a focused native-cancel transport API to `model_provider_transport.rs` that owns reqwest client construction, POST dispatch, bearer auth, timeout handling, and HTTP status mapping for `/responses/{id}/cancel`.

**Tech Stack:** Rust, reqwest, backend model runtime service, source-contract tests, offline cargo tests.

## Global Constraints

- Do not change provider native cancel support policy: only OpenAI-compatible and local runtime Responses routes are supported.
- Do not change cancel endpoint construction: `responses/{provider_response_id}/cancel` remains built from the runtime route base URL.
- Do not change durable provider-call lease completion payloads, status transitions, or cancellation response DTO fields.
- Do not move provider-call lease SQL, route lookup, or provider response id planning into the transport adapter in this slice.
- Do not change chat sampling, streaming, compaction, provider-call lease heartbeat, or stream early-stop behavior.
- This slice does not create a new crate. It tightens the backend-local adapter before a later provider-client crate split.

---

### Task 1: Add Native Cancel Adapter Source Contract

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Production `model_service.rs` imports `send_model_provider_native_cancel_request` and `ModelProviderNativeCancelRequest` from `model_provider_transport`.
- `execute_model_provider_native_cancel` delegates the provider HTTP POST to the adapter.
- `execute_model_provider_native_cancel` no longer owns reqwest client construction, bearer auth, or native-cancel HTTP failure formatting.

- [ ] **Step 1: Write failing source-contract test**

Add backend test `model_provider_native_cancel_transport_adapter_source_contract` near the provider transport source-contract tests:

```rust
#[test]
fn model_provider_native_cancel_transport_adapter_source_contract() {
    let service_source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    let transport_source = include_str!("model_provider_transport.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    let cancel_path = &service_source[service_source
        .find("async fn execute_model_provider_native_cancel")
        .unwrap()
        ..service_source
            .find("fn normalize_provider_call_lease_query")
            .unwrap()];

    assert!(service_source.contains("ModelProviderNativeCancelRequest"));
    assert!(service_source.contains("send_model_provider_native_cancel_request"));
    assert!(transport_source.contains("pub(super) struct ModelProviderNativeCancelRequest"));
    assert!(
        transport_source.contains("pub(super) async fn send_model_provider_native_cancel_request")
    );
    assert!(
        cancel_path.contains("send_model_provider_native_cancel_request(ModelProviderNativeCancelRequest")
    );
    assert!(!cancel_path.contains("reqwest::Client::builder()"));
    assert!(!cancel_path.contains(".post(endpoint)"));
    assert!(!cancel_path.contains(".bearer_auth(route.api_key())"));
    assert!(!cancel_path.contains("Provider native cancel failed: HTTP {}"));
}
```

- [ ] **Step 2: Verify red**

Run: `cargo test -p backend model_provider_native_cancel_transport_adapter --offline`

Expected: FAIL because native cancel transport symbols do not exist yet.

---

### Task 2: Implement Native Cancel Transport Adapter

**Files:**
- Modify: `backend/src/application/ai/model_provider_transport.rs`
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Creates `pub(super) struct ModelProviderNativeCancelRequest<'a>`.
- Creates `pub(super) async fn send_model_provider_native_cancel_request(request: ModelProviderNativeCancelRequest<'_>) -> Result<u16, AppError>`.
- `execute_model_provider_native_cancel` receives the returned status code and still calls `model_provider_native_cancel_resp_from_plan`.

- [ ] **Step 1: Add transport request type**

Add to `model_provider_transport.rs`:

```rust
pub(super) struct ModelProviderNativeCancelRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub timeout: Duration,
}
```

- [ ] **Step 2: Add transport dispatch function**

Add to `model_provider_transport.rs`:

```rust
pub(super) async fn send_model_provider_native_cancel_request(
    request: ModelProviderNativeCancelRequest<'_>,
) -> Result<u16, AppError> {
    let client = reqwest::Client::builder()
        .timeout(request.timeout)
        .build()
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .send()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let status = response.status();

    if !status.is_success() {
        return Err(AppError::bad_request(format!(
            "Provider native cancel failed: HTTP {}",
            status.as_u16()
        )));
    }

    Ok(status.as_u16())
}
```

- [ ] **Step 3: Wire model service**

Import the native-cancel adapter in `model_service.rs`:

```rust
use super::model_provider_transport::{
    ModelProviderNativeCancelRequest, send_model_provider_native_cancel_request,
};
```

Replace reqwest client construction in `execute_model_provider_native_cancel` with:

```rust
let status = send_model_provider_native_cancel_request(ModelProviderNativeCancelRequest {
    endpoint,
    api_key: route.api_key(),
    timeout: MODEL_PROVIDER_NATIVE_CANCEL_TIMEOUT,
})
.await?;
```

Keep the final response construction unchanged except for using the returned `status`:

```rust
Ok(model_provider_native_cancel_resp_from_plan(
    plan,
    true,
    Some(status),
    "native_cancel_sent",
))
```

- [ ] **Step 4: Verify green**

Run:

```bash
cargo test -p backend model_provider_native_cancel_transport_adapter --offline
cargo test -p backend provider_call_lease_cancel --offline
cargo test -p backend provider_stream_native_cancel --offline
```

Expected: all commands pass.

---

### Task 3: Update Matrix And Regression Gates

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Runtime loop matrix status advances from `slice-62 implemented` to `slice-63 implemented`.
- Runtime loop notes mention provider-native cancel transport adapter extraction.
- Runtime loop POC evidence mentions provider-native cancel dispatch through the backend-local transport adapter.
- Acceptance evidence includes `model_provider_native_cancel_transport_adapter`.
- Remaining gap narrows to provider-specific client modules and the remaining embedding/rerank/media provider HTTP client extraction.

- [ ] **Step 1: Update migration matrix**

Add this plan to Follow-up Implementation Plans:

```markdown
- Agent provider native cancel transport adapter: `docs/plans/2026-06-17-agent-provider-native-cancel-transport-adapter.md`
```

- [ ] **Step 2: Full verification**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend model_provider_native_cancel_transport_adapter --offline
cargo test -p backend model_provider_response_transport_adapter --offline
cargo test -p backend model_provider_http_transport_adapter --offline
cargo test -p backend provider_call_lease_cancel --offline
cargo test -p backend provider_stream_native_cancel --offline
cargo test -p backend streamed_tool_call_early_stop --offline
cargo test -p backend provider_abort --offline
cargo test --workspace --offline
```

Expected: all commands pass.
