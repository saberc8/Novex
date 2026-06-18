# Agent Parallel Tool Task Control Plan

## Goal

Move backend Agent tool I/O batch task control out of `agent_service.rs` into a focused runtime helper module. This makes the Codex-style parallel tool loop boundary explicit and gives future background `JoinHandle`/abort-on-drop control a stable place to evolve without mixing concurrency details with AgentService persistence and run-state orchestration.

## Architecture

Add `backend/src/application/ai/agent_tool_io_runtime.rs` as a backend-local runtime adapter. The module owns serial-vs-parallel polling, per-call timeout handling, and run-cancellation mapping into `AgentToolExecution::cancelled`. `AgentService` remains responsible for prepared call construction, DB lookups, concrete tool execution through `agent_tool_executor`, audit/event persistence, media persistence, and final run status transitions.

## Scope

- Move `execute_agent_tool_io_batch` into `agent_tool_io_runtime.rs`.
- Move the per-call timeout/cancellation wrapper into `agent_tool_io_runtime.rs`.
- Expose the minimal `pub(super)` fields on `PreparedAgentToolCall` and `ExecutedAgentToolCall` needed by the sibling runtime module.
- Keep the current `join_all` parallel behavior and deterministic result ordering.
- Preserve all existing response payload shapes for external cancellation and timeout cancellation.
- Keep serial persistence in `AgentService`.

## Out of Scope

- Replacing `join_all` with `JoinSet` in this slice.
- Adding a new queue worker or background supervisor.
- Changing approval, policy, tool routing, concrete executor behavior, or audit/event payloads.
- Moving media job/asset persistence out of `AgentService`.

## RED Tests

- Backend source-contract test: `agent_service.rs` imports `execute_agent_tool_io_batch` from `agent_tool_io_runtime` and no longer defines local `execute_agent_tool_io_batch` or `execute_agent_tool_io_with_timeout_and_cancel`.
- Backend runtime-module source-contract test: `agent_tool_io_runtime.rs` owns `execute_agent_tool_io_batch` and `execute_agent_tool_io_with_timeout_and_cancel`.
- Existing behavioral tests stay green:
  - `parallel_tool_io_batch_polls_calls_concurrently_and_preserves_order`
  - `serial_tool_io_batch_runs_calls_in_sequence`
  - `tool_io_timeout_returns_cancelled_execution`
  - `tool_io_runtime_registry_cancel_returns_external_cancel_execution`
  - `agent_service_parallel_tool_execution_separates_io_from_persistence`

## Implementation Steps

1. Add RED source-contract tests in `agent_service.rs` and the new runtime module.
2. Add module declaration in `backend/src/application/ai/mod.rs`.
3. Move batch execution and per-call timeout/cancel wrapper into `agent_tool_io_runtime.rs`.
4. Update imports and field visibility so `AgentService` calls the runtime helper.
5. Update the migration matrix Parallel tools row and acceptance evidence.
6. Verify focused tests, model loop, and full workspace; merge to `main`; run `cargo clean` in both worktrees.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p backend-rust agent_tool_io_runtime --offline`
- `cargo test -p backend-rust parallel_tool_io_batch --offline`
- `cargo test -p backend-rust tool_io_timeout --offline`
- `cargo test -p backend-rust runtime_registry --offline`
- `cargo test -p backend-rust parallel_tool --offline`
- `cargo test -p backend-rust model_loop --offline`
- `cargo test --workspace --offline`
