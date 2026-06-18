# Agent Tool Executor Dispatch Plan

## Goal

Move the next piece of Codex-style tool infrastructure into `novex-tools`: a serializable executor dispatch plan derived from `ToolExecutorBinding`. Backend tool I/O should use this plan to decide runtime dependencies and prefer executor-code dispatch, while retaining existing `tool.code` fallback behavior for deterministic legacy paths.

## Scope

- Add `ToolExecutorDispatchPlan` to `novex-tools`.
- Derive dependency flags from executor kind:
  - connector executors require connector credentials
  - MCP executors require MCP tool lookup
  - model executors require model runtime
- Preserve background-task and runtime-cancellation capability flags from the binding.
- Backend model-loop tool I/O derives the dispatch plan from `PreparedAgentToolCall.executor_binding`.
- `execute_agent_tool` receives the plan and dispatches by executor code when present.

## Out of Scope

- Moving GitHub, Feishu, media, RAG, or MCP executor implementations out of backend.
- Changing tool outputs or external side effects.
- Adding dynamic executor plugins.

## RED Tests

- `novex-tools` unit test: dispatch plan derives connector/model/MCP dependency flags and preserves capability flags.
- backend source-contract test: `execute_agent_tool_io` builds `ToolExecutorDispatchPlan::from_binding`, uses plan dependency flags, passes the plan to `execute_agent_tool`, and `execute_agent_tool` prefers executor-code dispatch before legacy tool-code fallback.

## Implementation Steps

1. Add `ToolExecutorDispatchPlan` and `from_binding` to `novex-tools`.
2. Import the plan in backend Agent service.
3. Build `executor_dispatch` from `prepared.executor_binding`.
4. Use dispatch plan flags for connector credential and MCP lookup decisions, with existing legacy fallback.
5. Pass `executor_dispatch.as_ref()` into `execute_agent_tool`.
6. Prefer executor-code checks in `execute_agent_tool`, with existing tool-code checks as compatibility fallback.
7. Update migration matrix and verification commands.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p novex-tools tool_executor_dispatch_plan --offline`
- `cargo test -p backend-rust tool_executor_dispatch_plan --offline`
- `cargo test --workspace --offline`
