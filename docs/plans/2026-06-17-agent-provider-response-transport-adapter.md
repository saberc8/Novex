# Agent Provider Response Transport Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move model provider unary response parsing and chat SSE completion assembly behind the backend-local provider transport adapter boundary.

**Architecture:** Keep `model_service.rs` responsible for route resolution, fallback, provider-call leases, Agent run events, usage/cost response shaping, and dispatch-mode choice. Move reusable provider response parsing into `model_provider_transport.rs`: reading unary response text, parsing JSON-or-SSE chat completions, parsing Responses compaction JSON-or-SSE bodies, and maintaining the incremental stream completion builder. `model_service.rs` converts transport outputs into `ModelChatResp` and emits stream events from returned delta chunks.

**Tech Stack:** Rust, reqwest, serde_json, tokio mpsc, backend model runtime service, source-contract tests, offline cargo tests.

## Global Constraints

- Do not change provider request payloads, route selection, fallback behavior, provider-call lease persistence, cost accounting, Agent event payloads, or stream early-stop semantics.
- Do not move native provider cancel clients in this slice; they remain in `model_service.rs`.
- `model_provider_transport.rs` may depend on backend-local model DTOs needed for provider output shape, but it must not depend on `ModelRuntimeRoute`, SQL rows, tenant context, or run-event persistence.
- `model_service.rs` must retain `ModelProviderStreamEvent` emission because events need route summary, provider, model, and provider-call lease metadata.
- This slice does not create a new crate. It prepares the later crate split by tightening the backend-local adapter boundary first.

---

### Task 1: Add Response Adapter Source Contract

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Production `model_service.rs` imports provider response parsing symbols from `model_provider_transport`.
- `execute_normalized_chat_completion_with_route` delegates unary body reading to `read_model_provider_response_text`.
- Chat/Responses parsing and stream builder types are no longer defined in `model_service.rs`.

- [ ] **Step 1: Write failing source-contract test**

Add backend test `model_provider_response_transport_adapter_source_contract` near the existing provider transport tests:

```rust
#[test]
fn model_provider_response_transport_adapter_source_contract() {
    let service_source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    let transport_source = include_str!("model_provider_transport.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    let route_path = &service_source[service_source
        .find("async fn execute_normalized_chat_completion_with_route")
        .unwrap()
        ..service_source.find("fn normalize_model_chat_command").unwrap()];

    assert!(service_source.contains("read_model_provider_response_text"));
    assert!(service_source.contains("ModelChatStreamCompletionBuilder"));
    assert!(transport_source.contains("pub(super) async fn read_model_provider_response_text"));
    assert!(transport_source.contains("pub(super) struct ModelChatProviderOutput"));
    assert!(transport_source.contains("pub(super) struct ModelChatCompactionProviderOutput"));
    assert!(transport_source.contains("pub(super) struct ModelChatStreamCompletionBuilder"));
    assert!(route_path.contains("read_model_provider_response_text(response).await?"));
    assert!(!route_path.contains("response.text().await.unwrap_or_default()"));
    assert!(!service_source.contains("fn model_chat_provider_output_from_body"));
    assert!(!service_source.contains("fn model_chat_provider_output_from_sse_text"));
    assert!(!service_source.contains("fn model_chat_compaction_provider_output_from_body"));
    assert!(!service_source.contains("fn model_chat_compaction_provider_output_from_sse_text"));
}
```

- [ ] **Step 2: Verify red**

Run: `cargo test -p backend model_provider_response_transport_adapter --offline`

Expected: FAIL because response parsing still lives in `model_service.rs`.

---

### Task 2: Move Provider Output Types And Unary Body Reader

**Files:**
- Modify: `backend/src/application/ai/model_provider_transport.rs`
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- `pub(super) struct ModelChatProviderOutput` exposes `answer`, `usage`, `provider_response_id`, `provider_response_status`, and `delta_chunks`.
- `pub(super) struct ModelChatCompactionProviderOutput` exposes `answer`, `usage`, `provider_response_id`, and `provider_response_status`.
- `pub(super) async fn read_model_provider_response_text(response: reqwest::Response) -> Result<String, AppError>`.

- [ ] **Step 1: Add transport imports**

Add to `model_provider_transport.rs`:

```rust
use super::model_service::{
    normalize_model_provider_usage_for_transport, ModelChatUsage, ModelProviderStreamChunk,
};
```

Expose a small wrapper in `model_service.rs` so transport parsing can reuse the existing usage normalization without moving route/provider usage normalization in this slice:

```rust
pub(super) fn normalize_model_provider_usage_for_transport(body: &Value) -> ModelChatUsage {
    normalize_model_provider_usage(body)
}
```

- [ ] **Step 2: Move output structs**

Move `ModelChatProviderOutput` and `ModelChatCompactionProviderOutput` into `model_provider_transport.rs` and mark fields `pub(super)`.

- [ ] **Step 3: Add unary body reader**

Add to `model_provider_transport.rs`:

```rust
pub(super) async fn read_model_provider_response_text(
    response: reqwest::Response,
) -> Result<String, AppError> {
    response.text().await.map_err(|err| AppError::Anyhow(err.into()))
}
```

- [ ] **Step 4: Wire route execution**

Replace:

```rust
let body_text = response.text().await.unwrap_or_default();
```

with:

```rust
let body_text = read_model_provider_response_text(response).await?;
```

- [ ] **Step 5: Verify green for moved body reader contract**

Run: `cargo test -p backend model_provider_response_transport_adapter --offline`

Expected: still FAIL until parsing functions move; the failure should no longer be caused by missing `read_model_provider_response_text`.

---

### Task 3: Move Chat SSE Builder And Chat Output Parsers

**Files:**
- Modify: `backend/src/application/ai/model_provider_transport.rs`
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- `pub(super) struct ModelChatStreamCompletionBuilder` keeps `observe_done`, `observe_sse_value`, `finish`, `provider_response_id`, and `provider_response_status`.
- `pub(super) fn parse_model_chat_provider_output_from_text(body_text: &str) -> Result<ModelChatProviderOutput, AppError>`.
- `pub(super) fn model_chat_sse_record_data_payload(record: &str) -> Option<String>`.
- `model_service.rs` keeps `model_chat_emit_complete_stream_records` and calls builder methods from the transport module.

- [ ] **Step 1: Move builder and chat parsing helpers**

Move these functions and helpers into `model_provider_transport.rs`:

```rust
ModelChatStreamCompletionBuilder
parse_model_chat_provider_output_from_text
model_chat_provider_output_from_body
model_chat_provider_output_from_sse_text
model_chat_response_payload_from_sse_value
model_chat_sse_data_payloads
model_chat_sse_record_data_payload
model_chat_provider_delta_chunks_from_sse_value
model_chat_responses_delta_content_from_value
model_chat_delta_content_from_choice
```

Also move any directly required private provider body helpers that are purely response-shape parsing helpers.

- [ ] **Step 2: Update service wrappers**

Update `model_chat_response_from_chat_completion_text` to call:

```rust
let output = parse_model_chat_provider_output_from_text(body_text)?;
```

- [ ] **Step 3: Verify chat stream regressions**

Run:

```bash
cargo test -p backend model_provider_response_transport_adapter --offline
cargo test -p backend model_stream_completion_builder --offline
cargo test -p backend provider_token_delta --offline
cargo test -p backend provider_stream_response_id --offline
cargo test -p backend provider_stream_lease_id --offline
```

Expected: all commands pass.

---

### Task 4: Move Responses Compaction Output Parsers

**Files:**
- Modify: `backend/src/application/ai/model_provider_transport.rs`
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- `pub(super) fn parse_model_chat_compaction_provider_output_from_text(body_text: &str) -> Result<ModelChatCompactionProviderOutput, AppError>`.
- Existing compaction answer extraction behavior and error strings remain unchanged.

- [ ] **Step 1: Move compaction parsers**

Move these functions into `model_provider_transport.rs`:

```rust
parse_model_chat_compaction_provider_output_from_text
model_chat_compaction_provider_output_from_body
model_chat_compaction_provider_output_from_sse_text
```

Move direct helper dependencies only if they are pure provider output parsing helpers. Keep request payload builders in `model_service.rs`.

- [ ] **Step 2: Update service wrapper**

Update `model_chat_response_from_responses_compaction_text` to call:

```rust
let output = parse_model_chat_compaction_provider_output_from_text(body_text)?;
```

- [ ] **Step 3: Verify compaction regressions**

Run:

```bash
cargo test -p backend provider_compact_transport --offline
cargo test -p backend provider_compact_unary --offline
cargo test -p backend provider_background_response_capture --offline
```

Expected: all commands pass.

---

### Task 5: Update Matrix And Regression Gates

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Runtime loop matrix status advances from `slice-61 implemented` to `slice-62 implemented`.
- Runtime loop notes mention provider response parsing/SSE builder adapter extraction.
- Runtime loop POC evidence mentions provider output parsing through backend-local transport adapter.
- Acceptance evidence includes `model_provider_response_transport_adapter`.
- Remaining gap narrows to moving native cancel clients and provider-specific client modules behind provider transport modules.

- [ ] **Step 1: Update migration matrix**

Add this plan to Follow-up Implementation Plans:

```markdown
- Agent provider response transport adapter: `docs/plans/2026-06-17-agent-provider-response-transport-adapter.md`
```

- [ ] **Step 2: Full verification**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend model_provider_response_transport_adapter --offline
cargo test -p backend model_provider_http_transport_adapter --offline
cargo test -p backend model_provider_stream_dispatch_mode --offline
cargo test -p backend model_provider_stream_dispatch_route_path --offline
cargo test -p backend model_stream_transport_executor --offline
cargo test -p backend model_stream_completion_builder --offline
cargo test -p backend provider_token_delta --offline
cargo test -p backend provider_stream_response_id --offline
cargo test -p backend provider_stream_lease_id --offline
cargo test -p backend provider_compact_transport --offline
cargo test -p backend provider_compact_unary --offline
cargo test -p backend provider_background_response_capture --offline
cargo test -p backend streamed_tool_call_early_stop --offline
cargo test -p backend provider_abort --offline
cargo test --workspace --offline
```

Expected: all commands pass.
