# Agent Provider Client Media Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move media image provider HTTP dispatch from backend-local transport into `novex-provider-client`.

**Architecture:** `novex-provider-client` owns the media image request DTO, provider payload conversion, reqwest client construction, bearer plus `x-api-key` auth, JSON body reading, HTTP status mapping, provider asset parsing, and returned `ModelMediaImageGenerationResp`. Backend `model_provider_transport/media.rs` remains the compatibility adapter that maps `ModelProviderClientError` into `AppError`; route resolution, provider-call leases, media trace/eval metadata, tenant context, and Agent media job persistence stay in backend.

**Tech Stack:** Rust 2021, Cargo workspace, `reqwest`, `serde_json`, `novex-model`, `novex-tools`, backend source-contract tests, provider-client crate unit tests.

## Global Constraints

- Preserve existing `model_service.rs` imports and call sites through `model_provider_transport`.
- Do not change media image provider payload shape; keep using `MediaImageGenerationRequest::to_provider_payload()`.
- Do not change provider auth behavior; keep both bearer auth and `x-api-key` header.
- Do not change Chinese media error messages, timeout use, asset parsing behavior, provider-call leases, source metadata, trace/eval metadata, media job persistence, or dry-run behavior.
- `novex-provider-client` must not depend on `backend`, backend `AppError`, SQL, tenant context, provider-call leases, run-event persistence, media job persistence, or trace/eval crates.
- Backend remains responsible for converting `ModelProviderClientError` into `AppError`.
- Use a RED source-contract test before moving production code.
- Verify with focused media/provider-client tests, formatting, diff checks, and the offline workspace test suite.
- Merge this phase back to `main`, sync the feature worktree, and run `cargo clean` in both worktrees.

---

### Task 1: Provider Client Media Dispatch

**Files:**
- Modify: `crates/novex-provider-client/Cargo.toml`
- Modify: `crates/novex-provider-client/src/lib.rs`
- Modify: `backend/src/application/ai/model_provider_transport/media.rs`
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Test: `crates/novex-provider-client/src/lib.rs`
- Test: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Consumes: `novex_tools::{MediaImageGenerationRequest, parse_media_image_generation_response}`, existing provider-client `model_provider_http_client(timeout)`, and `ModelProviderClientError`.
- Produces: `novex_provider_client::{ModelProviderMediaImageRequest, send_model_provider_media_image_request}` returning `Result<ModelMediaImageGenerationResp, ModelProviderClientError>`.

- [ ] **Step 1: Write the failing source-contract test**

Add a backend test named `provider_client_media_dispatch_lives_in_provider_client_crate` that reads `crates/novex-provider-client/Cargo.toml`, `crates/novex-provider-client/src/lib.rs`, `backend/src/application/ai/model_provider_transport/http.rs`, and `backend/src/application/ai/model_provider_transport/media.rs`. The test must assert:

```rust
assert!(provider_cargo_source.contains("novex-tools.workspace = true"));
assert!(provider_client_source.contains("use novex_tools::{parse_media_image_generation_response, MediaImageGenerationRequest};"));
assert!(provider_client_source.contains("pub struct ModelProviderMediaImageRequest"));
assert!(provider_client_source.contains("pub async fn send_model_provider_media_image_request"));
assert!(provider_client_source.contains("let request_payload = request.request.to_provider_payload();"));
assert!(provider_client_source.contains("model_provider_http_client(request.timeout)"));
assert!(provider_client_source.contains(".post(request.endpoint)"));
assert!(provider_client_source.contains(".bearer_auth(request.api_key)"));
assert!(provider_client_source.contains(".header(\"x-api-key\", request.api_key)"));
assert!(provider_client_source.contains(".json(&request_payload)"));
assert!(provider_client_source.contains("parse_media_image_generation_response(&provider_payload)"));
assert!(provider_client_source.contains("图片生成客户端初始化失败"));
assert!(provider_client_source.contains("图片生成请求失败"));
assert!(provider_client_source.contains("图片生成响应缺少资产 URL"));
assert!(backend_http_source.contains("pub(super) fn model_provider_client_error_to_app_error"));
assert!(backend_media_source.contains("pub(in crate::application::ai) use novex_provider_client::ModelProviderMediaImageRequest"));
assert!(backend_media_source.contains("novex_provider_client::send_model_provider_media_image_request(request)"));
assert!(backend_media_source.contains("model_provider_client_error_to_app_error"));
assert!(!backend_media_source.contains("MediaImageGenerationRequest"));
assert!(!backend_media_source.contains("parse_media_image_generation_response"));
assert!(!backend_media_source.contains("serde_json::{json, Value}"));
assert!(!backend_media_source.contains("model_provider_http_client(request.timeout)"));
assert!(!backend_media_source.contains(".post(request.endpoint)"));
assert!(!backend_media_source.contains(".header(\"x-api-key\", request.api_key)"));
assert!(!backend_media_source.contains("图片生成请求失败: HTTP"));
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p backend provider_client_media_dispatch_lives_in_provider_client_crate --offline`

Expected: FAIL because backend `media.rs` still owns the request DTO, payload conversion, provider POST dispatch, dual auth headers, JSON body reading, HTTP status mapping, provider asset parser call, and media error strings.

- [ ] **Step 3: Add provider-client dependency on novex-tools**

Add to `crates/novex-provider-client/Cargo.toml`:

```toml
novex-tools.workspace = true
```

- [ ] **Step 4: Move media DTO and dispatch into provider-client**

Add to `crates/novex-provider-client/src/lib.rs`:

```rust
use novex_model::{ModelEmbeddingVector, ModelMediaImageGenerationResp, ModelRerankScore};
use novex_tools::{parse_media_image_generation_response, MediaImageGenerationRequest};
```

Add the request DTO and dispatch function:

```rust
pub struct ModelProviderMediaImageRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub request: &'a MediaImageGenerationRequest,
    pub timeout: Duration,
}

pub async fn send_model_provider_media_image_request(
    request: ModelProviderMediaImageRequest<'_>,
) -> Result<ModelMediaImageGenerationResp, ModelProviderClientError> {
    let request_payload = request.request.to_provider_payload();
    let client = model_provider_http_client(request.timeout).map_err(|err| {
        ModelProviderClientError::BadResponse(format!("图片生成客户端初始化失败: {err}"))
    })?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .header("x-api-key", request.api_key)
        .json(&request_payload)
        .send()
        .await
        .map_err(|err| ModelProviderClientError::BadResponse(format!("图片生成请求失败: {err}")))?;
    let status = response.status();
    let provider_payload = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !status.is_success() {
        return Err(ModelProviderClientError::BadResponse(format!(
            "图片生成请求失败: HTTP {}",
            status.as_u16()
        )));
    }
    let Some(result) = parse_media_image_generation_response(&provider_payload) else {
        return Err(ModelProviderClientError::BadResponse(
            "图片生成响应缺少资产 URL".to_owned(),
        ));
    };

    Ok(ModelMediaImageGenerationResp {
        provider_payload,
        asset_url: result.asset_url,
        provider_asset_id: result.provider_asset_id,
    })
}
```

- [ ] **Step 5: Add provider-client unit tests for media request and response parsing**

Add tests in `crates/novex-provider-client/src/lib.rs`:

```rust
#[test]
fn media_image_request_carries_provider_dispatch_inputs() {
    let media_request = MediaImageGenerationRequest::new("draw a support diagram")
        .with_size("1024x1024")
        .with_count(2);
    let request = ModelProviderMediaImageRequest {
        endpoint: "https://provider.example/v1/images/generations",
        api_key: "secret",
        request: &media_request,
        timeout: Duration::from_secs(45),
    };

    assert_eq!(request.endpoint, "https://provider.example/v1/images/generations");
    assert_eq!(request.api_key, "secret");
    assert_eq!(request.timeout, Duration::from_secs(45));
    assert_eq!(request.request.to_provider_payload()["prompt"], "draw a support diagram");
    assert_eq!(request.request.to_provider_payload()["size"], "1024x1024");
    assert_eq!(request.request.to_provider_payload()["n"], 2);
}

#[test]
fn media_image_parser_dependency_maps_provider_asset_payload() {
    let provider_payload = json!({
        "id": "asset_123",
        "data": [{"url": "https://cdn.example/image.png"}]
    });

    let result = parse_media_image_generation_response(&provider_payload)
        .expect("provider payload should expose an image URL");

    assert_eq!(result.asset_url, "https://cdn.example/image.png");
    assert_eq!(result.provider_asset_id.as_deref(), Some("asset_123"));
}
```

- [ ] **Step 6: Replace backend media transport with thin adapter**

Replace `backend/src/application/ai/model_provider_transport/media.rs` with:

```rust
use novex_model::ModelMediaImageGenerationResp;
pub(in crate::application::ai) use novex_provider_client::ModelProviderMediaImageRequest;

use super::http::model_provider_client_error_to_app_error;
use crate::shared::error::AppError;

pub(in crate::application::ai) async fn send_model_provider_media_image_request(
    request: ModelProviderMediaImageRequest<'_>,
) -> Result<ModelMediaImageGenerationResp, AppError> {
    novex_provider_client::send_model_provider_media_image_request(request)
        .await
        .map_err(model_provider_client_error_to_app_error)
}
```

- [ ] **Step 7: Update existing source-contract tests and migration matrix**

Update existing media/provider transport source-contract tests so they expect backend `media.rs` to call the provider-client dispatch and no longer own request payload conversion, `model_provider_http_client(request.timeout)`, `x-api-key`, JSON response parsing, or media parser calls. Update the migration matrix runtime-loop notes and acceptance evidence to say `novex-provider-client` owns media image provider dispatch APIs, leaving chat/stream/compaction dispatch as backend-local adapter work.

- [ ] **Step 8: Verify focused and full workspace**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend provider_client_media_dispatch_lives_in_provider_client_crate --offline
cargo test -p backend model_provider_media_transport_adapter --offline
cargo test -p backend model_provider_transport_splits_provider_client_modules --offline
cargo test -p backend media_ --offline
cargo test -p backend provider_client_http_primitives_live_in_provider_client_crate --offline
cargo test -p backend provider_client_native_cancel_dispatch_lives_in_provider_client_crate --offline
cargo test -p backend provider_client_rag_dispatch_lives_in_provider_client_crate --offline
cargo test -p novex-provider-client --offline
cargo test --workspace --offline
```

Expected: all commands pass.

- [ ] **Step 9: Commit, merge, sync, clean**

Commit the plan before production changes:

```bash
git add docs/plans/2026-06-18-agent-provider-client-media-dispatch.md
git commit -m "docs: plan provider client media dispatch"
```

Commit implementation after verification:

```bash
git add crates/novex-provider-client/Cargo.toml crates/novex-provider-client/src/lib.rs backend/src/application/ai/model_provider_transport/media.rs backend/src/application/ai/model_service.rs docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-18-agent-provider-client-media-dispatch.md
git commit -m "feat: extract provider client media dispatch"
```

Merge the phase into `main`, fast-forward the feature worktree from `main`, then run `cargo clean` in both worktrees.
