# Agent Provider Client Chat Response Parsers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move provider-neutral chat, stream-completion, response metadata, and Responses compaction parsing from the backend adapter into `novex-provider-client`.

**Status:** Implemented in branch `feat/enterprise-agent-foundation`; pending final full verification and merge at the time this plan was updated.

**Architecture:** `novex-provider-client` owns provider response text reading, JSON-or-SSE chat output parsing, incremental stream completion assembly, response id/status extraction, and Responses compaction output parsing. `backend/src/application/ai/model_provider_transport.rs` remains a compatibility adapter that maps `ModelProviderClientError` into `AppError` and re-exports parser DTOs used by `model_service.rs`. Route resolution, fallback, provider-call leases, trace/eval, Agent events, tenant context, and cost accounting stay in backend.

**Tech Stack:** Rust 2021, Cargo workspace, `reqwest`, `serde_json`, `novex-model`, backend source-contract tests, provider-client crate unit tests, offline cargo verification.

## Global Constraints

- Do not change provider request payloads, route selection, fallback behavior, provider-call lease persistence, cost accounting, Agent event payloads, or stream early-stop semantics.
- `novex-provider-client` must not depend on `backend`, backend `AppError`, SQL, tenant context, provider-call leases, run-event persistence, or trace/eval crates.
- Backend remains responsible for converting `ModelProviderClientError` into `AppError`.
- Streaming event emission stays in `model_service.rs` because events need route summary, provider, model, and provider-call lease metadata.
- Verify with focused parser/stream/provider-client tests, formatting, diff checks, and the offline workspace test suite.

---

### Task 1: Add Provider-Client Chat Parser Source Contract

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `docs/plans/2026-06-18-agent-provider-client-chat-response-parsers.md`

**Interfaces:**
- Consumes: existing backend `model_provider_response_transport_adapter_source_contract` and provider-client source-contract pattern.
- Produces: backend test `provider_client_chat_response_parsers_live_in_provider_client_crate`.

- [ ] **Step 1: Write the failing source-contract test**

Add a backend test near the existing provider-client source-contract tests:

```rust
#[test]
fn provider_client_chat_response_parsers_live_in_provider_client_crate() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend manifest should live below workspace root");
    let source = |path: &str| {
        std::fs::read_to_string(workspace_root.join(path))
            .unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
    };
    let provider_client_source = source("crates/novex-provider-client/src/lib.rs");
    let backend_transport_source =
        source("backend/src/application/ai/model_provider_transport.rs");

    assert!(provider_client_source.contains("pub async fn read_model_provider_response_text"));
    assert!(provider_client_source.contains("pub struct ModelChatProviderOutput"));
    assert!(provider_client_source.contains("pub struct ModelChatCompactionProviderOutput"));
    assert!(provider_client_source.contains("pub struct ModelChatStreamCompletionBuilder"));
    assert!(provider_client_source.contains("pub fn parse_model_chat_provider_output_from_text"));
    assert!(provider_client_source.contains("pub fn parse_model_chat_provider_output_from_body"));
    assert!(provider_client_source.contains("pub fn parse_model_chat_provider_output_from_sse_text"));
    assert!(provider_client_source.contains("pub fn parse_model_chat_compaction_provider_output_from_text"));
    assert!(provider_client_source.contains("pub fn parse_model_chat_compaction_provider_output_from_body"));
    assert!(provider_client_source.contains("pub fn parse_model_chat_compaction_provider_output_from_sse_text"));
    assert!(provider_client_source.contains("pub fn model_chat_sse_record_data_payload"));
    assert!(provider_client_source.contains("pub fn model_provider_response_id_from_payloads"));
    assert!(provider_client_source.contains("pub fn normalize_model_provider_response_id"));
    assert!(backend_transport_source.contains("novex_provider_client::read_model_provider_response_text(response)"));
    assert!(backend_transport_source.contains("model_provider_client_error_to_app_error"));
    assert!(!backend_transport_source.contains("pub(super) struct ModelChatProviderOutput"));
    assert!(!backend_transport_source.contains("pub(super) struct ModelChatCompactionProviderOutput"));
    assert!(!backend_transport_source.contains("pub(super) struct ModelChatStreamCompletionBuilder"));
    assert!(!backend_transport_source.contains("fn model_chat_compaction_output_from_items"));
    assert!(!backend_transport_source.contains("fn model_chat_answer_from_provider_body"));
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p backend provider_client_chat_response_parsers_live_in_provider_client_crate --offline`

Expected: FAIL because chat/compaction parsers and stream builder still live in backend-local transport.

---

### Task 2: Move Response Text Reader, DTOs, Chat Parser, And Stream Builder

**Files:**
- Modify: `crates/novex-provider-client/src/lib.rs`
- Modify: `backend/src/application/ai/model_provider_transport.rs`

**Interfaces:**
- Produces provider-client APIs:
  - `pub async fn read_model_provider_response_text(response: reqwest::Response) -> Result<String, ModelProviderClientError>`
  - `pub struct ModelChatProviderOutput`
  - `pub struct ModelChatStreamCompletionBuilder`
  - `pub fn parse_model_chat_provider_output_from_text(body_text: &str) -> Result<ModelChatProviderOutput, ModelProviderClientError>`
  - `pub fn parse_model_chat_provider_output_from_body(body: &Value) -> Result<ModelChatProviderOutput, ModelProviderClientError>`
  - `pub fn parse_model_chat_provider_output_from_sse_text(body_text: &str) -> Result<ModelChatProviderOutput, ModelProviderClientError>`
- Backend adapter wrappers keep current signatures returning `Result<_, AppError>`.

- [ ] **Step 1: Move chat output types and helpers**

Move `ModelChatProviderOutput`, `ModelChatStreamCompletionBuilder`, chat JSON/SSE parser functions, SSE record parsing, delta chunk extraction, text extraction, terminal detection, and response metadata helpers into `crates/novex-provider-client/src/lib.rs`. Replace backend `AppError::bad_request(...)` returns with `ModelProviderClientError::BadResponse(...)` carrying the same Chinese error strings.

- [ ] **Step 2: Add backend compatibility wrappers**

In `backend/src/application/ai/model_provider_transport.rs`, re-export pure parser types/functions from provider-client and wrap fallible calls:

```rust
pub(super) async fn read_model_provider_response_text(
    response: reqwest::Response,
) -> Result<String, AppError> {
    novex_provider_client::read_model_provider_response_text(response)
        .await
        .map_err(model_provider_client_error_to_app_error)
}
```

`parse_model_chat_provider_output_from_text` and `parse_model_chat_provider_output_from_body` must keep their backend signatures and map provider-client errors with `model_provider_client_error_to_app_error`.

- [ ] **Step 3: Update stream finish error mapping**

Update `model_chat_streaming_provider_output(...)` in `backend/src/application/ai/model_service.rs`:

```rust
builder
    .finish()
    .map_err(model_provider_client_error_to_app_error)
```

- [ ] **Step 4: Verify chat parser and stream tests**

Run:

```bash
cargo test -p backend model_provider_response_transport_adapter --offline
cargo test -p backend model_stream_completion_builder --offline
cargo test -p backend provider_token_delta --offline
cargo test -p novex-provider-client chat --offline
```

Expected: all commands pass after the move.

---

### Task 3: Move Responses Compaction Parser

**Files:**
- Modify: `crates/novex-provider-client/src/lib.rs`
- Modify: `backend/src/application/ai/model_provider_transport.rs`
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Produces provider-client APIs:
  - `pub struct ModelChatCompactionProviderOutput`
  - `pub fn parse_model_chat_compaction_provider_output_from_text(body_text: &str) -> Result<ModelChatCompactionProviderOutput, ModelProviderClientError>`
  - `pub fn parse_model_chat_compaction_provider_output_from_body(body: &Value) -> Result<ModelChatCompactionProviderOutput, ModelProviderClientError>`
  - `pub fn parse_model_chat_compaction_provider_output_from_sse_text(body_text: &str) -> Result<ModelChatCompactionProviderOutput, ModelProviderClientError>`

- [ ] **Step 1: Move compaction output parser**

Move JSON and SSE compaction parser helpers into provider-client, preserving these existing error strings exactly:

```text
LLM compaction 响应缺少 output
LLM compaction SSE 响应不是合法 JSON
LLM compaction SSE 响应在 response.completed 前结束
LLM compaction 响应应包含 1 个 compaction 输出，实际 {compaction_count}/{output_item_count}
LLM compaction 输出缺少 encrypted_content
LLM compaction 响应为空
```

- [ ] **Step 2: Add backend compatibility wrappers**

In `backend/src/application/ai/model_provider_transport.rs`, keep the existing backend function names and return `AppError` by mapping provider-client parser errors.

- [ ] **Step 3: Verify compaction tests**

Run:

```bash
cargo test -p backend provider_compact_transport --offline
cargo test -p backend remote_compaction --offline
cargo test -p backend model_loop_compaction --offline
```

Expected: all commands pass.

---

### Task 4: Update Documentation, Verify, Commit, Merge, And Clean

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-18-agent-provider-client-chat-response-parsers.md`

**Interfaces:**
- Consumes: provider-client chat/compaction parser APIs from Tasks 2 and 3.
- Produces: migration matrix note that `novex-provider-client` owns response text reading, chat/stream/compaction parser DTOs, stream completion assembly, response metadata extraction, and JSON/SSE parser APIs.

- [ ] **Step 1: Update migration matrix**

Change the runtime-loop notes so backend-local provider transport no longer claims ownership of chat/compaction parsing and stream completion assembly. Add evidence that `novex-provider-client` owns these parser APIs, while full chat/Responses HTTP dispatch extraction remains next.

- [ ] **Step 2: Run verification**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p novex-provider-client --offline
cargo test -p backend model_provider_response_transport_adapter --offline
cargo test -p backend provider_compact_transport --offline
cargo test -p backend model_stream_completion_builder --offline
cargo test -p backend provider_token_delta --offline
cargo test --workspace --offline
```

- [ ] **Step 3: Commit implementation**

```bash
git add crates/novex-provider-client/src/lib.rs backend/src/application/ai/model_provider_transport.rs backend/src/application/ai/model_service.rs docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-18-agent-provider-client-chat-response-parsers.md
git commit -m "feat: extract provider client chat response parsers"
```

- [ ] **Step 4: Merge to main, verify, sync, and clean**

```bash
git -C /path/to/Novex merge --ff-only feat/enterprise-agent-foundation
cargo fmt -- --check
git diff --check
cargo test --workspace --offline
cargo clean
git -C /path/to/Novex/.worktrees/enterprise-agent-foundation merge --ff-only main
cargo clean
```
