# Agent MCP OAuth Authorization Contract Plan

## Goal

Move the MCP gateway one step closer to enterprise OAuth-backed MCP servers by adding a transport-neutral OAuth authorization plan contract in `novex-mcp`.

## Architecture

Keep `novex-mcp` as the pure protocol/policy crate. It should validate OAuth authorization metadata, build a safe authorization URL with PKCE, record token endpoint/client authentication intent, and expose sanitized evidence without resolving or leaking client secrets. Backend, UI, token exchange, persisted sessions, and provider-specific OAuth callbacks remain follow-up slices.

This is an adapter-port slice from Codex MCP infrastructure and MCP authorization conventions: authorization-code flow, PKCE S256, state binding, scoped access, client secret references, and audit-safe request planning.

## Scope

- Add `McpOAuthPkceMethod`, `McpOAuthClientAuth`, `McpOAuthAuthorizationConfig`, and `McpOAuthAuthorizationPlan` to `crates/novex-mcp`.
- Validate server code, HTTPS authorization/token endpoints, client id, redirect URI, state, scopes, PKCE challenge, and `env:` client secret references.
- Build a deterministic authorization URL with `response_type=code`, `client_id`, `redirect_uri`, `scope`, `state`, `code_challenge`, and `code_challenge_method`.
- Produce sanitized evidence that includes endpoints, client id, redirect URI, scopes, state, PKCE method, and client auth kind without exposing any client secret value.
- Update the migration matrix and enterprise foundation plan so MCP shows OAuth authorization contract progress while token exchange, persisted MCP sessions, and browser callback integration remain future slices.

## Out of Scope

- Browser redirect handlers or UI.
- OAuth token exchange HTTP dispatch.
- Refresh-token storage or persisted MCP sessions.
- Dynamic client registration.
- OAuth discovery document fetching.
- Changing existing backend MCP live dispatch behavior.

## RED Tests

- `mcp_oauth_authorization_plan_builds_pkce_authorize_url`: valid config builds an authorization URL with code flow, scopes, state, redirect URI, and S256 PKCE.
- `mcp_oauth_authorization_plan_sanitizes_client_secret_ref`: sanitized evidence exposes only `clientSecretRef` and never a resolved client secret value.
- `mcp_oauth_authorization_plan_rejects_non_https_endpoint`: non-HTTPS authorization or token endpoints fail closed.
- `mcp_oauth_authorization_plan_rejects_invalid_client_secret_ref`: client secret refs without the `env:` prefix fail closed.
- `mcp_oauth_authorization_plan_requires_scope_and_state`: empty scopes or state are rejected before backend can start an authorization flow.

## Implementation Steps

1. Add RED tests in `crates/novex-mcp/tests/oauth.rs` for PKCE URL construction, sanitized evidence, HTTPS enforcement, secret-ref validation, and required state/scopes.
2. Run `cargo test -p novex-mcp mcp_oauth --offline` and confirm the tests fail because the OAuth plan contract does not exist.
3. Add OAuth plan, client auth, PKCE, and error types in `crates/novex-mcp/src/oauth.rs`.
4. Implement validation helpers for HTTPS URLs, non-empty fields, scope normalization, S256 PKCE challenge, redirect URI parsing, and `env:` client secret refs.
5. Implement `McpOAuthAuthorizationPlan::new`, `authorization_url`, and `sanitized_evidence`.
6. Run focused `novex-mcp` tests, then backend MCP tests to confirm existing MCP execution remains unchanged.
7. Update `docs/plans/2026-06-16-codex-migration-matrix.md` and `docs/plans/2026-06-16-enterprise-agent-foundation.md`.
8. Run full verification, commit implementation, fast-forward merge to `main`, run `cargo clean`, and remove this worktree/branch.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p novex-mcp mcp_oauth --offline`
- `cargo test -p novex-mcp --offline`
- `cargo test -p backend-rust mcp --offline`
- `cargo test --workspace --offline`
