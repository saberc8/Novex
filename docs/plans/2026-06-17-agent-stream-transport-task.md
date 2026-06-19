# Agent Stream Transport Task Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the public boxed stream response future with an owned stream transport task boundary.

**Architecture:** Adapter-port Codex's task-owned lifecycle pattern from `codex-rs/core/src/state/turn.rs`, where active work is held by an abort-on-drop task handle. Novex keeps the existing provider call, fallback, provider-call lease, SSE completion builder, and Agent drain loop behavior, but exposes `ModelChatStreamTransportTask` instead of `Pin<Box<dyn Future<...>>>` on `ModelChatStreamCall`.

**Tech Stack:** Rust, Tokio `JoinHandle`, backend model runtime service, Agent model loop, source-contract tests.

## Global Constraints

- Do not change provider request payloads, provider route selection, retry policy, fallback policy, provider-call lease persistence, or SSE parsing.
- Do not remove `chat_completion_for_purpose`; non-stream callers keep the unary API.
- Preserve streamed tool-call early-stop, provider-native cancel dispatch, provider response id/status capture, provider-call lease id propagation, model_delta events, and `ModelChatStreamCompletionBuilder`.
- The Agent model loop must consume one `ModelChatStreamCall` object and must not access a public boxed future.
- Dropping an unfinished stream transport task must abort the spawned provider task so early-stop and cancellation do not detach background provider work.
- Use no new dependency; implement abort-on-drop semantics with Tokio's existing `JoinHandle`.

---

### Task 1: Add Owned Stream Transport Task Type

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Adds public `ModelChatStreamTransportTask`.
- Removes public `ModelChatStreamFuture`.
- Changes `ModelChatStreamCall` from `pub response: ModelChatStreamFuture` to `pub transport: ModelChatStreamTransportTask`.
- Adds `ModelChatStreamTransportTask::spawn(future)` and `ModelChatStreamTransportTask::wait(self)`.

- [ ] **Step 1: Write failing source-contract test**

Add backend tests `model_stream_transport_task_replaces_boxed_future` and `model_stream_transport_task_drop_aborts_provider_task`. The source-contract test should assert production source contains `pub struct ModelChatStreamTransportTask`, `pub transport: ModelChatStreamTransportTask`, `tokio::spawn`, `JoinHandle<Result<ModelChatResp, AppError>>`, and a `Drop for ModelChatStreamTransportTask` implementation. It should also assert the production source no longer contains `pub type ModelChatStreamFuture` or `pub response: ModelChatStreamFuture`. The behavior test should spawn a never-completing transport task, wait until it starts, drop the task, and assert an in-future drop guard fires within one second.

- [ ] **Step 2: Run focused test red**

Run: `cargo test -p backend model_stream_transport_task --offline`

Expected: FAIL because `ModelChatStreamTransportTask` does not exist and `ModelChatStreamFuture` still exists.

- [ ] **Step 3: Implement transport task type**

Near `ModelChatStreamLifecycle`, add:

```rust
pub struct ModelChatStreamTransportTask {
    handle: Option<tokio::task::JoinHandle<Result<ModelChatResp, AppError>>>,
}

impl ModelChatStreamTransportTask {
    fn spawn<F>(future: F) -> Self
    where
        F: Future<Output = Result<ModelChatResp, AppError>> + Send + 'static,
    {
        Self {
            handle: Some(tokio::spawn(future)),
        }
    }

    pub async fn wait(mut self) -> Result<ModelChatResp, AppError> {
        let handle = self
            .handle
            .take()
            .ok_or_else(|| AppError::Anyhow(anyhow::anyhow!("model stream transport task already awaited")))?;
        handle
            .await
            .map_err(|error| AppError::Anyhow(anyhow::anyhow!("model stream transport task failed: {error}")))?
    }
}

impl Drop for ModelChatStreamTransportTask {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}
```

Update `ModelChatStreamCall` to use `transport`. In `chat_completion_stream_for_purpose`, call `ModelChatStreamTransportTask::spawn(async move { service.chat_completion_for_purpose(purpose, command).await })`.

- [ ] **Step 4: Verify green**

Run: `cargo test -p backend model_stream_transport_task --offline`

Expected: PASS.

---

### Task 2: Await Transport Task From Agent Stream Call

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Agent helper destructures `transport` and awaits `transport.wait()` inside its existing cancellation-aware `tokio::select!`.
- Existing source-contract tests for stream-native API and lifecycle are updated from `response` to `transport`.
- Migration matrix advances the runtime-loop slice and records this task boundary as complete.

- [ ] **Step 1: Write failing Agent contract test**

Add backend test `agent_model_stream_transport_task_waits_without_boxed_future`. It should inspect `await_model_loop_stream_call_or_cancelled_with_delta_events` and assert it destructures `transport`, calls `transport.wait()`, and does not contain `let future = response`.

- [ ] **Step 2: Run focused test red**

Run: `cargo test -p backend model_stream_transport_task --offline`

Expected: FAIL because Agent still destructures `response` and pins the boxed future.

- [ ] **Step 3: Update Agent await helper**

Change:

```rust
let ModelChatStreamCall {
    lifecycle: _lifecycle,
    response,
    events: mut provider_stream_receiver,
} = model_stream_call;
let future = response;
```

to:

```rust
let ModelChatStreamCall {
    lifecycle: _lifecycle,
    transport,
    events: mut provider_stream_receiver,
} = model_stream_call;
let future = transport.wait();
```

Keep `tokio::pin!(future)` and the existing select branches unchanged.

- [ ] **Step 4: Update existing source-contract tests and matrix**

Update `model_stream_native_runtime_api_exposes_future_and_event_receiver`, `agent_model_stream_call_lifecycle_owns_future_and_events`, and migration matrix wording so the contract says typed stream calls expose a transport task and event receiver instead of a boxed response future.

- [ ] **Step 5: Verify**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend model_stream_transport_task --offline
cargo test -p backend model_stream_call_lifecycle --offline
cargo test -p backend model_stream_native_runtime_api --offline
cargo test -p backend model_stream_completion_builder --offline
cargo test -p backend streamed_tool_call_early_stop --offline
cargo test -p backend provider_abort --offline
cargo test --workspace --offline
```

Expected: all commands pass.
