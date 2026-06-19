# Agent MCP OAuth Persistence Contract

Date: 2026-06-18

## Goal

Add the backend persistence contract for MCP OAuth callback state and session material so a later public callback route can consume a hashed state exactly once and store secretRef-backed session metadata without persisting token values.

## Architecture

`ai_mcp_server` remains the registration/config source for MCP servers. This slice adds two focused persistence tables: `ai_mcp_oauth_state` for short-lived authorization handoff state and `ai_mcp_oauth_session` for active tenant/user/app scoped OAuth session metadata. The repository owns SQL boundaries while `novex-mcp` continues to own protocol/session validation and the backend token dispatch module continues to own token HTTP exchange.

## Scope

- Add a migration for `ai_mcp_oauth_state`.
- Add a migration for `ai_mcp_oauth_session`.
- Store only `state_hash`, never raw callback state.
- Store only `access_token_secret_ref` and `refresh_token_secret_ref`, never access/refresh token values.
- Add repository records for saving/consuming MCP OAuth states.
- Add repository records for upserting/listing MCP OAuth sessions by tenant/server/scope.
- Ensure state consumption is one-shot using `consumed_at IS NULL` and `expires_at > NOW()`.
- Add tests for migration contract and repository SQL safety invariants.
- Update the migration matrix from `slice-9` to `slice-10`.

## Out of Scope

- Public HTTP callback route.
- Live Postgres integration test.
- Secret manager write path for token values.
- Encrypting token values in the database.
- Refresh-token dispatch execution.
- Admin UI connect/reconnect/revoke controls.

## TDD Plan

1. Add RED migration test requiring `ai_mcp_oauth_state` and `ai_mcp_oauth_session` with hash/secretRef fields and no raw token/code columns.
2. Add RED repository SQL test requiring one-shot state consumption and expiry checks.
3. Add RED repository SQL test requiring session upsert on `(tenant_id, server_id, scope_type, scope_id)`.
4. Add migration and repository structs/methods to satisfy tests.
5. Update the migration matrix and MCP acceptance evidence.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p backend mcp_oauth_persistence --offline`
- `cargo test -p backend mcp --offline`
- `cargo test -p backend mcp_oauth_callback --offline`
- `cargo test -p backend mcp_oauth_token_dispatch --offline`
- `cargo test --workspace --offline`

## Follow-up

- Public MCP OAuth callback route with tenant/user binding.
- Callback service that consumes `state_hash`, dispatches token exchange, writes secret manager values, and upserts `ai_mcp_oauth_session`.
- Refresh-token execution path and scheduler hook.
- Admin UI for connect/reconnect/revoke.
- Deployed external MCP OAuth server smoke coverage.
