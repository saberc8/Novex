# Agent Stream Native Runtime API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Agent model-loop streaming from ad hoc sender injection in `ModelChatCommand` to a typed `ModelChatStreamCall` returned by `ModelRuntimeService`.

**Architecture:** Introduce a stream-native runtime facade that owns provider stream channel creation and exposes two typed outputs: a provider response future and a `ModelProviderStreamEvent` receiver. Keep the existing provider-call lease, fallback, SSE parsing, early response metadata, and native cancel behavior unchanged in this slice. Agent loop will request the stream call from `ModelRuntimeService` and pass the returned future/receiver into the existing cancellation-aware drain loop.

**Tech Stack:** Rust, Tokio mpsc, boxed futures, backend model runtime service, Agent model loop.

## Global Constraints

- Do not remove existing `chat_completion_for_purpose`; non-Agent callers keep the unary API.
- Do not change provider payload shape, retry policy, fallback policy, or provider-call lease persistence.
- Do not weaken streamed tool-call early-stop, provider-native cancel, trace, eval, or frontend delta behavior.
- Keep this as an API boundary slice; internal HTTP/SSE implementation can still use the current response future until the next slice replaces it with a fully incremental completion builder.
- Preserve optional `provider_stream_sender` on `ModelChatCommand` for backward compatibility, but Agent loop must stop creating or injecting it directly.

---

### Task 1: Add Typed Stream Call Facade

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Adds: `pub type ModelChatStreamFuture = Pin<Box<dyn Future<Output = Result<ModelChatResp, AppError>> + Send>>`.
- Adds: `pub struct ModelChatStreamCall { pub response: ModelChatStreamFuture, pub events: mpsc::UnboundedReceiver<ModelProviderStreamEvent> }`.
- Adds: `ModelRuntimeService::chat_completion_stream_for_purpose(purpose, command) -> Result<ModelChatStreamCall, AppError>`.

- [ ] **Step 1: Write failing stream facade contract test**

Add a backend unit test named `model_stream_native_runtime_api_exposes_future_and_event_receiver`. It should assert the production source contains `pub struct ModelChatStreamCall`, `pub type ModelChatStreamFuture`, and `pub async fn chat_completion_stream_for_purpose`.

- [ ] **Step 2: Run focused test red**

Run: `cargo test -p backend model_stream_native_runtime_api --offline`

Expected: FAIL because the stream facade does not exist yet.

- [ ] **Step 3: Implement the stream facade**

Add the boxed future type and stream call struct near `ModelProviderStreamEvent`. Implement `chat_completion_stream_for_purpose` by normalizing command, resolving the route, creating an mpsc channel, installing the sender into the command, cloning `self`, and boxing the existing fallback-backed chat completion future.

- [ ] **Step 4: Verify green**

Run: `cargo test -p backend model_stream_native_runtime_api --offline`

Expected: PASS.

---

### Task 2: Route Agent Model Loop Through Stream Facade

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Agent loop consumes `ModelChatStreamCall` instead of calling `provider_stream_channel()` and injecting `provider_stream_sender`.

- [ ] **Step 1: Write failing Agent source-contract test**

Add a backend unit test named `agent_model_loop_uses_stream_native_runtime_api`. It should inspect `execute_model_loop_existing_run` and assert it contains `chat_completion_stream_for_purpose`, does not contain `provider_stream_channel`, and does not set `provider_stream_sender: Some`.

- [ ] **Step 2: Run focused test red**

Run: `cargo test -p backend model_stream_native_runtime_api --offline`

Expected: FAIL because Agent still wires provider stream channels itself.

- [ ] **Step 3: Update Agent model loop**

Replace local `provider_stream_channel` construction with `let model_stream_call = self.model_runtime.chat_completion_stream_for_purpose(...).await?;`. Pass `model_stream_call.events` and `model_stream_call.response` into `await_model_loop_provider_future_or_cancelled_with_delta_events`.

- [ ] **Step 4: Remove dead Agent channel helper and update matrix**

Delete the now-unused Agent-side `provider_stream_channel` helper. Update Runtime loop matrix status to the next slice and narrow the remaining gap to the internal fully incremental completion builder.

- [ ] **Step 5: Verify**

Run:

```bash
cargo fmt -- --check
cargo test -p backend model_stream_native_runtime_api --offline
cargo test -p backend provider_stream_native_cancel --offline
cargo test -p backend streamed_tool_call_early_stop --offline
cargo test --workspace --offline
git diff --check
```

Expected: all commands pass.
