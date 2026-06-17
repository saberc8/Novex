# Agent Stream Completion Builder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move CodeAgent streaming chat completion from read-then-reparse SSE text to an internal incremental completion builder.

**Architecture:** Add a private `ModelChatStreamCompletionBuilder` in `model_service.rs` that observes each parsed SSE value, tracks provider response metadata, usage, terminal state, ordered delta chunks, and the final answer. Existing fallback, provider-call lease, route selection, request payloads, Agent stream receiver, and provider-native cancel behavior stay unchanged. The streaming HTTP path will emit provider delta events and return the builder's completed `ModelChatProviderOutput` directly instead of returning raw body text for a second parse.

**Tech Stack:** Rust, reqwest streaming chunks, serde_json, Tokio mpsc, backend model runtime service.

## Global Constraints

- Do not change provider payload shape, retry policy, fallback policy, or provider-call lease persistence.
- Do not remove `chat_completion_for_purpose` or the existing `ModelChatStreamCall` facade.
- Preserve `provider_stream_sender` backward compatibility while keeping Agent loop on the runtime facade.
- Preserve streamed tool-call early-stop, provider response id/status capture, provider-call lease id propagation, model_delta events, and final `provider_delta_chunks`.
- Keep compaction transports on their existing parser path in this slice; the new builder only owns chat-completion and Responses CodeAgent streams.

---

### Task 1: Add Incremental Completion Builder

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Adds private `ModelChatStreamCompletionBuilder`.
- Adds `observe_sse_value(&mut self, value: &Value) -> Vec<ModelProviderStreamChunk>`.
- Adds `observe_done(&mut self)`.
- Adds `finish(self) -> Result<ModelChatProviderOutput, AppError>`.

- [ ] **Step 1: Write failing builder contract tests**

Add tests named `model_stream_completion_builder_assembles_chat_sse_incrementally` and `model_stream_completion_builder_assembles_responses_sse_incrementally`. They should push parsed SSE JSON values into `ModelChatStreamCompletionBuilder`, call `observe_done` or a terminal `response.completed`, then assert answer text, usage, provider response id/status, delta ordering, and provider event names.

- [ ] **Step 2: Run focused test red**

Run: `cargo test -p backend-rust model_stream_completion_builder --offline`

Expected: FAIL because `ModelChatStreamCompletionBuilder` does not exist yet.

- [ ] **Step 3: Implement the builder**

Move the existing `model_chat_provider_output_from_sse_text` state fields into `ModelChatStreamCompletionBuilder`. `observe_sse_value` must update metadata, usage, terminal answer, terminal flag, and delta chunks in one pass. `finish` must keep the existing errors: incomplete SSE returns `LLM chat SSE 响应在完成前结束`; empty completed output returns `LLM chat SSE 响应为空`.

- [ ] **Step 4: Verify green**

Run: `cargo test -p backend-rust model_stream_completion_builder --offline`

Expected: PASS.

---

### Task 2: Use Builder in Streaming Runtime Path

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Replaces `model_chat_streaming_response_text(...) -> Result<String, AppError>` for chat streams with `model_chat_streaming_provider_output(...) -> Result<ModelChatProviderOutput, AppError>`.
- Adds `model_chat_response_from_provider_output(...) -> ModelChatResp` to share response shaping between streaming and text parser paths.

- [ ] **Step 1: Write failing source contract test**

Add `model_stream_completion_builder_runtime_path_returns_provider_output`. Assert the production source contains `model_chat_streaming_provider_output`, does not contain `model_chat_streaming_response_text`, and the streaming branch calls `model_chat_response_from_provider_output` without reparsing a streamed body string.

- [ ] **Step 2: Run focused test red**

Run: `cargo test -p backend-rust model_stream_completion_builder --offline`

Expected: FAIL because the runtime path still returns raw stream text.

- [ ] **Step 3: Route streaming HTTP through builder output**

Update `execute_normalized_chat_completion_with_route` so streamed chat-completion transports call `model_chat_streaming_provider_output` and immediately shape a `ModelChatResp` from the returned `ModelChatProviderOutput`. Non-streaming chat and compaction transports keep the existing text parser paths.

- [ ] **Step 4: Update matrix**

Update the runtime-loop row from `slice-55 implemented` to `slice-56 implemented`, mention internal incremental stream completion builder, and narrow the remaining gap to full provider future replacement / stream-first transport lifecycle.

- [ ] **Step 5: Verify**

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust model_stream_completion_builder --offline
cargo test -p backend-rust model_stream_native_runtime_api --offline
cargo test -p backend-rust provider_stream_response_id --offline
cargo test -p backend-rust provider_stream_lease_id --offline
cargo test -p backend-rust streamed_tool_call_early_stop --offline
cargo test --workspace --offline
git diff --check
```

Expected: all commands pass.
