# Agent Stream Transport Executor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `ModelChatStreamTransportTask` execute through a stream-specific model runtime path instead of delegating to the public unary chat facade.

**Architecture:** Adapter-port Codex's owned task/session boundary: the Agent stream call owns a transport task, and that task owns model execution lifecycle. Novex keeps its enterprise route resolution, fallback, provider-call lease, usage/cost accounting, and SSE completion builder, but moves the stream task entry point from `chat_completion_for_purpose` to a private `execute_chat_completion_stream_transport` executor.

**Tech Stack:** Rust, Tokio `JoinHandle`, backend model runtime service, provider route fallback, source-contract tests.

## Global Constraints

- Do not change provider request payload shape, response parsing, route policy, fallback policy, provider-call lease persistence, usage recording, or cost accounting.
- Do not remove or change `chat_completion_for_purpose`; unary callers keep the public API.
- Preserve `ModelChatStreamTransportTask` abort-on-drop semantics and Agent-side stream event draining.
- The stream transport task must not call `chat_completion_for_purpose`.
- The stream transport executor must normalize the command, resolve the configured route, and call the existing fallback executor directly.
- This slice does not split `execute_normalized_chat_completion_with_route`; route-level HTTP/SSE split remains the next provider-native transport step.

---

### Task 1: Introduce Stream Transport Executor

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Adds private `ModelRuntimeService::execute_chat_completion_stream_transport(purpose, command) -> Result<ModelChatResp, AppError>`.
- `chat_completion_stream_for_purpose` spawns this private executor instead of the public unary `chat_completion_for_purpose`.
- Existing `ModelChatStreamTransportTask::spawn` API remains unchanged.

- [ ] **Step 1: Write failing source-contract test**

Add backend test `model_stream_transport_task_uses_stream_specific_executor`. It should inspect the production body from `pub async fn chat_completion_stream_for_purpose` through `async fn execute_normalized_chat_completion_with_provider_call_lease`. Assert that body contains `execute_chat_completion_stream_transport`, `normalize_model_chat_command(command)?`, `resolve_route_for_purpose_with_route_id`, and `execute_normalized_chat_completion_with_fallback`. Assert the stream facade body does not contain `chat_completion_for_purpose(purpose, command)`.

- [ ] **Step 2: Run focused test red**

Run: `cargo test -p backend model_stream_transport_executor --offline`

Expected: FAIL because the stream facade still spawns `chat_completion_for_purpose`.

- [ ] **Step 3: Implement stream transport executor**

Add a private method near `chat_completion_stream_for_purpose`:

```rust
async fn execute_chat_completion_stream_transport(
    &self,
    purpose: ModelRoutePurpose,
    command: ModelChatCommand,
) -> Result<ModelChatResp, AppError> {
    let command = normalize_model_chat_command(command)?;
    let route = self
        .resolve_route_for_purpose_with_route_id(purpose, command.route_id.as_deref())
        .await?
        .ok_or_else(|| AppError::bad_request("LLM 模型环境变量未配置完整"))?;
    self.execute_normalized_chat_completion_with_fallback(
        purpose,
        &route,
        &command,
        command.conversation_id,
    )
    .await
}
```

Change the task spawn in `chat_completion_stream_for_purpose` to:

```rust
let transport = ModelChatStreamTransportTask::spawn(async move {
    service
        .execute_chat_completion_stream_transport(purpose, command)
        .await
});
```

- [ ] **Step 4: Verify green**

Run: `cargo test -p backend model_stream_transport_executor --offline`

Expected: PASS.

---

### Task 2: Update Migration Matrix And Regression Gates

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Runtime loop matrix status advances from `slice-58 implemented` to `slice-59 implemented`.
- Acceptance evidence includes `model_stream_transport_executor`.
- Remaining gap narrows to route-level provider stream/unary split.

- [ ] **Step 1: Update source contracts if needed**

If existing stream transport tests mention only the task wrapper, keep them and add the executor-specific test. Do not weaken `model_stream_transport_task_drop_aborts_provider_task`.

- [ ] **Step 2: Update migration matrix**

Update the Runtime loop row to mention the private stream transport executor, update Runtime loop POC evidence to say `ModelChatStreamTransportTask` runs the stream-specific executor, add `cargo test -p backend model_stream_transport_executor --offline` to the verification command, and add this plan to Follow-up Implementation Plans.

- [ ] **Step 3: Verify**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend model_stream_transport_executor --offline
cargo test -p backend model_stream_transport_task --offline
cargo test -p backend model_stream_call_lifecycle --offline
cargo test -p backend model_stream_native_runtime_api --offline
cargo test -p backend model_stream_completion_builder --offline
cargo test -p backend streamed_tool_call_early_stop --offline
cargo test -p backend provider_abort --offline
cargo test --workspace --offline
```

Expected: all commands pass.
