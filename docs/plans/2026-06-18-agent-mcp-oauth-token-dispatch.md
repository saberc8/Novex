# Agent MCP OAuth Token Dispatch

Date: 2026-06-18

## Goal

Add the backend-owned MCP OAuth token HTTP dispatch adapter that consumes the `novex-mcp` token exchange/session contract, resolves secret refs, calls a token endpoint, and returns sanitized session evidence without persisting tokens.

## Architecture

`crates/novex-mcp` remains a pure protocol/contract crate. The backend owns network I/O, env-secret resolution, status handling, response parsing, and evidence assembly in a focused `mcp_oauth_token_dispatch` module under `backend/src/application/ai`. This mirrors the existing MCP Streamable HTTP live dispatch style, but keeps OAuth admin/callback concerns out of `agent_tool_executor`.

## Scope

- Add a backend-local token dispatch module.
- Resolve `env:` secret refs for PKCE code verifier and optional client secret.
- Send token requests as `application/x-www-form-urlencoded`.
- Parse standard OAuth JSON token responses into `McpOAuthTokenResponse`.
- Convert token responses into `McpOAuthSessionMaterial` using secret-backed access/refresh token refs.
- Return sanitized evidence containing request plan, response metadata, and session material only.
- Add fake-dispatch and local-server tests.

## Out of Scope

- Public HTTP callback route.
- State storage/lookup.
- PostgreSQL MCP OAuth session persistence.
- Refresh-token exchange execution.
- Admin UI flows.
- Third-party deployed MCP smoke tests.

## TDD Plan

1. Add RED tests for env-secret resolution and sanitized session evidence.
2. Add RED tests for missing PKCE/client secret failure without dispatching.
3. Add RED tests for token endpoint HTTP failures without token/secret leakage.
4. Add RED local-server smoke verifying method, headers, form body, and response parsing through real `reqwest`.
5. Implement the smallest focused backend module needed to pass.
6. Update the migration matrix and enterprise foundation notes from `slice-7` to `slice-8`.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p backend mcp_oauth_token_dispatch --offline`
- `cargo test -p backend mcp --offline`
- `cargo test -p novex-mcp mcp_oauth_session --offline`
- `cargo test --workspace --offline`

## Follow-up

- MCP OAuth callback API with state validation and tenant/user binding.
- PostgreSQL session persistence and encrypted token storage/secret binding.
- Refresh-token dispatch path and scheduler hook.
- Admin UI for connect/reconnect/revoke.
- Deployed external MCP OAuth server smoke coverage.
