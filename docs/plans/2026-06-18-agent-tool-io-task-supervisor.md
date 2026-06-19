# Agent Tool I/O Task Supervisor Plan

## Goal

Replace the parallel Agent tool I/O `join_all` polling path with an owned, abort-on-drop task supervisor helper. This moves the Parallel tools migration one step closer to Codex-style task ownership while preserving deterministic result ordering, timeout cancellation payloads, runtime-registry cancellation behavior, and serial audit/persistence in `AgentService`.

## Architecture

Extend `backend/src/application/ai/agent_tool_io_runtime.rs` with a private `AgentToolIoTask` wrapper around `tokio::task::JoinHandle<Result<ExecutedAgentToolCall, AppError>>`, following the existing `ModelChatStreamTransportTask` abort-on-drop pattern in `model_service.rs`. The parallel batch path spawns one owned task per prepared tool call and awaits task handles in input order; dropping not-yet-awaited tasks aborts pending work. The serial path continues to await directly without spawning background tasks.

## Scope

- Add an `AgentToolIoTask` helper with `spawn`, `wait`, and `Drop` abort behavior.
- Replace the parallel `join_all` implementation in `execute_agent_tool_io_batch` with supervised task handles.
- Preserve ordered `Vec<ExecutedAgentToolCall>` output for parallel batches.
- Preserve existing external-cancel and timeout response payload shapes.
- Adjust the model-loop call site so the tool I/O future is spawn-safe by capturing a cloned `AgentService`.
- Update migration matrix acceptance evidence for the Parallel tools row.

## Out of Scope

- Changing approval, Guardian review, tool routing, concrete executor dispatch, media persistence, or audit/event ordering.
- Moving tool execution into a global worker pool or queue.
- Adding metrics/tracing for each spawned tool task.
- Changing serial execution semantics.

## RED Tests

- Runtime source-contract test: `agent_tool_io_runtime.rs` defines `AgentToolIoTask`, stores a `JoinHandle<Result<ExecutedAgentToolCall, AppError>>`, implements `Drop`, calls `handle.abort()`, uses `AgentToolIoTask::spawn` in the parallel branch, and no longer imports/uses `join_all`.
- Runtime behavior test: dropping a pending `AgentToolIoTask` aborts the underlying future, verified with an `AbortGuard` oneshot signal.
- Existing behavioral tests remain green:
  - `parallel_tool_io_batch_polls_calls_concurrently_and_preserves_order`
  - `serial_tool_io_batch_runs_calls_in_sequence`
  - `tool_io_timeout_returns_cancelled_execution`
  - `tool_io_runtime_registry_cancel_returns_external_cancel_execution`
  - `agent_service_model_loop_executes_parallel_batches_via_io_executor`

## Implementation Steps

1. Add RED source-contract and abort-on-drop tests in `agent_tool_io_runtime.rs`.
2. Introduce private `AgentToolIoTask` with owned `JoinHandle` and abort-on-drop behavior.
3. Replace parallel `join_all` with ordered task spawning/waiting.
4. Tighten `execute_agent_tool_io_batch` generic bounds for spawned futures and update `AgentService` to pass a cloned service into the tool I/O closure.
5. Update migration matrix Parallel tools status and acceptance evidence.
6. Verify focused tests, model loop, full workspace, merge to `main`, remove worktree branch, and run `cargo clean`.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p backend agent_tool_io_runtime --offline`
- `cargo test -p backend parallel_tool_io_batch --offline`
- `cargo test -p backend tool_io_timeout --offline`
- `cargo test -p backend runtime_registry --offline`
- `cargo test -p backend parallel_tool --offline`
- `cargo test -p backend model_loop --offline`
- `cargo test --workspace --offline`
