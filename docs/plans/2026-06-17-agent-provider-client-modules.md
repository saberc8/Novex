# Agent Provider Client Modules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split backend-local provider transport HTTP/cancel/RAG/media responsibilities into focused provider-client modules as the next step toward dedicated provider-client crates.

**Architecture:** `model_provider_transport.rs` remains the compatibility facade used by `model_service.rs`, but it delegates request DTOs and dispatch functions to file-level submodules. `http.rs` owns common reqwest client construction and generic POST dispatch, `native_cancel.rs` owns provider-native cancel dispatch, `rag.rs` owns embedding/rerank dispatch and parsing, and `media.rs` owns image generation dispatch and provider asset parsing.

**Tech Stack:** Rust 2021, Cargo workspace, reqwest, serde_json, backend source-contract tests.

## Global Constraints

- Preserve all existing `model_service.rs` imports and call sites through `pub(super) use` re-exports from `model_provider_transport.rs`.
- Do not change provider payload shapes, error messages, timeouts, or parser behavior in this slice.
- Keep chat response/SSE parsing in `model_provider_transport.rs` for now; only split request dispatch families that are already isolated.
- Use a RED source-contract test before moving production code.
- Verify with focused provider transport tests, formatting, diff checks, and the offline workspace test suite.

---

### Task 1: Provider Transport Submodules

**Files:**
- Modify: `backend/src/application/ai/model_provider_transport.rs`
- Create: `backend/src/application/ai/model_provider_transport/http.rs`
- Create: `backend/src/application/ai/model_provider_transport/native_cancel.rs`
- Create: `backend/src/application/ai/model_provider_transport/rag.rs`
- Create: `backend/src/application/ai/model_provider_transport/media.rs`
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Test: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Consumes: existing facade names `ModelProviderHttpRequest`, `send_model_provider_http_request`, `ModelProviderNativeCancelRequest`, `send_model_provider_native_cancel_request`, `ModelProviderEmbeddingRequest`, `send_model_provider_embedding_request`, `ModelProviderRerankRequest`, `send_model_provider_rerank_request`, `ModelProviderMediaImageRequest`, `send_model_provider_media_image_request`, `parse_model_provider_embedding_vectors`, and `parse_model_provider_rerank_scores`.
- Produces: the same facade names re-exported from `model_provider_transport.rs`, backed by focused submodule implementations.

- [ ] **Step 1: Write the failing source-contract test**

Add a backend test named `model_provider_transport_splits_provider_client_modules` that reads `model_provider_transport.rs` plus `model_provider_transport/http.rs`, `native_cancel.rs`, `rag.rs`, and `media.rs` at runtime using `std::fs::read_to_string`. The test must assert:

```rust
assert!(transport_source.contains("mod http;"));
assert!(transport_source.contains("mod media;"));
assert!(transport_source.contains("mod native_cancel;"));
assert!(transport_source.contains("mod rag;"));
assert!(transport_source.contains("pub(super) use http::{"));
assert!(transport_source.contains("pub(super) use media::{"));
assert!(transport_source.contains("pub(super) use native_cancel::{"));
assert!(transport_source.contains("pub(super) use rag::{"));
assert!(!transport_source.contains("pub(super) async fn send_model_provider_embedding_request"));
assert!(!transport_source.contains("pub(super) async fn send_model_provider_rerank_request"));
assert!(!transport_source.contains("pub(super) async fn send_model_provider_media_image_request"));
assert!(!transport_source.contains("pub(super) async fn send_model_provider_native_cancel_request"));
assert!(!transport_source.contains("fn parse_rerank_score("));
assert!(!transport_source.contains("fn parse_embedding_vector("));
assert!(http_source.contains("pub(super) fn model_provider_http_client"));
assert!(http_source.contains("pub(super) async fn send_model_provider_http_request"));
assert!(native_cancel_source.contains("pub(super) async fn send_model_provider_native_cancel_request"));
assert!(native_cancel_source.contains("model_provider_http_client(request.timeout)"));
assert!(rag_source.contains("pub(super) async fn send_model_provider_embedding_request"));
assert!(rag_source.contains("pub(super) async fn send_model_provider_rerank_request"));
assert!(rag_source.contains("pub(super) fn parse_model_provider_embedding_vectors"));
assert!(rag_source.contains("pub(super) fn parse_model_provider_rerank_scores"));
assert!(rag_source.contains("model_provider_http_client(request.timeout)"));
assert!(media_source.contains("pub(super) async fn send_model_provider_media_image_request"));
assert!(media_source.contains("model_provider_http_client(request.timeout)"));
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p backend-rust model_provider_transport_splits_provider_client_modules --offline`

Expected: FAIL because the provider client submodule files do not exist yet and the root transport file still owns these functions.

- [ ] **Step 3: Extract the generic HTTP module**

Create `http.rs` with `ModelProviderHttpRequest`, `model_provider_http_client(timeout)`, and `send_model_provider_http_request`. Re-export request/dispatch from `model_provider_transport.rs`.

- [ ] **Step 4: Extract native cancel, RAG, and media modules**

Move native cancel request/dispatch into `native_cancel.rs`, embedding/rerank request/dispatch/parser helpers into `rag.rs`, and media image request/dispatch into `media.rs`. Each dispatch function must use `super::http::model_provider_http_client(request.timeout)`.

- [ ] **Step 5: Keep facade compatibility**

In `model_provider_transport.rs`, add:

```rust
mod http;
mod media;
mod native_cancel;
mod rag;

pub(super) use http::{send_model_provider_http_request, ModelProviderHttpRequest};
pub(super) use media::{send_model_provider_media_image_request, ModelProviderMediaImageRequest};
pub(super) use native_cancel::{
    send_model_provider_native_cancel_request, ModelProviderNativeCancelRequest,
};
pub(super) use rag::{
    parse_model_provider_embedding_vectors, parse_model_provider_rerank_scores,
    send_model_provider_embedding_request, send_model_provider_rerank_request,
    ModelProviderEmbeddingRequest, ModelProviderRerankRequest,
};
```

- [ ] **Step 6: Update migration matrix**

Change the runtime-loop notes to say backend-local provider transport is now split into provider-client submodules for generic HTTP, native cancel, RAG, and media, while crate extraction remains next.

- [ ] **Step 7: Run focused verification**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend-rust model_provider_transport_splits_provider_client_modules --offline
cargo test -p backend-rust model_provider_http_transport_adapter --offline
cargo test -p backend-rust model_provider_native_cancel_transport_adapter --offline
cargo test -p backend-rust model_provider_rag_transport_adapter --offline
cargo test -p backend-rust model_provider_media_transport_adapter --offline
cargo test -p backend-rust model_provider_transport_types_live_in_shared_model_crate --offline
```

- [ ] **Step 8: Run full workspace verification**

Run: `cargo test --workspace --offline`

- [ ] **Step 9: Commit**

Commit docs and code with:

```bash
git add backend/src/application/ai/model_service.rs backend/src/application/ai/model_provider_transport.rs backend/src/application/ai/model_provider_transport docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-provider-client-modules.md
git commit -m "feat: split provider transport client modules"
```
