# Agent Concrete Tool Executors Plan

## Goal

Move concrete backend Agent tool execution implementations out of `agent_service.rs` into the focused `agent_tool_executor` adapter module. `AgentService` should keep preparing routed calls, checking policy, recording audit/events, persisting media records, and driving the run state machine; the backend tool executor module should own Feishu, GitHub, MCP mock/dry-run, media-image execution, dry-run fallback, and executor-code selection.

## Scope

- Extend `backend/src/application/ai/agent_tool_executor.rs` from selection-only to selection plus execution.
- Move `execute_agent_tool`, `execute_mcp_tool`, `execute_feishu_message_tool`, `execute_media_image_tool`, GitHub search/read execution, and their private helpers into the executor module.
- Keep shared request parsing in `novex-tools`.
- Keep `media_records_from_tool_execution` and all persistence/audit logic in `agent_service.rs`.
- Preserve existing response payload shapes, dry-run behavior, final-output text, and error messages.

## Out of Scope

- Live MCP streaming client.
- Replacing direct `reqwest` GitHub/Feishu calls with connector trait objects.
- Moving media job/asset persistence out of `AgentService`.
- Changing tool approval, policy, timeout, cancellation, batch planning, or event payload contracts.

## RED Tests

- Backend source-contract test: `agent_service.rs` no longer defines concrete executor functions and imports `execute_agent_tool` from `agent_tool_executor`.
- Backend executor module source-contract test: `agent_tool_executor.rs` owns the concrete Feishu/GitHub/MCP/media executor functions.
- Existing behavior tests move or continue to pass:
  - MCP mock response hides resolved secret value.
  - Feishu webhook config trims configured URL.
  - GitHub credential selection prefers DB secret refs and falls back to env.
  - Media image tool still uses tenant-bound `ModelRoutePurpose::MediaGeneration`.

## Implementation Steps

1. Add RED source-contract tests in `agent_service.rs` and `agent_tool_executor.rs`.
2. Move Feishu webhook config, MCP payload helpers, GitHub auth/client helpers, and concrete executor functions into `agent_tool_executor.rs`.
3. Make `execute_agent_tool` public within the application module and update `agent_service.rs` to call the imported function.
4. Trim `agent_service.rs` imports/constants that only served concrete tool I/O.
5. Update the migration matrix Tool router row and acceptance evidence to mention backend concrete executor module extraction.
6. Verify focused tests, model loop, and full workspace; merge to `main`; run `cargo clean` in both worktrees.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p backend-rust agent_concrete_tool_executors --offline`
- `cargo test -p backend-rust mcp_tool_execution --offline`
- `cargo test -p backend-rust github_connector_auth --offline`
- `cargo test -p backend-rust media_image_tool_uses_tenant_bound_model_route --offline`
- `cargo test -p backend-rust agent_tool_executor_selection --offline`
- `cargo test -p backend-rust model_loop --offline`
- `cargo test --workspace --offline`
