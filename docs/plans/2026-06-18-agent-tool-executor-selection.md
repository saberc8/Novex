# Agent Tool Executor Selection Plan

## Goal

Start concrete executor extraction by moving backend tool executor selection out of `agent_service.rs` into a focused backend adapter module. This keeps existing tool implementations in place while making the dispatch boundary explicit, testable, and ready for later connector/MCP/sandbox executor extraction.

## Scope

- Add `backend/src/application/ai/agent_tool_executor.rs`.
- Move canonical built-in Agent tool code constants into the new module.
- Add `AgentToolExecutorSelection` for:
  - MCP
  - Feishu message
  - media image generation
  - GitHub repo search
  - GitHub repo read
  - dry-run fallback
- Add helper functions for GitHub credential lookup and MCP lookup decisions.
- Update `agent_service.rs` to ask the selection module what to run, while preserving all existing execution functions and outputs.

## Out of Scope

- Moving actual GitHub, Feishu, media, or MCP execution functions out of `agent_service.rs`.
- Adding sandbox execution.
- Changing tool outputs, audits, events, or external side effects.

## RED Tests

- Backend source-contract test: `agent_service` must call `AgentToolExecutorSelection::from_dispatch`, must use `agent_tool_requires_github_connector_credential`, and must use `agent_tool_requires_mcp_lookup`.
- Backend unit test in `agent_tool_executor.rs`: selection prefers executor code from `ToolExecutorDispatchPlan` and preserves legacy tool-code fallbacks.
- Backend unit test in `agent_tool_executor.rs`: dependency helpers request GitHub credentials and MCP lookup only for the expected executor/tool combinations.

## Implementation Steps

1. Add RED source-contract test in `agent_service.rs`.
2. Add `agent_tool_executor.rs` with failing tests and module declaration.
3. Implement `AgentToolExecutorSelection` and dependency helpers.
4. Replace inline selection logic in `execute_agent_tool_io` and `execute_agent_tool`.
5. Update migration matrix to Tool router slice 5.
6. Verify and merge to main, then run `cargo clean` in main and feature worktree.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p backend agent_tool_executor_selection --offline`
- `cargo test -p backend tool_executor_dispatch_plan --offline`
- `cargo test --workspace --offline`
