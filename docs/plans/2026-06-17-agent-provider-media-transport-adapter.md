# Agent Provider Media Transport Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move media image provider HTTP dispatch from `model_service.rs` into the backend-local provider transport adapter.

**Architecture:** Keep `model_service.rs` responsible for route resolution, provider-call leases, media trace/eval metadata, and the public `generate_media_image*` API. Add a focused media transport API to `model_provider_transport.rs` that owns request payload submission, reqwest client construction, bearer and `x-api-key` auth headers, timeout handling, JSON body reading, HTTP status mapping, and media asset response parsing.

**Tech Stack:** Rust, reqwest, serde_json, novex-tools media request/response helpers, backend model runtime service, source-contract tests, offline cargo tests.

## Global Constraints

- Do not change media image provider payload shape; keep using `MediaImageGenerationRequest::to_provider_payload()`.
- Do not change provider auth behavior; keep both bearer auth and `x-api-key` header.
- Do not change media provider-call lease creation, heartbeat, completion payloads, or source metadata.
- Do not change `execute_media_image_tool`, media job persistence, media asset persistence, or dry-run behavior.
- Do not change embedding/rerank/chat transport adapters in this slice.
- This slice does not create a new crate. It completes the backend-local transport adapter boundary before a later provider-client crate split.

---

### Task 1: Add Media Transport Source Contract

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Production `model_service.rs` imports `ModelProviderMediaImageRequest` and `send_model_provider_media_image_request` from `model_provider_transport`.
- `ModelRuntimeService::generate_media_image` delegates provider HTTP work to the media adapter.
- `model_service.rs` no longer owns media reqwest client construction, provider POST dispatch, bearer auth, `x-api-key` auth, HTTP failure formatting, JSON body reading, or media provider response parsing.

- [ ] **Step 1: Write failing source-contract test**

Add backend test `model_provider_media_transport_adapter_source_contract` near the provider transport source-contract tests:

```rust
#[test]
fn model_provider_media_transport_adapter_source_contract() {
    let service_source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    let transport_source = include_str!("model_provider_transport.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    let media_path = &service_source[service_source
        .find("pub async fn generate_media_image")
        .unwrap()
        ..service_source
            .find("pub async fn generate_media_image_for_source")
            .unwrap()];

    assert!(service_source.contains("ModelProviderMediaImageRequest"));
    assert!(service_source.contains("send_model_provider_media_image_request"));
    assert!(transport_source.contains("pub(super) struct ModelProviderMediaImageRequest"));
    assert!(
        transport_source.contains("pub(super) async fn send_model_provider_media_image_request")
    );
    assert!(
        media_path.contains("send_model_provider_media_image_request(ModelProviderMediaImageRequest")
    );
    assert!(!media_path.contains("reqwest::Client::builder()"));
    assert!(!media_path.contains(".post(route.endpoint())"));
    assert!(!media_path.contains(".bearer_auth(route.api_key())"));
    assert!(!media_path.contains(".header(\"x-api-key\", route.api_key())"));
    assert!(!media_path.contains("parse_media_image_generation_response"));
    assert!(!service_source.contains("parse_media_image_generation_response"));
}
```

- [ ] **Step 2: Verify red**

Run: `cargo test -p backend-rust model_provider_media_transport_adapter --offline`

Expected: FAIL because media transport symbols do not exist yet.

---

### Task 2: Implement Media Image Transport Adapter

**Files:**
- Modify: `backend/src/application/ai/model_provider_transport.rs`
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Creates `pub(super) struct ModelProviderMediaImageRequest<'a>`.
- Creates `pub(super) async fn send_model_provider_media_image_request(request: ModelProviderMediaImageRequest<'_>) -> Result<ModelMediaImageGenerationResp, AppError>`.
- `ModelRuntimeService::generate_media_image` remains public and returns `ModelMediaImageGenerationResp`, but delegates transport work to the adapter.

- [ ] **Step 1: Add transport request type**

Add to `model_provider_transport.rs`:

```rust
pub(super) struct ModelProviderMediaImageRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub request: &'a MediaImageGenerationRequest,
    pub timeout: Duration,
}
```

- [ ] **Step 2: Add transport dispatch function**

Add to `model_provider_transport.rs`:

```rust
pub(super) async fn send_model_provider_media_image_request(
    request: ModelProviderMediaImageRequest<'_>,
) -> Result<ModelMediaImageGenerationResp, AppError> {
    let request_payload = request.request.to_provider_payload();
    let client = reqwest::Client::builder()
        .timeout(request.timeout)
        .build()
        .map_err(|err| AppError::bad_request(format!("图片生成客户端初始化失败: {err}")))?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .header("x-api-key", request.api_key)
        .json(&request_payload)
        .send()
        .await
        .map_err(|err| AppError::bad_request(format!("图片生成请求失败: {err}")))?;
    let status = response.status();
    let provider_payload = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !status.is_success() {
        return Err(AppError::bad_request(format!(
            "图片生成请求失败: HTTP {}",
            status.as_u16()
        )));
    }
    let Some(result) = parse_media_image_generation_response(&provider_payload) else {
        return Err(AppError::bad_request("图片生成响应缺少资产 URL"));
    };

    Ok(ModelMediaImageGenerationResp {
        provider_payload,
        asset_url: result.asset_url,
        provider_asset_id: result.provider_asset_id,
    })
}
```

- [ ] **Step 3: Wire model service**

Import the media adapter symbols in `model_service.rs`, remove the service-level `parse_media_image_generation_response` import, then replace the body of `generate_media_image` with:

```rust
send_model_provider_media_image_request(ModelProviderMediaImageRequest {
    endpoint: route.endpoint(),
    api_key: route.api_key(),
    request,
    timeout: MODEL_MEDIA_IMAGE_TIMEOUT,
})
.await
```

- [ ] **Step 4: Verify green**

Run:

```bash
cargo test -p backend-rust model_provider_media_transport_adapter --offline
cargo test -p backend-rust media_ --offline
```

Expected: all commands pass.

---

### Task 3: Update Matrix And Regression Gates

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Runtime loop matrix status advances from `slice-64 implemented` to `slice-65 implemented`.
- Runtime loop notes mention media image provider HTTP dispatch and response parsing through `model_provider_transport`.
- Runtime loop POC evidence mentions configured media generation provider calls through the backend-local transport adapter.
- Acceptance evidence includes `model_provider_media_transport_adapter`.
- Remaining gap narrows to provider-specific client modules and later provider-client crate split.

- [ ] **Step 1: Update migration matrix**

Add this plan to Follow-up Implementation Plans:

```markdown
- Agent provider media transport adapter: `docs/plans/2026-06-17-agent-provider-media-transport-adapter.md`
```

- [ ] **Step 2: Full verification**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend-rust model_provider_media_transport_adapter --offline
cargo test -p backend-rust media_ --offline
cargo test -p backend-rust model_provider_rag_transport_adapter --offline
cargo test -p backend-rust runtime_embedding --offline
cargo test -p backend-rust rerank_ --offline
cargo test -p backend-rust model_provider_native_cancel_transport_adapter --offline
cargo test -p backend-rust model_provider_response_transport_adapter --offline
cargo test -p backend-rust model_provider_http_transport_adapter --offline
cargo test --workspace --offline
```

Expected: all commands pass.
