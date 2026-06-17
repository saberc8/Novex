# Agent Provider Transport Types Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move provider transport output DTOs out of the large backend model service and into the shared model crate so provider transport can later become dedicated provider-client modules/crates.

**Architecture:** `crates/novex-model` owns provider transport value types that are independent of database/service orchestration. `backend/src/application/ai/model_service.rs` re-exports those types for existing backend callers, while `model_provider_transport.rs` imports them directly from `novex_model`.

**Tech Stack:** Rust 2021, Cargo workspace, `serde`, `serde_json`, backend source-contract tests.

## Global Constraints

- Keep the existing backend public API shape stable by re-exporting moved DTOs from `model_service.rs`.
- Do not move provider HTTP dispatch or parsing behavior in this slice.
- Use source-contract tests first, then minimal implementation.
- Verify with focused backend tests, `novex-model` tests, formatting, diff checks, and the offline workspace test suite.

---

### Task 1: Provider Transport DTO Boundary

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `backend/src/application/ai/model_provider_transport.rs`
- Modify: `crates/novex-model/src/lib.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Test: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Consumes: existing backend DTO names `ModelProviderStreamChunk`, `ModelEmbeddingVector`, `ModelRerankScore`, and `ModelMediaImageGenerationResp`.
- Produces: `novex_model::{ModelProviderStreamChunk, ModelEmbeddingVector, ModelRerankScore, ModelMediaImageGenerationResp}` plus `pub use` compatibility from `model_service.rs`.

- [ ] **Step 1: Write the failing source-contract test**

Add a backend test named `model_provider_transport_types_live_in_shared_model_crate` that reads `model_service.rs`, `model_provider_transport.rs`, and `crates/novex-model/src/lib.rs`. The test must assert:

```rust
assert!(model_source.contains("pub struct ModelProviderStreamChunk"));
assert!(model_source.contains("pub struct ModelEmbeddingVector"));
assert!(model_source.contains("pub struct ModelRerankScore"));
assert!(model_source.contains("pub struct ModelMediaImageGenerationResp"));
assert!(service_source.contains("pub use novex_model::{"));
assert!(!service_source.contains("pub struct ModelProviderStreamChunk"));
assert!(!service_source.contains("pub struct ModelEmbeddingVector"));
assert!(!service_source.contains("pub struct ModelRerankScore"));
assert!(!service_source.contains("pub struct ModelMediaImageGenerationResp"));
assert!(transport_source.contains("use novex_model::{"));
assert!(!transport_source.contains("use super::model_service::{"));
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p backend-rust model_provider_transport_types_live_in_shared_model_crate --offline`

Expected: FAIL because `novex-model` does not yet own those DTO structs and transport still imports them from `model_service`.

- [ ] **Step 3: Move DTO definitions**

Add these definitions to `crates/novex-model/src/lib.rs`:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProviderStreamChunk {
    pub index: usize,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_event: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMediaImageGenerationResp {
    pub provider_payload: Value,
    pub asset_url: String,
    pub provider_asset_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelRerankScore {
    pub index: usize,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelEmbeddingVector {
    pub index: usize,
    pub vector: Vec<f32>,
}
```

- [ ] **Step 4: Update backend imports and compatibility**

Remove the local DTO definitions from `model_service.rs`, add:

```rust
pub use novex_model::{
    ModelEmbeddingVector, ModelMediaImageGenerationResp, ModelProviderStreamChunk,
    ModelRerankScore,
};
```

Update `model_provider_transport.rs` to import those DTOs directly from `novex_model` together with `normalize_model_provider_usage` and `ModelTokenUsage`.

- [ ] **Step 5: Update migration matrix**

Change the runtime-loop notes to say provider transport value DTOs now live in `novex-model`, and that provider-specific client modules/crates remain next.

- [ ] **Step 6: Run focused verification**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend-rust model_provider_transport_types_live_in_shared_model_crate --offline
cargo test -p backend-rust model_provider_media_transport_adapter --offline
cargo test -p backend-rust model_provider_rag_transport_adapter --offline
cargo test -p novex-model --offline
```

- [ ] **Step 7: Run full workspace verification**

Run: `cargo test --workspace --offline`

- [ ] **Step 8: Commit**

Commit docs and code with:

```bash
git add backend/src/application/ai/model_service.rs backend/src/application/ai/model_provider_transport.rs crates/novex-model/src/lib.rs docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-provider-transport-types.md
git commit -m "feat: move provider transport types into model crate"
```
