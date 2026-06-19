# Agent Provider Client Crate RAG Parsers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a dedicated `novex-provider-client` workspace crate and move provider RAG response parsing into it as the first provider-client crate extraction slice.

**Architecture:** Keep backend HTTP dispatch in `backend/src/application/ai/model_provider_transport/rag.rs` for now because it still maps backend `AppError` messages and timeouts. Move provider-neutral embedding/rerank response parsers into `crates/novex-provider-client`, returning shared `novex-model` DTOs. The backend RAG transport adapter imports those parser functions from the crate and keeps the existing facade names stable for `model_service.rs`.

**Tech Stack:** Rust 2021, Cargo workspace, `serde_json`, `novex-model`, backend source-contract tests, crate unit tests.

## Global Constraints

- Do not change embedding/rerank provider request payloads, error messages, or backend public facade names.
- `crates/novex-provider-client` must not depend on `backend` or backend `AppError`.
- Keep parser behavior identical: support `results`/`data`, `index`/`document_index`/`documentIndex`, `relevance_score`/`relevanceScore`/`score`, numeric strings, finite filtering, and empty vector rejection.
- Use a RED source-contract test before creating the crate.
- Verify with focused backend tests, provider-client crate tests, formatting, diff checks, and the offline workspace test suite.

---

### Task 1: Provider Client Crate For RAG Parsers

**Files:**
- Modify: `Cargo.toml`
- Modify: `backend/Cargo.toml`
- Create: `crates/novex-provider-client/Cargo.toml`
- Create: `crates/novex-provider-client/src/lib.rs`
- Modify: `backend/src/application/ai/model_provider_transport/rag.rs`
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Test: `backend/src/application/ai/model_service.rs`
- Test: `crates/novex-provider-client/src/lib.rs`

**Interfaces:**
- Consumes: existing backend parser facade names `parse_model_provider_embedding_vectors` and `parse_model_provider_rerank_scores`.
- Produces: `novex_provider_client::{parse_model_provider_embedding_vectors, parse_model_provider_rerank_scores}` returning `Vec<ModelEmbeddingVector>` and `Vec<ModelRerankScore>`.

- [ ] **Step 1: Write the failing source-contract test**

Add a backend test named `provider_client_rag_parsers_live_in_provider_client_crate` that reads `Cargo.toml`, `backend/Cargo.toml`, `backend/src/application/ai/model_provider_transport/rag.rs`, and `crates/novex-provider-client/src/lib.rs` at runtime. The test must assert:

```rust
assert!(workspace_source.contains("\"crates/novex-provider-client\""));
assert!(workspace_source.contains("novex-provider-client = { path = \"crates/novex-provider-client\" }"));
assert!(backend_cargo_source.contains("novex-provider-client.workspace = true"));
assert!(provider_client_source.contains("pub fn parse_model_provider_embedding_vectors"));
assert!(provider_client_source.contains("pub fn parse_model_provider_rerank_scores"));
assert!(provider_client_source.contains("fn parse_embedding_vector("));
assert!(provider_client_source.contains("fn parse_rerank_score("));
assert!(rag_source.contains("use novex_provider_client::{"));
assert!(rag_source.contains("parse_model_provider_embedding_vectors"));
assert!(rag_source.contains("parse_model_provider_rerank_scores"));
assert!(!rag_source.contains("fn parse_embedding_vector("));
assert!(!rag_source.contains("fn parse_rerank_score("));
assert!(!rag_source.contains("fn json_usize("));
assert!(!rag_source.contains("fn json_f32("));
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p backend provider_client_rag_parsers_live_in_provider_client_crate --offline`

Expected: FAIL because the workspace crate does not exist and backend RAG transport still owns parser helpers.

- [ ] **Step 3: Create crate and workspace dependencies**

Add `crates/novex-provider-client` to workspace members and workspace dependencies. Add `novex-provider-client.workspace = true` to `backend/Cargo.toml`.

- [ ] **Step 4: Move parser code into crate**

Create `crates/novex-provider-client/src/lib.rs` with:

```rust
use novex_model::{ModelEmbeddingVector, ModelRerankScore};
use serde_json::Value;

pub fn parse_model_provider_rerank_scores(body: &Value) -> Vec<ModelRerankScore> { /* existing behavior */ }
pub fn parse_model_provider_embedding_vectors(body: &Value) -> Vec<ModelEmbeddingVector> { /* existing behavior */ }
```

Include crate unit tests for DashScope rerank shapes and OpenAI-compatible embedding shapes.

- [ ] **Step 5: Update backend RAG adapter**

In `backend/src/application/ai/model_provider_transport/rag.rs`, remove local parser helpers and import:

```rust
use novex_provider_client::{
    parse_model_provider_embedding_vectors, parse_model_provider_rerank_scores,
};
```

Keep the existing backend facade re-export in `model_provider_transport.rs` unchanged.

- [ ] **Step 6: Update migration matrix**

Change the runtime-loop notes to say provider-client crate extraction has started with RAG response parsers in `novex-provider-client`; HTTP dispatch extraction remains next.

- [ ] **Step 7: Run focused verification**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend provider_client_rag_parsers_live_in_provider_client_crate --offline
cargo test -p backend model_provider_rag_transport_adapter --offline
cargo test -p backend runtime_embedding --offline
cargo test -p backend rerank_ --offline
cargo test -p novex-provider-client --offline
```

- [ ] **Step 8: Run full workspace verification**

Run: `cargo test --workspace --offline`

- [ ] **Step 9: Commit**

Commit docs and code with:

```bash
git add Cargo.toml backend/Cargo.toml backend/src/application/ai/model_service.rs backend/src/application/ai/model_provider_transport/rag.rs crates/novex-provider-client docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-provider-client-crate-rag-parsers.md
git commit -m "feat: extract provider client rag parsers"
```
