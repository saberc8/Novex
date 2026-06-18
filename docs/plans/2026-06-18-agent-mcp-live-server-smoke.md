# Agent MCP Live Server Smoke Plan

## Goal

Add a deterministic local live-server smoke for MCP Streamable HTTP tool execution so the backend proves the real `reqwest` dispatch path, protocol headers, bearer auth, JSON-RPC body, response parsing, and safe audit payload without depending on an external MCP service.

## Architecture

Keep the production transport boundary from the previous slice: `novex-mcp` owns request/response protocol contracts, while `backend/src/application/ai/agent_tool_executor.rs` owns live HTTP dispatch and execution evidence. Add a test-only local HTTP server harness inside the backend executor tests. The harness listens on `127.0.0.1:0`, receives a real POST from `dispatch_mcp_streamable_http_request`, validates MCP headers/body/auth, returns a JSON-RPC `tools/call` result, and lets the existing live execution helper parse it into `AgentToolExecution`.

This is live acceptance coverage, not an external integration dependency: it validates the same network code used by production while remaining offline and deterministic for CI.

## Scope

- Add a backend unit/integration-style test that starts a local TCP listener and serves one MCP Streamable HTTP `tools/call` response.
- Exercise the real `dispatch_mcp_streamable_http_request` function instead of a fake dispatcher.
- Verify `Authorization: Bearer <token>`, `Accept`, `Content-Type`, `MCP-Protocol-Version`, JSON-RPC method, tool name, and tool arguments.
- Verify the resulting `AgentToolExecution` is live, non-dry-run, non-mocked, and preserves structured response content.
- Verify the execution payload contains only sanitized auth/request evidence and does not leak the resolved token.
- Update the migration matrix and enterprise foundation plan so MCP marks local live-server smoke coverage as implemented while external MCP server smoke, stdio lifecycle, OAuth, and persisted sessions remain follow-up work.

## Out of Scope

- Starting or managing real external MCP servers.
- Stdio MCP process supervision.
- OAuth/browser authorization.
- Persisted MCP session IDs or resumable streams.
- Admin UI changes.
- Adding a public CLI smoke command.

## RED Tests

- `mcp_tool_execution_live_http_dispatch_reaches_local_streamable_http_server`: starts a local server, executes a live-enabled MCP tool through the real HTTP dispatcher, asserts the server received expected headers/body/auth, and asserts the execution response maps the JSON-RPC result without leaking the token.

## Implementation Steps

1. Add RED test helpers in `backend/src/application/ai/agent_tool_executor.rs` test module:
   - `LocalMcpServerCapture` for observed headers/body/auth.
   - `run_one_shot_mcp_server` that listens on `127.0.0.1:0`, parses one HTTP request through axum, sends captured request data over a oneshot channel, and returns a JSON-RPC `tools/call` result.
2. Add RED test `mcp_tool_execution_live_http_dispatch_reaches_local_streamable_http_server` that calls `execute_mcp_tool_with_http_dispatch` with the real `dispatch_mcp_streamable_http_request` adapter.
3. Run `cargo test -p backend-rust mcp_tool_execution_live_http_dispatch_reaches_local_streamable_http_server --offline` and confirm it fails because the harness/helper does not exist yet.
4. Implement the smallest test-only server harness using existing backend dependencies: `axum`, `tokio`, `serde_json`, and `std::net`.
5. Keep production code unchanged unless the local smoke exposes a real defect in dispatch behavior.
6. Run focused MCP tests, backend MCP tests, formatting, whitespace checks, and full workspace tests.
7. Update docs, commit implementation, fast-forward merge to `main`, run `cargo clean`, remove this worktree, and delete the feature branch.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p backend-rust mcp_tool_execution_live_http_dispatch_reaches_local_streamable_http_server --offline`
- `cargo test -p backend-rust mcp_tool_execution --offline`
- `cargo test -p backend-rust mcp --offline`
- `cargo test -p novex-mcp mcp_streamable_http --offline`
- `cargo test --workspace --offline`
