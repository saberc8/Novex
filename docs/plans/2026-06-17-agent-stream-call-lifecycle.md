# Agent Stream Call Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `ModelChatStreamCall` the single owned lifecycle object consumed by the Agent model loop.

**Architecture:** Add typed lifecycle metadata to `ModelChatStreamCall` and move Agent waiting code to accept the whole stream call instead of separate response future and event receiver parameters. This keeps the existing provider future, mpsc stream events, fallback, provider leases, early-stop native cancel, and completion builder behavior unchanged while tightening the API boundary for the next stream-first transport replacement.

**Tech Stack:** Rust, Tokio mpsc, boxed futures, backend model runtime service, Agent model loop.

## Global Constraints

- Do not change provider payload shape, retry policy, fallback policy, provider-call lease persistence, or response parsing.
- Do not remove `chat_completion_for_purpose`; non-stream callers keep the unary API.
- Keep `ModelChatStreamCall.response` and `ModelChatStreamCall.events` available for compatibility in this slice.
- Agent model loop must stop passing `.response` and `.events` separately into its await helper.
- Preserve streamed tool-call early-stop, provider-native cancel dispatch, provider response id/status capture, model_delta events, and `ModelChatStreamCompletionBuilder`.

---

### Task 1: Add Stream Lifecycle Metadata

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Adds public `ModelChatStreamLifecycle`.
- Adds `pub lifecycle: ModelChatStreamLifecycle` to `ModelChatStreamCall`.
- Adds private `model_chat_stream_lifecycle(purpose, command)` helper.

- [ ] **Step 1: Write failing lifecycle contract test**

Add backend test `model_stream_call_lifecycle_exposes_purpose_route_and_source`. It should assert the production source contains `pub struct ModelChatStreamLifecycle`, fields for `purpose`, `requested_route_id`, and `source`, and `ModelChatStreamCall` contains `pub lifecycle: ModelChatStreamLifecycle`.

- [ ] **Step 2: Run focused test red**

Run: `cargo test -p backend-rust model_stream_call_lifecycle --offline`

Expected: FAIL because lifecycle metadata does not exist.

- [ ] **Step 3: Implement lifecycle metadata**

Add `ModelChatStreamLifecycle { purpose, requested_route_id, source }`. Build it in `chat_completion_stream_for_purpose` before installing the provider stream sender. `source` should come from `command.provider_call_context.source` when non-empty, otherwise `model_runtime`.

- [ ] **Step 4: Verify green**

Run: `cargo test -p backend-rust model_stream_call_lifecycle --offline`

Expected: PASS.

---

### Task 2: Make Agent Await Consume the Whole Stream Call

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Renames the Agent await helper to `await_model_loop_stream_call_or_cancelled_with_delta_events`.
- Changes helper input from separate future and receiver to one `ModelChatStreamCall`.
- Agent model loop passes `model_stream_call` directly.

- [ ] **Step 1: Write failing Agent source-contract test**

Add backend test `agent_model_stream_call_lifecycle_owns_future_and_events`. It should inspect `execute_model_loop_existing_run` and the await helper. Assert the model loop passes `model_stream_call` directly, does not pass `model_stream_call.events` or `model_stream_call.response`, and the helper takes `model_stream_call: ModelChatStreamCall`.

- [ ] **Step 2: Run focused test red**

Run: `cargo test -p backend-rust model_stream_call_lifecycle --offline`

Expected: FAIL because Agent still splits the stream call into separate future and receiver parameters.

- [ ] **Step 3: Update Agent await helper**

Import/use `ModelChatStreamCall` in Agent service if needed. Update the helper signature to accept `model_stream_call: ModelChatStreamCall`, destructure `response` and `events` inside the helper, and keep existing select/cancel/drain behavior unchanged.

- [ ] **Step 4: Update migration matrix**

Update Runtime loop status from `slice-56 implemented` to `slice-57 implemented`, mention stream call lifecycle ownership, and narrow the remaining gap to replacing the boxed provider response future with a native stream-first transport task.

- [ ] **Step 5: Verify**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend-rust model_stream_call_lifecycle --offline
cargo test -p backend-rust model_stream_native_runtime_api --offline
cargo test -p backend-rust model_stream_completion_builder --offline
cargo test -p backend-rust streamed_tool_call_early_stop --offline
cargo test --workspace --offline
```

Expected: all commands pass.
