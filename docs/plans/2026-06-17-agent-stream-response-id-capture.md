# Agent Stream Response Id Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Capture Responses stream `response.created` id/status before provider completion and carry it through Agent stream events and early-stop completion metadata.

**Architecture:** Extend `ModelProviderStreamEvent` with optional provider response metadata. Change the SSE stream emitter from stateless chunk emission to a small mutable stream state that remembers the latest response id/status from any SSE record, then stamps subsequent delta events with that metadata. The Agent provider stream state retains the same metadata so streamed tool-call early-stop can expose it without requiring a full `ModelChatResp`.

**Tech Stack:** Rust, Tokio mpsc provider stream channel, backend model runtime SSE parsing, Agent model-loop provider stream state.

## Global Constraints

- Do not perform provider-native remote cancellation in this slice.
- Do not change model answer assembly, token usage parsing, or provider retry semantics.
- Keep `model_delta` and `model_stream_tool_call` payloads backward compatible by only adding optional metadata fields when present.
- Preserve chat-completions SSE behavior where response id is available on delta records themselves.
- Preserve early-stop behavior when no provider response id has been captured.

---

### Task 1: Capture Provider Response Metadata While Streaming

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Changes: `ModelProviderStreamEvent { provider_response_id: Option<String>, provider_response_status: Option<String>, ... }`.
- Produces: `ModelChatStreamState` that tracks `next_chunk_index`, `provider_response_id`, and `provider_response_status`.

- [ ] **Step 1: Write the failing stream metadata test**

Add a backend unit test named `provider_stream_event_carries_responses_created_metadata`. It should feed a `response.created` SSE record followed by a `response.output_text.delta` record into `model_chat_emit_complete_stream_records`, then assert the emitted `ModelProviderStreamEvent` has `provider_response_id == Some("resp_stream_1")`, `provider_response_status == Some("in_progress")`, and the delta chunk content is unchanged.

- [ ] **Step 2: Run the focused test red**

Run: `cargo test -p backend-rust provider_stream_response_id --offline`

Expected: FAIL because provider stream events do not carry provider response id/status and the emitter only returns a next chunk index.

- [ ] **Step 3: Implement stream state and event metadata**

Add optional metadata fields to `ModelProviderStreamEvent`. Replace the `next_chunk_index` local in `model_chat_streaming_response_text` with `ModelChatStreamState::default()`. Update `model_chat_emit_complete_stream_records` to accept `&mut ModelChatStreamState`, update metadata from `model_chat_response_payload_from_sse_value(&value)` before chunk extraction, and stamp every emitted event with cloned metadata.

- [ ] **Step 4: Verify green**

Run: `cargo test -p backend-rust provider_stream_response_id --offline`

Expected: PASS.

---

### Task 2: Preserve Metadata In Agent Stream Events And Early Stop Completion

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Changes: `ModelLoopProviderCompletion<T>` gains `provider_response_id: Option<String>` and `provider_response_status: Option<String>`.
- Changes: `ModelLoopProviderStreamState` retains latest provider response metadata from stream events.

- [ ] **Step 1: Write failing Agent metadata tests**

Add tests named `provider_stream_response_id_is_added_to_model_delta_payload` and `streamed_tool_call_early_stop_retains_provider_response_metadata`. The first should assert `model_delta_event_payload_from_stream_event` includes `providerResponseId` and `providerResponseStatus` when the stream event carries them. The second should assert a streamed tool-call early-stop completion has the same metadata.

- [ ] **Step 2: Run focused tests red**

Run: `cargo test -p backend-rust provider_stream_response_id --offline`

Expected: FAIL because Agent payloads and completions do not retain stream response metadata yet.

- [ ] **Step 3: Implement Agent metadata retention**

Add response metadata fields to `ModelLoopProviderStreamState`, update them before tool-call parsing on each event, and include them in `model_loop_streamed_tool_call_completion`. Add optional `providerResponseId` and `providerResponseStatus` to `model_delta_event_payload_from_stream_event` and `model_stream_tool_call_event_payload`.

- [ ] **Step 4: Update matrix and verify**

Update the Runtime loop rows to say early stream response id/status capture is implemented, and narrow the remaining gap to provider-native remote cancellation dispatch from that early metadata.

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust provider_stream_response_id --offline
cargo test -p backend-rust streamed_tool_call_early_stop --offline
cargo test -p backend-rust provider_token_delta --offline
cargo test --workspace --offline
git diff --check
```

Expected: all commands pass.
