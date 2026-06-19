# Agent MCP OAuth Session Contract

Date: 2026-06-18

## Goal

Extend `crates/novex-mcp` beyond the existing authorization-code/PKCE browser handoff contract with the next OAuth boundary: token-exchange request planning, sanitized token/session material, and refresh-expiry decisions.

This keeps the Codex-style MCP foundation moving while preserving Novex's tenant/config/secret control plane. The crate should describe what to send and how to store session material safely; backend integration will later own real HTTP dispatch, callback routing, tenant persistence, and UI/API flows.

## Scope

- Add an authorization-code token exchange request plan for the OAuth token endpoint.
- Require secret refs for PKCE code verifier and client secret values.
- Add sanitized evidence for token exchange requests that never exposes authorization codes, token values, or secret values.
- Parse a token response DTO into secret-backed OAuth session material.
- Track session scopes, token type, optional refresh token secret ref, and optional absolute expiry.
- Add refresh-needed helper logic with skew.
- Keep everything deterministic and offline-testable inside `novex-mcp`.

## Out of Scope

- Real HTTP calls to token endpoints.
- OAuth callback handlers.
- Database persistence for MCP OAuth sessions.
- Refresh-token exchange dispatch.
- Browser UI or tenant admin APIs.
- Deployed third-party MCP server smoke tests.

## TDD Plan

1. Add RED tests for token exchange plan shape, headers, form fields, and sanitized evidence.
2. Add RED tests requiring `env:` secret refs for PKCE code verifier and client secret values.
3. Add RED tests parsing token responses into secret-backed session material.
4. Add RED tests rejecting unsupported token types.
5. Add RED tests for refresh-needed skew behavior.
6. Implement only the smallest `novex-mcp` contract needed to pass.
7. Update the living migration matrix and enterprise plan notes.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p novex-mcp mcp_oauth_session --offline`
- `cargo test -p novex-mcp --offline`
- `cargo test -p backend mcp --offline`
- `cargo test --workspace --offline`

## Follow-up

- Backend OAuth callback API that validates state and binds server/tenant/user context.
- Provider-client-backed HTTP token exchange adapter with tenant-bound provider-call/session evidence.
- PostgreSQL MCP OAuth session table and refresh scheduling.
- Refresh-token exchange request/response execution path.
- Deployed external MCP server smoke coverage.
