# Agent Provider Client RAG Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move embedding and rerank provider dispatch APIs from backend-local RAG transport into `novex-provider-client`.

**Architecture:** `novex-provider-client` owns provider-neutral RAG request DTOs, request payload construction, bearer-auth POST dispatch, JSON response reading, HTTP status message shaping, empty-response checks, and parser reuse. Backend `model_provider_transport/rag.rs` remains the compatibility adapter that maps `ModelProviderClientError` to `AppError`; model route resolution, provider-call leases, trace/eval metadata, tenant context, and public model-service APIs stay in backend.

**Tech Stack:** Rust 2021, Cargo workspace, `reqwest`, `serde_json`, `novex-model`, backend source-contract tests, provider-client crate unit tests.

## Global Constraints

- Preserve existing `model_service.rs` imports and call sites through `model_provider_transport`.
- Do not change embedding/rerank request payload shapes, timeouts, bearer-auth behavior, parser behavior, or Chinese error messages.
- `novex-provider-client` must not depend on `backend-rust`, backend `AppError`, SQL, tenant context, provider-call leases, or run-event persistence.
- Backend remains responsible for converting `ModelProviderClientError` into `AppError`.
- Keep parser facade re-exports from `model_provider_transport.rs` stable for existing tests and service helpers.
- Use a RED source-contract test before moving production code.
- Verify with focused RAG/provider-client tests, formatting, diff checks, and the offline workspace test suite.

---

### Task 1: Provider Client RAG Dispatch

**Files:**
- Modify: `crates/novex-provider-client/src/lib.rs`
- Modify: `backend/src/application/ai/model_provider_transport/http.rs`
- Modify: `backend/src/application/ai/model_provider_transport/rag.rs`
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Test: `crates/novex-provider-client/src/lib.rs`
- Test: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Consumes: existing provider-client parser functions `parse_model_provider_embedding_vectors` and `parse_model_provider_rerank_scores`, plus `model_provider_http_client(timeout)`.
- Produces: `novex_provider_client::{ModelProviderEmbeddingRequest, ModelProviderRerankRequest, send_model_provider_embedding_request, send_model_provider_rerank_request}` returning `Result<Vec<ModelEmbeddingVector>, ModelProviderClientError>` and `Result<Vec<ModelRerankScore>, ModelProviderClientError>`.

- [ ] **Step 1: Write the failing source-contract test**

Add a backend test named `provider_client_rag_dispatch_lives_in_provider_client_crate` that reads `crates/novex-provider-client/src/lib.rs`, `backend/src/application/ai/model_provider_transport/http.rs`, and `backend/src/application/ai/model_provider_transport/rag.rs`. The test must assert:

```rust
assert!(provider_client_source.contains("pub struct ModelProviderEmbeddingRequest"));
assert!(provider_client_source.contains("pub struct ModelProviderRerankRequest"));
assert!(provider_client_source.contains("pub async fn send_model_provider_embedding_request"));
assert!(provider_client_source.contains("pub async fn send_model_provider_rerank_request"));
assert!(provider_client_source.contains("\"model\": request.model.unwrap_or_default()"));
assert!(provider_client_source.contains("\"input\": request.texts"));
assert!(provider_client_source.contains("\"query\": request.query"));
assert!(provider_client_source.contains("\"documents\": request.documents"));
assert!(provider_client_source.contains("parse_model_provider_embedding_vectors(&body)"));
assert!(provider_client_source.contains("parse_model_provider_rerank_scores(&body)"));
assert!(provider_client_source.contains("ModelProviderClientError::BadResponse"));
assert!(provider_client_source.contains("Embedding 模型响应为空"));
assert!(provider_client_source.contains("Rerank 模型响应为空"));
assert!(backend_http_source.contains("pub(super) fn model_provider_client_error_to_app_error"));
assert!(backend_rag_source.contains("pub(in crate::application::ai) use novex_provider_client::{"));
assert!(backend_rag_source.contains("ModelProviderEmbeddingRequest"));
assert!(backend_rag_source.contains("ModelProviderRerankRequest"));
assert!(backend_rag_source.contains("novex_provider_client::send_model_provider_embedding_request(request)"));
assert!(backend_rag_source.contains("novex_provider_client::send_model_provider_rerank_request(request)"));
assert!(backend_rag_source.contains("model_provider_client_error_to_app_error"));
assert!(!backend_rag_source.contains("serde_json::{json, Value}"));
assert!(!backend_rag_source.contains("model_provider_http_client(request.timeout)"));
assert!(!backend_rag_source.contains(".post(request.endpoint)"));
assert!(!backend_rag_source.contains("Embedding 模型调用失败: {status}"));
assert!(!backend_rag_source.contains("Rerank 模型调用失败: {status}"));
assert!(!backend_rag_source.contains("Embedding 模型响应为空"));
assert!(!backend_rag_source.contains("Rerank 模型响应为空"));
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p backend-rust provider_client_rag_dispatch_lives_in_provider_client_crate --offline`

Expected: FAIL because backend `rag.rs` still owns `ModelProviderEmbeddingRequest`, `ModelProviderRerankRequest`, provider POST dispatch, JSON response reading, HTTP status mapping, and empty response errors.

- [ ] **Step 3: Extend provider-client error type**

Add a bad provider response variant to `ModelProviderClientError`:

```rust
BadResponse(String),
```

Update `Display` and `Error::source`:

```rust
Self::BadResponse(message) => write!(f, "{message}"),
Self::BadResponse(_) => None,
```

Update backend `model_provider_client_error_to_app_error` to map `BadResponse(message)` to `AppError::bad_request(message)`.

- [ ] **Step 4: Move RAG request DTOs and dispatch into provider-client**

Add to `crates/novex-provider-client/src/lib.rs`:

```rust
pub struct ModelProviderEmbeddingRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub model: Option<&'a str>,
    pub texts: &'a [String],
    pub timeout: Duration,
}

pub struct ModelProviderRerankRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub model: Option<&'a str>,
    pub query: &'a str,
    pub documents: &'a [String],
    pub timeout: Duration,
}

pub async fn send_model_provider_embedding_request(
    request: ModelProviderEmbeddingRequest<'_>,
) -> Result<Vec<ModelEmbeddingVector>, ModelProviderClientError> {
    let client =
        model_provider_http_client(request.timeout).map_err(ModelProviderClientError::Transport)?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .json(&json!({
            "model": request.model.unwrap_or_default(),
            "input": request.texts,
        }))
        .send()
        .await
        .map_err(ModelProviderClientError::Transport)?;
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(ModelProviderClientError::BadResponse(format!(
            "Embedding 模型调用失败: {status}"
        )));
    }
    let vectors = parse_model_provider_embedding_vectors(&body);
    if vectors.is_empty() {
        return Err(ModelProviderClientError::BadResponse(
            "Embedding 模型响应为空".to_owned(),
        ));
    }
    Ok(vectors)
}

pub async fn send_model_provider_rerank_request(
    request: ModelProviderRerankRequest<'_>,
) -> Result<Vec<ModelRerankScore>, ModelProviderClientError> {
    let client =
        model_provider_http_client(request.timeout).map_err(ModelProviderClientError::Transport)?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .json(&json!({
            "model": request.model.unwrap_or_default(),
            "query": request.query,
            "documents": request.documents,
        }))
        .send()
        .await
        .map_err(ModelProviderClientError::Transport)?;
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(ModelProviderClientError::BadResponse(format!(
            "Rerank 模型调用失败: {status}"
        )));
    }
    let scores = parse_model_provider_rerank_scores(&body);
    if scores.is_empty() {
        return Err(ModelProviderClientError::BadResponse(
            "Rerank 模型响应为空".to_owned(),
        ));
    }
    Ok(scores)
}
```

- [ ] **Step 5: Add provider-client unit tests for request shape and bad-response display**

Add tests in `crates/novex-provider-client/src/lib.rs`:

```rust
#[test]
fn bad_response_error_preserves_provider_message() {
    let error = ModelProviderClientError::BadResponse("Embedding 模型响应为空".to_owned());

    assert_eq!(error.to_string(), "Embedding 模型响应为空");
}

#[test]
fn rag_requests_carry_provider_dispatch_inputs() {
    let texts = vec!["alpha".to_owned(), "beta".to_owned()];
    let embedding = ModelProviderEmbeddingRequest {
        endpoint: "https://provider.example/v1/embeddings",
        api_key: "secret",
        model: Some("embed-demo"),
        texts: &texts,
        timeout: Duration::from_secs(20),
    };

    assert_eq!(embedding.endpoint, "https://provider.example/v1/embeddings");
    assert_eq!(embedding.api_key, "secret");
    assert_eq!(embedding.model, Some("embed-demo"));
    assert_eq!(embedding.texts, &texts);
    assert_eq!(embedding.timeout, Duration::from_secs(20));

    let documents = vec!["doc-a".to_owned(), "doc-b".to_owned()];
    let rerank = ModelProviderRerankRequest {
        endpoint: "https://provider.example/v1/rerank",
        api_key: "secret",
        model: Some("rerank-demo"),
        query: "question",
        documents: &documents,
        timeout: Duration::from_secs(30),
    };

    assert_eq!(rerank.endpoint, "https://provider.example/v1/rerank");
    assert_eq!(rerank.api_key, "secret");
    assert_eq!(rerank.model, Some("rerank-demo"));
    assert_eq!(rerank.query, "question");
    assert_eq!(rerank.documents, &documents);
    assert_eq!(rerank.timeout, Duration::from_secs(30));
}
```

- [ ] **Step 6: Replace backend RAG transport with thin adapter**

Replace `backend/src/application/ai/model_provider_transport/rag.rs` with:

```rust
use novex_model::{ModelEmbeddingVector, ModelRerankScore};
pub(in crate::application::ai) use novex_provider_client::{
    ModelProviderEmbeddingRequest, ModelProviderRerankRequest,
};

use super::http::model_provider_client_error_to_app_error;
use crate::shared::error::AppError;

pub(in crate::application::ai) async fn send_model_provider_embedding_request(
    request: ModelProviderEmbeddingRequest<'_>,
) -> Result<Vec<ModelEmbeddingVector>, AppError> {
    novex_provider_client::send_model_provider_embedding_request(request)
        .await
        .map_err(model_provider_client_error_to_app_error)
}

pub(in crate::application::ai) async fn send_model_provider_rerank_request(
    request: ModelProviderRerankRequest<'_>,
) -> Result<Vec<ModelRerankScore>, AppError> {
    novex_provider_client::send_model_provider_rerank_request(request)
        .await
        .map_err(model_provider_client_error_to_app_error)
}
```

- [ ] **Step 7: Update existing source-contract tests**

Update `model_provider_rag_transport_adapter_source_contract`, `model_provider_transport_splits_provider_client_modules`, and `provider_client_rag_parsers_live_in_provider_client_crate` so they assert:

```rust
assert!(provider_client_source.contains("pub struct ModelProviderEmbeddingRequest"));
assert!(provider_client_source.contains("pub struct ModelProviderRerankRequest"));
assert!(provider_client_source.contains("pub async fn send_model_provider_embedding_request"));
assert!(provider_client_source.contains("pub async fn send_model_provider_rerank_request"));
assert!(rag_source.contains("pub(in crate::application::ai) use novex_provider_client::{"));
assert!(rag_source.contains("ModelProviderEmbeddingRequest"));
assert!(rag_source.contains("ModelProviderRerankRequest"));
assert!(rag_source.contains("model_provider_client_error_to_app_error"));
assert!(!rag_source.contains("serde_json::{json, Value}"));
assert!(!rag_source.contains("model_provider_http_client(request.timeout)"));
```

- [ ] **Step 8: Update migration matrix**

Change the runtime-loop notes to say `novex-provider-client` owns RAG response parsers, reusable HTTP primitives, and embedding/rerank dispatch APIs; native-cancel and media dispatch extraction remain next.

- [ ] **Step 9: Run focused verification**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend-rust provider_client_rag_dispatch_lives_in_provider_client_crate --offline
cargo test -p backend-rust provider_client_rag_parsers_live_in_provider_client_crate --offline
cargo test -p backend-rust provider_client_http_primitives_live_in_provider_client_crate --offline
cargo test -p backend-rust model_provider_rag_transport_adapter --offline
cargo test -p backend-rust model_provider_transport_splits_provider_client_modules --offline
cargo test -p backend-rust runtime_embedding --offline
cargo test -p backend-rust rerank_ --offline
cargo test -p novex-provider-client --offline
```

- [ ] **Step 10: Run full workspace verification**

Run: `cargo test --workspace --offline`

- [ ] **Step 11: Commit**

Commit docs and code with:

```bash
git add backend/src/application/ai/model_provider_transport/http.rs backend/src/application/ai/model_provider_transport/rag.rs backend/src/application/ai/model_service.rs crates/novex-provider-client/src/lib.rs docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-provider-client-rag-dispatch.md
git commit -m "feat: extract provider client rag dispatch"
```
