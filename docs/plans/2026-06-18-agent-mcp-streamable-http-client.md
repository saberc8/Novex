# Agent MCP Streamable HTTP Client Plan

## Goal

Move the MCP gateway one step beyond mock/dry-run execution by adding a reusable Streamable HTTP client contract in `novex-mcp`. The contract should produce sanitized JSON-RPC `tools/call` request plans, parse JSON and SSE-style responses, and expose deterministic errors that backend agent execution can later route through a real HTTP transport.

## Architecture

Keep `novex-mcp` as the pure protocol/policy crate. It should not own `reqwest`, SQLx, secrets, tenant authorization, or backend audit. Instead, it builds transport-agnostic request plans and response parsers that backend code can use safely. Backend MCP execution keeps the current mock-first behavior, but the non-mocked dry-run response should include the same sanitized live request plan that a future transport will send.

This is an adapter port from Codex MCP ideas and current MCP Streamable HTTP transport conventions: JSON-RPC POST requests, `tools/call`, `Accept: application/json, text/event-stream`, `Content-Type: application/json`, and protocol-version metadata.

## Scope

- Add `McpJsonRpcRequest` and `McpStreamableHttpRequestPlan` to `crates/novex-mcp`.
- Add a builder for sanitized `tools/call` Streamable HTTP request plans.
- Add JSON-RPC response parsing for successful `tools/call` results.
- Add SSE `data:` response parsing for Streamable HTTP servers that answer through event streams.
- Add deterministic MCP client errors for HTTP status, unsupported content type, JSON-RPC error, and malformed JSON.
- Add backend dry-run payload evidence under `liveRequest` for non-mocked MCP tools without exposing bearer tokens.
- Update the migration matrix so MCP shows transport-contract progress while backend live dispatch remains next.

## Out of Scope

- Sending network requests from backend.
- OAuth/browser authorization.
- stdio MCP process lifecycle.
- Long-lived resource subscriptions or streaming tool progress.
- Persisting MCP sessions.
- Changing existing mock-response execution behavior.

## RED Tests

- `novex-mcp` request-plan test: `tools/call` request uses POST, JSON-RPC 2.0, `Accept` includes both JSON and event-stream, `Content-Type` is JSON, and no bearer token appears in serialized request evidence.
- `novex-mcp` JSON response test: a JSON-RPC `result` maps to `McpToolInvocationResult` with `status = "succeeded"` and preserves `content`/`structuredContent`.
- `novex-mcp` SSE response test: an event-stream `data:` JSON-RPC message maps to the same result contract.
- `novex-mcp` error test: JSON-RPC `error` maps to a structured client error with kind/code/message.
- Backend source/behavior test: non-mocked MCP dry-run includes `liveRequest` request-plan metadata and does not include resolved secret values.

## Implementation Steps

1. Add RED tests in `crates/novex-mcp/tests/streamable_http.rs` for request planning, JSON response parsing, SSE response parsing, and JSON-RPC errors.
2. Add backend RED test in `backend/src/application/ai/agent_tool_executor.rs` for sanitized `liveRequest` evidence in non-mocked MCP dry-runs.
3. Implement the request-plan and response-parser types in `novex-mcp`.
4. Wire backend non-mocked MCP dry-run payloads to include the sanitized request plan.
5. Update `docs/plans/2026-06-16-codex-migration-matrix.md`.
6. Verify focused tests and full workspace, merge to `main`, run `cargo clean`, and remove this worktree branch.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p novex-mcp mcp_streamable_http --offline`
- `cargo test -p backend-rust mcp_tool_execution --offline`
- `cargo test -p backend-rust mcp --offline`
- `cargo test --workspace --offline`
