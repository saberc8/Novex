# Agent MCP OAuth Callback State

Date: 2026-06-18

## Goal

Add the backend-owned MCP OAuth callback/state contract that validates provider callback parameters, rejects callback errors before any token request, and orchestrates the existing token dispatch adapter with sanitized evidence.

## Architecture

`crates/novex-mcp` continues to own pure OAuth token-exchange/session request contracts. The backend owns callback concerns because callback state is tied to tenant/user/session boundaries, runtime secrets, and HTTP entrypoints. This slice introduces a service-level contract under `backend/src/application/ai` that can later be called by a public callback route after loading the expected state and server OAuth config from persistence.

## Scope

- Add a backend callback token command for MCP OAuth authorization-code callbacks.
- Validate non-empty callback state and exact expected-state match before dispatch.
- Reject provider callback `error` values before dispatching to the token endpoint.
- Reject missing authorization code before dispatch.
- Build a `McpOAuthTokenExchangePlan` from validated callback input.
- Reuse `exchange_mcp_oauth_token_with_dispatch` for env-secret resolution, HTTP token dispatch, response parsing, and session material creation.
- Return sanitized callback plus token evidence without leaking authorization codes, PKCE verifier, client secret, access tokens, or refresh tokens.
- Add fake-dispatch tests for success and pre-dispatch failure paths.

## Out of Scope

- Public HTTP callback route.
- Persisted OAuth state lookup.
- Tenant/user binding in PostgreSQL.
- Encrypted session persistence.
- Refresh-token execution.
- Admin UI connect/reconnect/revoke flows.

## TDD Plan

1. Add RED test for successful callback state validation, token dispatch orchestration, and sanitized evidence.
2. Add RED test for state mismatch preventing dispatch.
3. Add RED test for provider callback error preventing dispatch.
4. Add RED test for missing authorization code preventing dispatch.
5. Implement the smallest backend contract needed to pass.
6. Update the migration matrix and enterprise foundation notes from `slice-8` to `slice-9`.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p backend-rust mcp_oauth_callback --offline`
- `cargo test -p backend-rust mcp_oauth_token_dispatch --offline`
- `cargo test -p backend-rust mcp --offline`
- `cargo test -p novex-mcp mcp_oauth_session --offline`
- `cargo test --workspace --offline`

## Follow-up

- Public MCP OAuth callback route with tenant/user auth context.
- Persisted OAuth state/session storage and encrypted token binding.
- Refresh-token dispatch path and scheduler hook.
- Admin UI for connect/reconnect/revoke.
- Deployed external MCP OAuth server smoke coverage.
