# Agent SecretRef Resolver

Date: 2026-06-18

## Goal

Add a tenant-aware backend secretRef resolver so agent infrastructure can read `env:` and `sys:` secrets through one audited boundary without exposing plaintext in API responses or evidence.

## Architecture

`sys_secret` remains the PostgreSQL-backed secret version store. `SecretService` owns sealing, unsealing, tenant checks, and ref parsing. Application services such as MCP OAuth refresh, MCP bearer auth, connector execution, and provider routes should call a resolver boundary instead of reading env vars or database ciphertext directly.

## Scope

- Add a repository query for the latest active `sys_secret` version including ciphertext.
- Add an unseal helper paired with the existing local seal helper.
- Add `SecretService::resolve_secret_ref` for `env:` and `sys:` refs.
- Keep resolved plaintext out of serializable public response structs.
- Enforce tenant-scope mismatch rejection for `sys:tenant:<tenantId>:<code>`.
- Add tests for ref parsing, seal/unseal roundtrip, env resolution, unsupported refs, and repository SQL contract.
- Update the migration matrix to record the shared secret resolver as a prerequisite for MCP OAuth refresh.

## Out of Scope

- Replacing the local XOR sealer with production KMS.
- Adding public HTTP endpoints that return plaintext.
- MCP OAuth refresh-token dispatch itself.
- Rotating all existing secret refs to `sys:`.
- Reading historical inactive secret versions.

## TDD Plan

1. Add RED tests for `seal_secret_value` / `unseal_secret_value` roundtrip without exposing ciphertext in public responses.
2. Add RED tests for `resolve_secret_ref` env lookup and unsupported prefix failure.
3. Add RED tests for `sys:` parsing and tenant mismatch rejection.
4. Add RED repository SQL contract requiring latest active ciphertext lookup.
5. Implement repository/service helpers.
6. Update the Codex migration matrix acceptance notes.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p backend secret --offline`
- `cargo test -p backend mcp_oauth_callback --offline`
- `cargo test --workspace --offline`

## Follow-up

- MCP OAuth refresh-token execution using `SecretService::resolve_secret_ref`.
- MCP live HTTP dispatch support for `sys:` bearer refs.
- Connector execution support for `sys:` credential refs.
- Production KMS-backed secret sealing provider.
