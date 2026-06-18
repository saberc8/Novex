# Agent MCP OAuth Callback Route

Date: 2026-06-18

## Goal

Close the MCP OAuth authorization-code loop by wiring callback state consumption, token endpoint dispatch, tenant-scoped secret writes, and persistent session upsert behind an authenticated backend capability route.

## Architecture

`novex-mcp` keeps the pure OAuth protocol/session contract. The backend callback path owns tenant/user binding, one-shot state consumption, token HTTP dispatch, secret-manager writes, and `ai_mcp_oauth_session` persistence. Token values must only exist inside the token dispatch completion boundary long enough to write `sys_secret` versions, while public evidence and session records contain only `secretRef` values.

## Scope

- Add a secret-writer hook to the MCP OAuth token dispatch path.
- Keep access/refresh token values out of response structs, debug evidence, database session rows, and route responses.
- Add tenant-aware `SecretService` APIs for internal token secret writes.
- Add a callback completion command/response to `CapabilityService`.
- Consume `ai_mcp_oauth_state` by `state_hash`, `server_id`, `tenant_id`, and `redirect_uri`.
- Exchange the authorization code through the existing token dispatch adapter.
- Write access and optional refresh tokens into `sys_secret` as new versions.
- Upsert `ai_mcp_oauth_session` with secret refs, scopes, expiry, and sanitized metadata.
- Add `POST /ai/capabilities/mcp/servers/:server_id/oauth/callback` guarded by `ai:mcp:update`.
- Update the Codex migration matrix from `slice-10` to `slice-11`.

## Out of Scope

- Unauthenticated browser redirect callback route.
- Refresh-token execution and scheduler refresh.
- Deployed third-party MCP OAuth smoke test.
- Admin UI connect/reconnect/revoke controls.
- Replacing the existing local XOR secret sealing implementation.

## TDD Plan

1. Add RED tests proving token dispatch writes token values to a secret-writer hook without leaking them in sanitized evidence.
2. Add RED tests proving secret-writer failures are sanitized and stop session completion.
3. Add RED service tests proving callback completion consumes hashed state, dispatches token exchange, writes tenant-scoped secrets, and produces an upsertable session record.
4. Add RED HTTP source/handler tests proving the callback route is present, tenant-aware, and permission-guarded.
5. Implement the smallest code path to pass the tests.
6. Update migration matrix acceptance evidence.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p backend-rust mcp_oauth_token_dispatch --offline`
- `cargo test -p backend-rust mcp_oauth_callback_route --offline`
- `cargo test -p backend-rust mcp_oauth_persistence --offline`
- `cargo test -p backend-rust mcp --offline`
- `cargo test --workspace --offline`

## Follow-up

- Public browser callback route with signed one-time handoff and CSRF-safe UX.
- OAuth refresh-token grant execution path.
- Admin UI OAuth connect/reconnect/revoke controls.
- External MCP OAuth provider smoke coverage.
