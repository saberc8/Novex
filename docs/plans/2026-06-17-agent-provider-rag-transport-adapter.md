# Agent Provider RAG Transport Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move embedding and rerank provider HTTP dispatch from `model_service.rs` into the backend-local provider transport adapter.

**Architecture:** Keep `model_service.rs` responsible for route resolution, provider-call leases, request metadata for trace/eval, and the public `embed_texts*` / `rerank_documents*` API. Add focused RAG transport APIs to `model_provider_transport.rs` that own reqwest client construction, POST dispatch, bearer auth, timeout handling, JSON body reading, HTTP status mapping, and embedding/rerank response parsing.

**Tech Stack:** Rust, reqwest, serde_json, backend model runtime service, source-contract tests, offline cargo tests.

## Global Constraints

- Do not change embedding request payload shape: `{ "model": route.model().unwrap_or_default(), "input": texts }`.
- Do not change rerank request payload shape: `{ "model": route.model().unwrap_or_default(), "query": query, "documents": documents }`.
- Do not change empty-input behavior: empty texts/documents return empty vectors/scores without provider calls.
- Do not change provider-call lease creation, heartbeat, completion payloads, or source metadata.
- Do not change RAG retrieval fallback behavior in `knowledge_service.rs`.
- Do not change media image generation in this slice; media gets its own transport adapter slice.
- This slice does not create a new crate. It tightens the backend-local adapter before a later provider-client crate split.

---

### Task 1: Add RAG Transport Source Contract

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Production `model_service.rs` imports `ModelProviderEmbeddingRequest`, `ModelProviderRerankRequest`, `send_model_provider_embedding_request`, `send_model_provider_rerank_request`, `parse_model_provider_embedding_vectors`, and `parse_model_provider_rerank_scores` from `model_provider_transport`.
- `ModelRuntimeService::embed_texts` delegates provider HTTP work to the embedding adapter.
- `ModelRuntimeService::rerank_documents` delegates provider HTTP work to the rerank adapter.
- `model_service.rs` no longer owns embedding/rerank reqwest client construction, bearer auth, JSON body reading, HTTP failure formatting, or private embedding/rerank parser helpers.

- [ ] **Step 1: Write failing source-contract test**

Add backend test `model_provider_rag_transport_adapter_source_contract` near the provider transport source-contract tests:

```rust
#[test]
fn model_provider_rag_transport_adapter_source_contract() {
    let service_source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    let transport_source = include_str!("model_provider_transport.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    let embed_path = &service_source[service_source
        .find("pub async fn embed_texts")
        .unwrap()
        ..service_source
            .find("pub async fn embed_texts_for_source")
            .unwrap()];
    let rerank_path = &service_source[service_source
        .find("pub async fn rerank_documents")
        .unwrap()
        ..service_source
            .find("pub async fn rerank_documents_for_source")
            .unwrap()];

    assert!(service_source.contains("ModelProviderEmbeddingRequest"));
    assert!(service_source.contains("ModelProviderRerankRequest"));
    assert!(service_source.contains("send_model_provider_embedding_request"));
    assert!(service_source.contains("send_model_provider_rerank_request"));
    assert!(service_source.contains("parse_model_provider_embedding_vectors"));
    assert!(service_source.contains("parse_model_provider_rerank_scores"));
    assert!(transport_source.contains("pub(super) struct ModelProviderEmbeddingRequest"));
    assert!(transport_source.contains("pub(super) struct ModelProviderRerankRequest"));
    assert!(transport_source.contains("pub(super) async fn send_model_provider_embedding_request"));
    assert!(transport_source.contains("pub(super) async fn send_model_provider_rerank_request"));
    assert!(transport_source.contains("pub(super) fn parse_model_provider_embedding_vectors"));
    assert!(transport_source.contains("pub(super) fn parse_model_provider_rerank_scores"));
    assert!(embed_path.contains("send_model_provider_embedding_request(ModelProviderEmbeddingRequest"));
    assert!(rerank_path.contains("send_model_provider_rerank_request(ModelProviderRerankRequest"));
    assert!(!embed_path.contains("reqwest::Client::builder()"));
    assert!(!embed_path.contains(".post(route.endpoint())"));
    assert!(!embed_path.contains(".bearer_auth(route.api_key())"));
    assert!(!embed_path.contains("Embedding 模型调用失败: {status}"));
    assert!(!rerank_path.contains("reqwest::Client::builder()"));
    assert!(!rerank_path.contains(".post(route.endpoint())"));
    assert!(!rerank_path.contains(".bearer_auth(route.api_key())"));
    assert!(!rerank_path.contains("Rerank 模型调用失败: {status}"));
    assert!(!service_source.contains("fn parse_rerank_score("));
    assert!(!service_source.contains("fn parse_embedding_vector("));
}
```

- [ ] **Step 2: Verify red**

Run: `cargo test -p backend model_provider_rag_transport_adapter --offline`

Expected: FAIL because RAG transport symbols do not exist yet.

---

### Task 2: Implement Embedding/Rerank Transport Adapter

**Files:**
- Modify: `backend/src/application/ai/model_provider_transport.rs`
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Creates `pub(super) struct ModelProviderEmbeddingRequest<'a>`.
- Creates `pub(super) struct ModelProviderRerankRequest<'a>`.
- Creates `pub(super) async fn send_model_provider_embedding_request(request: ModelProviderEmbeddingRequest<'_>) -> Result<Vec<ModelEmbeddingVector>, AppError>`.
- Creates `pub(super) async fn send_model_provider_rerank_request(request: ModelProviderRerankRequest<'_>) -> Result<Vec<ModelRerankScore>, AppError>`.
- Creates transport-owned parser functions `parse_model_provider_embedding_vectors` and `parse_model_provider_rerank_scores`.
- `ModelRuntimeService::parse_embedding_vectors` and `ModelRuntimeService::parse_rerank_scores` remain as public wrappers for existing tests/callers.

- [ ] **Step 1: Add request types**

Add to `model_provider_transport.rs`:

```rust
pub(super) struct ModelProviderEmbeddingRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub model: Option<&'a str>,
    pub texts: &'a [String],
    pub timeout: Duration,
}

pub(super) struct ModelProviderRerankRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub model: Option<&'a str>,
    pub query: &'a str,
    pub documents: &'a [String],
    pub timeout: Duration,
}
```

- [ ] **Step 2: Add transport dispatch functions**

Add to `model_provider_transport.rs`:

```rust
pub(super) async fn send_model_provider_embedding_request(
    request: ModelProviderEmbeddingRequest<'_>,
) -> Result<Vec<ModelEmbeddingVector>, AppError> {
    let client = reqwest::Client::builder()
        .timeout(request.timeout)
        .build()
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .json(&serde_json::json!({
            "model": request.model.unwrap_or_default(),
            "input": request.texts,
        }))
        .send()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(AppError::bad_request(format!(
            "Embedding 模型调用失败: {status}"
        )));
    }
    let vectors = parse_model_provider_embedding_vectors(&body);
    if vectors.is_empty() {
        return Err(AppError::bad_request("Embedding 模型响应为空"));
    }
    Ok(vectors)
}

pub(super) async fn send_model_provider_rerank_request(
    request: ModelProviderRerankRequest<'_>,
) -> Result<Vec<ModelRerankScore>, AppError> {
    let client = reqwest::Client::builder()
        .timeout(request.timeout)
        .build()
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .json(&serde_json::json!({
            "model": request.model.unwrap_or_default(),
            "query": request.query,
            "documents": request.documents,
        }))
        .send()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(AppError::bad_request(format!(
            "Rerank 模型调用失败: {status}"
        )));
    }
    let scores = parse_model_provider_rerank_scores(&body);
    if scores.is_empty() {
        return Err(AppError::bad_request("Rerank 模型响应为空"));
    }
    Ok(scores)
}
```

- [ ] **Step 3: Move parser helpers to transport**

Move `parse_rerank_score`, `parse_embedding_vector`, `json_usize`, and `json_f32` from `model_service.rs` into `model_provider_transport.rs`, then expose:

```rust
pub(super) fn parse_model_provider_rerank_scores(body: &Value) -> Vec<ModelRerankScore> {
    body.get("results")
        .or_else(|| body.get("data"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_rerank_score)
        .collect()
}

pub(super) fn parse_model_provider_embedding_vectors(body: &Value) -> Vec<ModelEmbeddingVector> {
    body.get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_embedding_vector)
        .collect()
}
```

- [ ] **Step 4: Wire model service**

Import the RAG adapter symbols in `model_service.rs`, then replace the bodies of `embed_texts` and `rerank_documents` with adapter calls while preserving empty-input behavior.

- [ ] **Step 5: Verify green**

Run:

```bash
cargo test -p backend model_provider_rag_transport_adapter --offline
cargo test -p backend runtime_embedding --offline
cargo test -p backend rerank_ --offline
```

Expected: all commands pass.

---

### Task 3: Update Matrix And Regression Gates

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Runtime loop matrix status advances from `slice-63 implemented` to `slice-64 implemented`.
- Runtime loop notes mention embedding/rerank provider HTTP dispatch and response parsing through `model_provider_transport`.
- Runtime loop POC evidence mentions configured RAG embedding/rerank provider calls through the backend-local transport adapter.
- Acceptance evidence includes `model_provider_rag_transport_adapter`.
- Remaining gap narrows to provider-specific client modules and media provider HTTP client extraction.

- [ ] **Step 1: Update migration matrix**

Add this plan to Follow-up Implementation Plans:

```markdown
- Agent provider RAG transport adapter: `docs/plans/2026-06-17-agent-provider-rag-transport-adapter.md`
```

- [ ] **Step 2: Full verification**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend model_provider_rag_transport_adapter --offline
cargo test -p backend runtime_embedding --offline
cargo test -p backend rerank_ --offline
cargo test -p backend model_provider_native_cancel_transport_adapter --offline
cargo test -p backend model_provider_response_transport_adapter --offline
cargo test -p backend model_provider_http_transport_adapter --offline
cargo test -p backend provider_call_lease_cancel --offline
cargo test --workspace --offline
```

Expected: all commands pass.
