# Agent Provider Client Chat Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move provider-neutral chat and Responses HTTP dispatch APIs from backend route execution into `novex-provider-client`.

**Status:** Implemented in branch `feat/enterprise-agent-foundation`; pending final full verification and merge at the time this plan was updated.

**Architecture:** Keep backend responsible for route selection, provider request planning, payload construction, stream/unary mode selection, provider-call leases, trace/eval metadata, and stream event emission. Add chat-specific provider-client request APIs that POST the already-built payload with the standard LLM failure message; stream dispatch returns `reqwest::Response` for backend event draining, while unary dispatch returns provider response text for backend parser routing. `backend/src/application/ai/model_provider_transport.rs` remains the compatibility facade that maps `ModelProviderClientError` into `AppError`.

**Tech Stack:** Rust 2021, Cargo workspace, `reqwest`, `serde_json`, `novex-provider-client`, backend source-contract tests, provider-client unit tests, offline cargo verification.

## Global Constraints

- Do not move `ModelRuntimeRoute`, `ModelChatCommand`, provider request planning, payload construction, fallback, provider-call leases, cost accounting, Agent events, trace/eval, or tenant context into `novex-provider-client`.
- `novex-provider-client` must not depend on `backend-rust`, SQL, tenant context, run-event persistence, or trace/eval crates.
- Chat dispatch must preserve the existing user-facing failure message shape: `LLM 模型调用失败: HTTP {status}`.
- Streaming chat dispatch must continue returning the raw `reqwest::Response` so backend can emit `ModelProviderStreamEvent` with route, provider, model, and lease metadata.
- Unary chat dispatch must read response text inside `novex-provider-client` and return it to backend for transport-specific parsing.
- Verify with focused chat-dispatch/provider-client tests, formatting, diff checks, and the offline workspace test suite.

---

### Task 1: Add Provider-Client Chat Dispatch Source Contract

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Consumes: existing provider-client HTTP primitives and chat parser source-contract tests.
- Produces: backend test `provider_client_chat_dispatch_lives_in_provider_client_crate`.

- [ ] **Step 1: Write the failing source-contract test**

Add this backend source-contract test near the existing provider-client tests:

```rust
#[test]
fn provider_client_chat_dispatch_lives_in_provider_client_crate() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend manifest should live below workspace root");
    let source = |path: &str| {
        std::fs::read_to_string(workspace_root.join(path))
            .unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
    };
    let provider_client_source = source("crates/novex-provider-client/src/lib.rs");
    let backend_transport_source = source("backend/src/application/ai/model_provider_transport.rs");
    let service_source = source("backend/src/application/ai/model_service.rs");
    let route_path = &service_source[service_source
        .find("async fn execute_normalized_chat_completion_with_route")
        .unwrap()
        ..service_source.find("fn normalize_model_chat_command").unwrap()];

    assert!(provider_client_source.contains("pub struct ModelProviderChatRequest"));
    assert!(provider_client_source.contains("pub async fn send_model_provider_chat_request"));
    assert!(provider_client_source.contains("pub async fn send_model_provider_chat_unary_request"));
    assert!(provider_client_source.contains("failure_message: \"LLM 模型调用失败\""));
    assert!(provider_client_source.contains("read_model_provider_response_text(response).await"));
    assert!(backend_transport_source.contains("ModelProviderChatRequest"));
    assert!(backend_transport_source
        .contains("novex_provider_client::send_model_provider_chat_request(request)"));
    assert!(backend_transport_source
        .contains("novex_provider_client::send_model_provider_chat_unary_request(request)"));
    assert!(backend_transport_source.contains("model_provider_client_error_to_app_error"));
    assert!(route_path.contains("send_model_provider_chat_request(ModelProviderChatRequest"));
    assert!(route_path.contains("send_model_provider_chat_unary_request(ModelProviderChatRequest"));
    assert!(!route_path.contains("send_model_provider_http_request(ModelProviderHttpRequest"));
    assert!(!route_path.contains("read_model_provider_response_text(response).await?"));
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p backend-rust provider_client_chat_dispatch_lives_in_provider_client_crate --offline`

Expected: FAIL because provider-client does not yet expose chat-specific dispatch APIs and route execution still calls generic HTTP dispatch directly.

---

### Task 2: Add Chat Dispatch APIs In Provider Client

**Files:**
- Modify: `crates/novex-provider-client/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub struct ModelProviderChatRequest<'a> { pub endpoint: &'a str, pub api_key: &'a str, pub payload: &'a Value, pub timeout: Duration }`
  - `pub async fn send_model_provider_chat_request(request: ModelProviderChatRequest<'_>) -> Result<reqwest::Response, ModelProviderClientError>`
  - `pub async fn send_model_provider_chat_unary_request(request: ModelProviderChatRequest<'_>) -> Result<String, ModelProviderClientError>`

- [ ] **Step 1: Add provider-client unit tests**

Add tests in `crates/novex-provider-client/src/lib.rs`:

```rust
#[test]
fn chat_request_carries_provider_dispatch_inputs() {
    let payload = json!({"model": "gpt-compatible", "messages": [], "stream": false});
    let request = ModelProviderChatRequest {
        endpoint: "https://provider.example/v1/chat/completions",
        api_key: "secret",
        payload: &payload,
        timeout: Duration::from_secs(120),
    };

    assert_eq!(request.endpoint, "https://provider.example/v1/chat/completions");
    assert_eq!(request.api_key, "secret");
    assert_eq!(request.payload["model"], "gpt-compatible");
    assert_eq!(request.timeout, Duration::from_secs(120));
}

#[test]
fn chat_http_status_error_preserves_backend_message_shape() {
    let error = ModelProviderClientError::HttpStatus {
        failure_message: "LLM 模型调用失败".to_owned(),
        status: 503,
    };

    assert_eq!(error.to_string(), "LLM 模型调用失败: HTTP 503");
}
```

- [ ] **Step 2: Add minimal provider-client implementation**

Add `ModelProviderChatRequest`, `send_model_provider_chat_request`, and `send_model_provider_chat_unary_request`. Implement chat dispatch by delegating to `send_model_provider_http_request(ModelProviderHttpRequest { failure_message: "LLM 模型调用失败", ... })`; implement unary dispatch by awaiting chat dispatch then calling `read_model_provider_response_text(response).await`.

- [ ] **Step 3: Verify provider-client tests**

Run: `cargo test -p novex-provider-client chat --offline`

Expected: PASS.

---

### Task 3: Add Backend Compatibility Facade And Route Execution Wiring

**Files:**
- Modify: `backend/src/application/ai/model_provider_transport.rs`
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Consumes: provider-client chat dispatch APIs from Task 2.
- Produces backend wrappers:
  - `pub(super) async fn send_model_provider_chat_request(request: ModelProviderChatRequest<'_>) -> Result<reqwest::Response, AppError>`
  - `pub(super) async fn send_model_provider_chat_unary_request(request: ModelProviderChatRequest<'_>) -> Result<String, AppError>`

- [ ] **Step 1: Add backend facade wrappers**

In `backend/src/application/ai/model_provider_transport.rs`, re-export `ModelProviderChatRequest` and add wrappers that map `ModelProviderClientError` through `model_provider_client_error_to_app_error`.

- [ ] **Step 2: Wire route execution**

Update `execute_normalized_chat_completion_with_route(...)`:

```rust
let stream_dispatch = matches!(dispatch_mode, ModelProviderDispatchMode::Stream);
if stream_dispatch && model_chat_provider_request_streams_chat_completion(&provider_request) {
    let response = send_model_provider_chat_request(ModelProviderChatRequest {
        endpoint: &provider_request.endpoint,
        api_key: route.api_key(),
        payload: &provider_request.payload,
        timeout: MODEL_CHAT_TIMEOUT,
    })
    .await?;
    let output = model_chat_streaming_provider_output(response, route, command).await?;
    return Ok(model_chat_response_from_provider_output(
        route,
        output,
        started.elapsed().as_millis(),
        conversation_id,
    ));
}

let body_text = send_model_provider_chat_unary_request(ModelProviderChatRequest {
    endpoint: &provider_request.endpoint,
    api_key: route.api_key(),
    payload: &provider_request.payload,
    timeout: MODEL_CHAT_TIMEOUT,
})
.await?;
```

- [ ] **Step 3: Update existing source-contract tests**

Change `model_provider_stream_dispatch_route_path_separates_unary_and_stream`, `model_provider_http_transport_adapter_source_contract`, and `model_provider_response_transport_adapter_source_contract` so they no longer expect route execution to call generic `send_model_provider_http_request` or `read_model_provider_response_text(response).await?` for chat dispatch.

- [ ] **Step 4: Verify focused backend tests**

Run:

```bash
cargo test -p backend-rust provider_client_chat_dispatch_lives_in_provider_client_crate --offline
cargo test -p backend-rust model_provider_http_transport_adapter --offline
cargo test -p backend-rust model_provider_response_transport_adapter --offline
cargo test -p backend-rust model_provider_stream_dispatch_route_path --offline
cargo test -p backend-rust provider_token_delta --offline
cargo test -p backend-rust provider_compact_transport --offline
```

Expected: all commands pass.

---

### Task 4: Update Documentation, Verify, Commit, Merge, And Clean

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-18-agent-provider-client-chat-dispatch.md`

**Interfaces:**
- Consumes: provider-client chat dispatch APIs from Tasks 2 and 3.
- Produces: migration matrix note that `novex-provider-client` owns chat/Responses dispatch APIs while backend still owns route execution and stream event emission.

- [ ] **Step 1: Update migration matrix**

Change runtime-loop notes and acceptance evidence to say `novex-provider-client` owns chat dispatch request DTO, stream dispatch API, unary dispatch API, and response text reading. Leave provider request planning and payload construction in backend.

- [ ] **Step 2: Run verification**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p novex-provider-client --offline
cargo test -p backend-rust provider_client_chat_dispatch_lives_in_provider_client_crate --offline
cargo test -p backend-rust model_provider_http_transport_adapter --offline
cargo test -p backend-rust model_provider_response_transport_adapter --offline
cargo test -p backend-rust model_provider_stream_dispatch_route_path --offline
cargo test -p backend-rust provider_token_delta --offline
cargo test -p backend-rust provider_compact_transport --offline
cargo test --workspace --offline
```

- [ ] **Step 3: Commit implementation**

```bash
git add crates/novex-provider-client/src/lib.rs backend/src/application/ai/model_provider_transport.rs backend/src/application/ai/model_service.rs docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-18-agent-provider-client-chat-dispatch.md
git commit -m "feat: extract provider client chat dispatch"
```

- [ ] **Step 4: Merge to main, verify, sync, and clean**

```bash
git -C /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex merge --ff-only feat/enterprise-agent-foundation
cargo fmt -- --check
git diff --check
cargo test --workspace --offline
cargo clean
git -C /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex/.worktrees/enterprise-agent-foundation merge --ff-only main
cargo clean
```
