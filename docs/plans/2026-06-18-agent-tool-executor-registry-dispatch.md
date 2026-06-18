# Agent Tool Executor Registry Dispatch Plan

## Goal

Make the backend model loop consume the `novex-tools` executor registry when preparing, tracing, and auditing tool calls. This keeps concrete tool execution in the backend for now, while making the Codex-style executor boundary observable and testable.

## Scope

- Build the model-loop `ToolExecutorRegistry` from `agent_model_loop_tool_executor_bindings()`.
- Resolve every routed model-loop tool call through the registry before it can be prepared for execution.
- Carry the selected `ToolExecutorBinding` through `PreparedAgentToolCall`.
- Persist executor binding metadata in action-selected events and tool-call audit requests.
- Preserve the existing backend executors and code-based dispatch behavior.

## Out of Scope

- Moving connector, MCP, media, or sandbox execution out of backend.
- Introducing dynamic plugin loading for executors.
- Changing user-visible tool outputs.

## RED Tests

- Add a pure payload test proving executor binding metadata serializes with `executorCode`, `kind`, and capability flags.
- Add a source-contract test proving backend uses `ToolExecutorRegistry::from_bindings(agent_model_loop_tool_executor_bindings())`, resolves routed calls through `executor_for`, stores the binding on `PreparedAgentToolCall`, and reads it in audit/event payloads.

## Implementation Steps

1. Import the executor registry types from `novex-tools`.
2. Add `executor_binding: Option<ToolExecutorBinding>` to `PreparedAgentToolCall`.
3. Add a small helper for building the model-loop executor registry and a small helper for serializing binding payloads.
4. Resolve executor bindings while preparing routed model-loop tool calls.
5. Add `executorBinding` to waiting-approval and running `ActionSelected` payloads.
6. Add `executorBinding` to tool-call audit request payloads.
7. Update test helpers and migration matrix.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p backend-rust tool_executor_registry --offline`
- `cargo test -p backend-rust model_loop_tool_executor_binding_payload --offline`
- `cargo test --workspace --offline`
