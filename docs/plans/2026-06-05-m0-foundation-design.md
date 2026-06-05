# M0 Foundation Skeleton Design

## Goal

Build the first Novex foundation baseline from `docs/ARCHITECTURE.md`: a compileable Rust workspace, explicit AI crate boundaries, sidecar/app/template directory skeletons, and visible AI control-plane entry points in the existing admin system.

M0 is a foundation slice. It does not implement real RAG, Agent loops, model calls, Milvus, parser jobs, connectors, plugins, triggers, or eval execution.

## Architecture

Novex remains a monorepo. The existing `backend` continues to own HTTP API, RBAC, auth, audit, scheduler, files, and control-plane orchestration. Reusable AI foundation logic lives under `crates/*`, and those crates must not depend on `backend`.

The workspace root becomes `Novex/Cargo.toml`, with `backend` and AI crates as members. `backend` may depend on the AI crates for stable types and capability summaries, but domain logic for Run Graph, models, RAG, tools, connectors, plugins, triggers, memory, and eval remains in the crates.

Directory boundaries:

- `backend/`: existing Axum API and RBAC control plane.
- `crates/novex-ai-core/`: shared tenant/resource/run/trace/policy types.
- `crates/novex-model/`: model provider/profile/route types and adapter traits.
- `crates/novex-rag/`: knowledge, chunk, retrieval, rerank, citation boundaries.
- `crates/novex-agent/`: intent, planner, ReAct/runtime boundaries.
- `crates/novex-tools/`: tool schema, risk, approval, executor boundaries.
- `crates/novex-connectors/`: connector schema, credential scope, datasource boundaries.
- `crates/novex-mcp/`: MCP server/tool discovery and gateway boundaries.
- `crates/novex-plugin/`: plugin manifest, installation, capability boundaries.
- `crates/novex-trigger/`: webhook/schedule/event routing boundaries.
- `crates/novex-memory/`: memory scope, policy, store boundaries.
- `crates/novex-eval/`: eval dataset/case/run/report boundaries.
- `services/parser-worker/`: Python parser and ML sidecar skeleton.
- `services/model-runtime/`: optional model adapter/runtime sidecar skeleton.
- `apps/*`: customer-facing Next.js app skeleton placeholders.
- `templates/`: deliverable template skeletons.
- `infra/`: local deployment and environment skeletons.

## Admin Control Plane

The existing menu and permission mechanism is the source of truth. M0 adds an AI catalog and placeholder pages through seed migration rather than hard-coded frontend navigation.

Top-level admin routes:

- `/ai/dashboard`
- `/ai/models`
- `/ai/knowledge`
- `/ai/agents`
- `/ai/tools`
- `/ai/connectors`
- `/ai/plugins`
- `/ai/triggers`
- `/ai/evals`
- `/ai/traces`
- `/ai/templates`

System identity provider routes belong under system security rather than the AI menu:

- `/system/identity/providers`
- `/system/identity/accounts`
- `/system/identity/policies`

Each route gets a minimal page that states the module boundary and current M0 status. Later milestones will replace these pages with real CRUD and workflow screens.

## Backend API

M0 adds a lightweight AI foundation endpoint:

- `GET /ai/foundation/summary`

The endpoint is permission protected with `ai:foundation:read`. It returns module IDs, names, status, and boundary descriptions from stable crate metadata. It must not perform model calls or touch future AI business tables.

## Data And Permissions

M0 adds a migration that seeds:

- AI menu catalog and child menus.
- AI read placeholder permissions.
- System identity placeholder menus and permissions.
- Admin role grants for all seeded AI/system identity menu items.

The migration must be idempotent using `ON CONFLICT DO NOTHING`, matching existing seed conventions.

M0 does not introduce the full data model from the architecture document. Tenant, ACL, model, run, tool, connector, plugin, trigger, media, memory, and eval tables are deferred to later implementation slices so that table contracts are designed with their first real API surfaces.

## Error Handling

The backend endpoint uses the existing `AppError` and `ApiResponse` envelope. Missing permissions return the same forbidden envelope as the rest of the system. Workspace crates expose typed data only; they do not depend on backend errors.

Frontend placeholder pages avoid network writes. If the summary API is unavailable, the admin UI still renders static module boundaries.

## Testing

Verification for M0:

- `cargo test --workspace` from `Novex/`.
- `cargo test` from `Novex/backend/` if backend standalone workflow remains supported.
- `pnpm typecheck` from `Novex/admin/`.
- `pnpm test` from `Novex/admin/`.

Focused tests:

- Rust crate metadata lists all required M0 modules.
- Backend router exposes `/ai/foundation/summary` and enforces `ai:foundation:read`.
- Frontend route/menu utilities continue to accept new AI routes.

## Non-Goals

- No real model provider calls.
- No parser worker execution.
- No Milvus integration.
- No RAG answer API.
- No Agent Runtime loop.
- No connector or tool side effects.
- No plugin installation flow.
- No trigger webhook receiver.
- No eval runner.
