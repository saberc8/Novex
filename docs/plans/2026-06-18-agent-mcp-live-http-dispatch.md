# Agent MCP Live HTTP Dispatch Plan

## Goal

Move backend MCP tool execution from sanitized dry-run planning to gated live Streamable HTTP dispatch, using the existing `novex-mcp` request/response contract and keeping secrets out of persisted execution payloads.

## Architecture

Keep `novex-mcp` transport-neutral: it owns JSON-RPC `tools/call` request planning and JSON/SSE response parsing, but not `reqwest`, environment secrets, backend audit payloads, or tenant policy. Add a narrow backend-local dispatch boundary in `backend/src/application/ai/agent_tool_executor.rs`: production code resolves the configured secret, sends the request with `reqwest`, parses the response through `novex-mcp`, and records sanitized evidence; tests inject a fake dispatcher so no network is needed.

This is an adapter-port slice from Codex-style tool execution: backend decides whether a tool may run live, executes through a small boundary, captures structured observations, and fails closed without leaking bearer tokens.

## Scope

- Add a backend-local `execute_mcp_tool_with_http_dispatch` helper that accepts an environment resolver and dispatch function for deterministic tests.
- Gate live MCP execution with `metadata.liveExecutionEnabled == true`.
- Keep `metadata.mockResponse` behavior unchanged and highest priority.
- Keep non-live MCP tools on the current dry-run path with `liveRequest` evidence.
- Resolve `env:` secret refs for live execution and pass the bearer token only to the dispatch boundary.
- Send Streamable HTTP `POST` requests with the headers/body from `McpStreamableHttpRequestPlan`.
- Parse successful JSON or SSE responses through `parse_mcp_tool_call_response`.
- Return safe failed `AgentToolExecution` payloads for missing endpoint, missing required secret, HTTP dispatch errors, and MCP parse errors.
- Update the migration matrix and enterprise foundation plan so MCP shows live HTTP dispatch progress while stdio lifecycle, OAuth, and persisted MCP sessions remain future slices.

## Out of Scope

- MCP stdio process lifecycle and subprocess supervision.
- OAuth/browser authorization.
- Long-lived MCP session persistence.
- Resource subscriptions, streaming progress events, and resumable MCP streams.
- Admin UI changes for MCP server configuration.
- Running real external MCP servers in tests.

## RED Tests

- `mcp_tool_execution_live_http_dispatch_uses_streamable_http_plan`: a live-enabled non-mock MCP tool calls the injected dispatcher with the `tools/call` plan, bearer token, protocol headers, and model-provided arguments.
- `mcp_tool_execution_live_http_dispatch_maps_json_response_without_leaking_secret`: a fake JSON-RPC result maps to a non-dry-run successful execution and the serialized payload does not contain the resolved token.
- `mcp_tool_execution_live_http_dispatch_failure_returns_safe_payload`: dispatch failure returns a failed execution with sanitized request/auth evidence and no resolved token.
- Existing tests continue to prove mock response priority and dry-run `liveRequest` behavior.

## Implementation Steps

1. Add RED backend tests in `backend/src/application/ai/agent_tool_executor.rs` using a fake dispatcher and live-enabled MCP metadata.
2. Run `cargo test -p backend mcp_tool_execution_live_http_dispatch --offline` and confirm the tests fail because the helper/live dispatch behavior does not exist.
3. Add backend imports for `McpStreamableHttpResponse`, `parse_mcp_tool_call_response`, and `std::future::Future`.
4. Extract `mcp_streamable_http_request_plan` so dry-run evidence and live dispatch share one request-plan builder.
5. Add `mcp_live_execution_enabled`, `mcp_auth_requires_secret`, and sanitized auth payload helpers.
6. Implement `execute_mcp_tool_with_http_dispatch` with mock-first, dry-run fallback, live gate, secret resolution, dispatch invocation, parse mapping, and safe failure payloads.
7. Wire existing `execute_mcp_tool` to call the helper with real environment resolution and `reqwest` dispatch.
8. Implement `dispatch_mcp_streamable_http_request` with a short timeout, configured headers, optional bearer auth, JSON body, response status/content-type/body capture, and safe string errors.
9. Run focused backend MCP tests, then MCP crate tests, then full workspace verification.
10. Update planning docs, commit implementation, fast-forward merge to `main`, run `cargo clean`, and remove this worktree/branch.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p backend mcp_tool_execution_live_http_dispatch --offline`
- `cargo test -p backend mcp_tool_execution --offline`
- `cargo test -p backend mcp --offline`
- `cargo test -p novex-mcp mcp_streamable_http --offline`
- `cargo test --workspace --offline`
