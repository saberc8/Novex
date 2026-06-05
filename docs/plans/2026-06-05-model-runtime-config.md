# Model Runtime Config Implementation Plan

Date: 2026-06-05

## Goal

Map deployment model environment variables into Novex runtime model routes, expose sanitized backend/admin diagnostics, and verify the whole system with real provider health checks.

## Tasks

1. Extend `novex-model`
   - Add runtime endpoint structs, route summaries, env parsing, URL joining, missing-env reporting, and key masking.
   - Add tests for user env mapping, DashScope `/reranks`, missing env reporting, and secret masking.

2. Add backend model runtime API
   - Add `model_service` with summary and sanitized health-check logic.
   - Add HTTP routes under `/ai/models`.
   - Add permission seed for `ai:model:healthCheck`.
   - Add tests for permission enforcement, route registration, permission seed, and summary masking.

3. Add admin model operations page
   - Add model runtime types and API wrappers.
   - Replace placeholder page with route summary and health-check UI.
   - Add API wrapper tests.

4. Verify
   - Rust: `cargo fmt -- --check`, `cargo test --workspace --offline`.
   - Admin: `pnpm typecheck`, `pnpm lint`, `pnpm vitest run`, `pnpm build`.
   - Runtime smoke with supplied env values:
     - `/health`
     - `/ready`
     - `/ai/models/runtime-config`
     - `/ai/models/health-check`
     - `/ai/models` admin page

## Acceptance Criteria

- No raw API key is committed, logged by tests, or returned by the API.
- Runtime config reports four complete routes when the supplied env vars are present.
- Health checks pass for LLM, embedding, reranker, and draw reachability.
- Admin page can display route summaries and sanitized health results.
- Full Rust and admin verification commands pass.
