# Agent Provider Client Focused Modules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split `novex-provider-client` from one large `lib.rs` into focused provider-client modules without changing the public API.

**Architecture:** Keep `crates/novex-provider-client/src/lib.rs` as a small crate facade with module declarations, public re-exports, and `CRATE_ID`. Move provider-neutral primitives into modules by responsibility: error vocabulary, HTTP transport, chat request planning, chat dispatch, chat/Responses parsing, compaction parsing, native cancel, media generation, and RAG embedding/rerank. Backend continues importing through the existing crate/facade names.

**Tech Stack:** Rust 2021, Cargo workspace, `reqwest`, `serde_json`, `novex-model`, `novex-tools`, provider-client unit tests, backend source-contract tests, offline cargo verification.

## Global Constraints

- Do not change public item names already consumed by backend: `ModelProviderClientError`, `ModelProviderHttpRequest`, `ModelProviderChatRequest`, `ModelProviderChatPlanInput`, `ModelProviderChatPlan`, `ModelProviderChatTransport`, `ModelChatProviderOutput`, `ModelChatCompactionProviderOutput`, `ModelChatStreamCompletionBuilder`, native cancel/media/RAG request DTOs, parser functions, dispatch functions, and response-id helpers.
- Do not move backend route resolution, tenant context, provider-call leases, trace/eval, stream event emission, or app error mapping into `novex-provider-client`.
- Keep provider-client modules independent of `backend-rust`, SQL, HTTP handlers, and Agent runtime state.
- Preserve all existing provider-client parser, dispatch, and planner behavior.
- Keep `lib.rs` as a facade; implementation modules must own their local unit tests.
- Verify with a RED source-contract test, provider-client full tests, focused backend provider-client boundary tests, formatting, diff checks, and the offline workspace suite.

---

### Task 1: Add Focused Module Source Contract

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Produces backend source-contract test `provider_client_crate_uses_focused_modules`.
- Consumes the future module files under `crates/novex-provider-client/src`.

- [x] **Step 1: Write the failing source-contract test**

Add this test near the existing provider-client source-contract tests:

```rust
#[test]
fn provider_client_crate_uses_focused_modules() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend manifest should live below workspace root");
    let source = |path: &str| {
        std::fs::read_to_string(workspace_root.join(path))
            .unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
    };

    let lib_source = source("crates/novex-provider-client/src/lib.rs");
    for module in [
        "error",
        "http",
        "chat_plan",
        "chat_dispatch",
        "chat_parse",
        "compaction",
        "native_cancel",
        "media",
        "rag",
    ] {
        assert!(lib_source.contains(&format!("mod {module};")));
    }
    assert!(lib_source.contains("pub use error::ModelProviderClientError;"));
    assert!(lib_source.contains("pub use chat_plan::{"));
    assert!(lib_source.contains("pub use chat_dispatch::{"));
    assert!(lib_source.contains("pub use chat_parse::{"));
    assert!(lib_source.contains("pub use compaction::{"));
    assert!(lib_source.contains("pub use native_cancel::{"));
    assert!(lib_source.contains("pub use media::{"));
    assert!(lib_source.contains("pub use rag::{"));
    assert!(lib_source.lines().count() <= 120);

    let chat_plan_source = source("crates/novex-provider-client/src/chat_plan.rs");
    assert!(chat_plan_source.contains("pub struct ModelProviderChatPlanInput"));
    assert!(chat_plan_source.contains("pub fn build_model_provider_chat_plan"));
    assert!(chat_plan_source.contains("chat_plan_builder_maps_regular_chat_completion_payload"));

    let chat_parse_source = source("crates/novex-provider-client/src/chat_parse.rs");
    assert!(chat_parse_source.contains("pub struct ModelChatProviderOutput"));
    assert!(chat_parse_source.contains("pub struct ModelChatStreamCompletionBuilder"));
    assert!(chat_parse_source.contains("pub fn parse_model_chat_provider_output_from_text"));

    let compaction_source = source("crates/novex-provider-client/src/compaction.rs");
    assert!(compaction_source.contains("pub struct ModelChatCompactionProviderOutput"));
    assert!(compaction_source.contains("pub fn parse_model_chat_compaction_provider_output_from_text"));

    let rag_source = source("crates/novex-provider-client/src/rag.rs");
    assert!(rag_source.contains("pub struct ModelProviderEmbeddingRequest"));
    assert!(rag_source.contains("pub fn parse_model_provider_embedding_vectors"));
}
```

- [x] **Step 2: Verify RED**

Run: `cargo test -p backend-rust provider_client_crate_uses_focused_modules --offline`

Expected: FAIL because provider-client is still implemented in a single `src/lib.rs`.

---

### Task 2: Split Error And HTTP Primitives

**Files:**
- Create: `crates/novex-provider-client/src/error.rs`
- Create: `crates/novex-provider-client/src/http.rs`
- Modify: `crates/novex-provider-client/src/lib.rs`

**Interfaces:**
- Produces:
  - `error::ModelProviderClientError`
  - `http::{ModelProviderHttpRequest, model_provider_http_client, send_model_provider_http_request, read_model_provider_response_text}`
- Consumes: existing public error and generic HTTP primitive definitions from `lib.rs`.

- [x] **Step 1: Move error type**

Create `error.rs` with `ModelProviderClientError`, its `Display`, and `Error` implementations. Re-export it from `lib.rs` with:

```rust
mod error;
pub use error::ModelProviderClientError;
```

- [x] **Step 2: Move HTTP primitives**

Create `http.rs` with `ModelProviderHttpRequest`, `model_provider_http_client`, `send_model_provider_http_request`, and `read_model_provider_response_text`. Import `crate::ModelProviderClientError` inside the module.

- [x] **Step 3: Move HTTP tests**

Move `http_status_error_preserves_backend_message_shape` and `bad_response_error_preserves_provider_message` into `error.rs`. Move `http_request_carries_provider_post_inputs` into `http.rs`.

- [x] **Step 4: Verify focused primitive tests**

Run: `cargo test -p novex-provider-client http --offline`

Expected: PASS.

---

### Task 3: Split Chat Planning, Dispatch, Parsing, And Compaction

**Files:**
- Create: `crates/novex-provider-client/src/chat_plan.rs`
- Create: `crates/novex-provider-client/src/chat_dispatch.rs`
- Create: `crates/novex-provider-client/src/chat_parse.rs`
- Create: `crates/novex-provider-client/src/compaction.rs`
- Modify: `crates/novex-provider-client/src/lib.rs`

**Interfaces:**
- Produces:
  - `chat_plan::{ModelProviderChatTransport, ModelProviderChatMessage, ModelProviderChatFileContext, ModelProviderChatRequestKind, ModelProviderChatCompactionMetadata, ModelProviderChatRequestMetadata, ModelProviderChatPlanInput, ModelProviderChatPlan, build_model_provider_chat_plan, model_provider_chat_plan_streams_chat_completion}`
  - `chat_dispatch::{ModelProviderChatRequest, send_model_provider_chat_request, send_model_provider_chat_unary_request}`
  - `chat_parse::{ModelChatProviderOutput, ModelChatStreamCompletionBuilder, parse_model_chat_provider_output_from_text, parse_model_chat_provider_output_from_body, parse_model_chat_provider_output_from_sse_text, model_chat_sse_record_data_payload, model_provider_response_id_from_payloads, model_provider_response_id_from_payload, normalize_model_provider_response_id}`
  - `compaction::{ModelChatCompactionProviderOutput, parse_model_chat_compaction_provider_output_from_text, parse_model_chat_compaction_provider_output_from_body, parse_model_chat_compaction_provider_output_from_sse_text}`

- [x] **Step 1: Move chat plan code and tests**

Move chat plan DTOs, endpoint/payload helpers, and `chat_plan_builder_*` tests into `chat_plan.rs`.

- [x] **Step 2: Move chat dispatch code and tests**

Move `ModelProviderChatRequest`, `send_model_provider_chat_request`, `send_model_provider_chat_unary_request`, `chat_request_carries_provider_dispatch_inputs`, and `chat_http_status_error_preserves_backend_message_shape` into `chat_dispatch.rs`.

- [x] **Step 3: Move chat parsing code and tests**

Move `ModelChatProviderOutput`, `ModelChatStreamCompletionBuilder`, chat parser helpers, response-id helpers, and chat parser tests into `chat_parse.rs`.

- [x] **Step 4: Move compaction parsing code and tests**

Move `ModelChatCompactionProviderOutput`, compaction parser helpers, and compaction parser tests into `compaction.rs`. Share response id/status helpers from `chat_parse`.

- [x] **Step 5: Verify chat modules**

Run:

```bash
cargo test -p novex-provider-client chat --offline
cargo test -p novex-provider-client compaction --offline
```

Expected: all commands pass.

---

### Task 4: Split Native Cancel, Media, And RAG Modules

**Files:**
- Create: `crates/novex-provider-client/src/native_cancel.rs`
- Create: `crates/novex-provider-client/src/media.rs`
- Create: `crates/novex-provider-client/src/rag.rs`
- Modify: `crates/novex-provider-client/src/lib.rs`

**Interfaces:**
- Produces:
  - `native_cancel::{ModelProviderNativeCancelRequest, send_model_provider_native_cancel_request}`
  - `media::{ModelProviderMediaImageRequest, send_model_provider_media_image_request}`
  - `rag::{ModelProviderEmbeddingRequest, ModelProviderRerankRequest, send_model_provider_embedding_request, send_model_provider_rerank_request, parse_model_provider_rerank_scores, parse_model_provider_embedding_vectors}`

- [x] **Step 1: Move native cancel code and tests**

Move native cancel request/dispatch and tests into `native_cancel.rs`.

- [x] **Step 2: Move media code and tests**

Move media image request/dispatch/parser dependency tests into `media.rs`.

- [x] **Step 3: Move RAG code and tests**

Move embedding/rerank request, dispatch, parser functions, `json_usize`, `json_f32`, and RAG tests into `rag.rs`.

- [x] **Step 4: Verify provider-client crate**

Run: `cargo test -p novex-provider-client --offline`

Expected: PASS.

---

### Task 5: Update Source Contracts, Matrix, Verify, Commit, Merge, And Clean

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-18-agent-provider-client-focused-modules.md`

**Interfaces:**
- Consumes: focused provider-client module files from Tasks 2-4.
- Produces: source-contract evidence that provider-client is no longer a monolithic crate file.

- [x] **Step 1: Update matrix and plan status**

Change runtime-loop status to the next slice and update the notes to say `novex-provider-client` now has focused internal modules for error, HTTP, chat planning, chat dispatch, chat parsing, compaction parsing, native cancel, media, and RAG.

- [x] **Step 2: Run focused verification**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend-rust provider_client_crate_uses_focused_modules --offline
cargo test -p backend-rust provider_client_chat_request_plan_lives_in_provider_client_crate --offline
cargo test -p backend-rust provider_client_chat_dispatch_lives_in_provider_client_crate --offline
cargo test -p backend-rust provider_client_chat_response_parsers_live_in_provider_client_crate --offline
cargo test -p backend-rust provider_client_rag_dispatch_lives_in_provider_client_crate --offline
cargo test -p backend-rust provider_client_media_dispatch_lives_in_provider_client_crate --offline
cargo test -p backend-rust provider_client_native_cancel_dispatch_lives_in_provider_client_crate --offline
cargo test -p novex-provider-client --offline
cargo test --workspace --offline
```

Expected: all commands pass.

- [x] **Step 3: Commit implementation**

```bash
git add crates/novex-provider-client/src backend/src/application/ai/model_service.rs docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-18-agent-provider-client-focused-modules.md
git commit -m "refactor: split provider client focused modules"
```

- [ ] **Step 4: Merge into main and verify main**

```bash
git -C /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex merge --ff-only feat/enterprise-agent-foundation
```

Then run in `/Users/yusenlin/Avalon/freedom/github/zm-agent/Novex`:

```bash
cargo fmt -- --check
git diff --check
cargo test --workspace --offline
```

- [ ] **Step 5: Sync feature worktree and clean both workspaces**

```bash
git -C /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex/.worktrees/enterprise-agent-foundation merge --ff-only main
cargo clean
```

Run `cargo clean` in both main and feature worktree after main verification.
