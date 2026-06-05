# M1-M5 Roadmap Design

## Goal

Continue Novex from the current M1 metadata foundation through M5 in architecture-aligned, verifiable slices. Each milestone must leave the system runnable, permission-protected, and testable.

## Current State

M0 is complete. M1 currently has:

- `novex-rag` knowledge metadata enums.
- `ai_dataset` and `ai_document` metadata tables.
- Knowledge dataset/document list and dataset create APIs.
- Admin `/ai/knowledge` dataset/document console.

It does not yet have document upload into a dataset, parsing, chunk storage, embedding, retrieval, rerank, ask API, citations, or trace.

## Approach Options

### Option A: Direct Full External Stack

Implement M1 using real parser worker, external embedding model, Milvus, rerank, and answer LLM immediately.

Trade-off: most realistic, but it blocks development on external services, credentials, network policy, and deployment-specific model choices.

### Option B: Deterministic Local MVP With Adapter Boundaries

Implement M1 as a complete local RAG loop first: upload text/markdown/plain files, parse deterministically, chunk in Rust, store chunks in PostgreSQL, run keyword/vector-like local retrieval, generate citation-bearing extractive answers, and record trace. Keep explicit adapter boundaries for model route, embedding provider, rerank provider, and Milvus.

Trade-off: not production-quality semantic retrieval yet, but it creates the correct contracts and makes the product usable and testable without external dependencies.

### Option C: Control-Plane Only Until M5

Build all registries and template metadata first, defer runtime behavior.

Trade-off: fast breadth, weak product value; later milestones would be untested placeholders.

## Selected Approach

Use Option B for M1, then build M2-M5 on the same runtime contracts. This keeps Novex moving as a working product while preserving the architecture boundary that `docs/ARCHITECTURE.md` requires.

## Milestone Scope

### M1: Knowledge RAG MVP

Deliver a minimum closed loop:

- Knowledge document upload through backend and admin UI.
- Parser job metadata.
- Text/Markdown parser in-process and `services/parser-worker` contract scaffold.
- Chunk generation and chunk persistence.
- Local deterministic embedding/retrieval fallback.
- Adapter traits/config slots for embedding, rerank, answer model, and Milvus.
- Ask API returning an answer, cited chunks, and trace ID.
- RAG trace records retrieval hits, rerank scores, answer route, and token-ish metrics.

M1 does not require real external model credentials. External providers can be plugged in later through M2 model/tool/connector governance.

### M2: Skills / Tools / Connectors / Plugins / MCP

Deliver registries and POCs:

- `ai_skill`, `ai_tool`, `ai_connector`, `ai_plugin`, `ai_trigger`, and `ai_mcp_server` metadata.
- Tool call audit table and API.
- GitHub connector/provider POC metadata.
- Media/image tool POC metadata.
- Webhook trigger verification contract.
- Feishu message tool POC metadata.

M2 should not implement unsafe tool execution beyond auditable dry-run POCs.

### M3: Agent Runtime

Deliver a Run Graph state machine:

- Run, step, event tables.
- Start/pause/resume/cancel APIs.
- Approval and human input pause reasons.
- Event snapshot API.
- Budget model.
- A deterministic ReAct-like loop scaffold that can call registered dry-run tools.

M3 should use M2 tool registry and M1 context builder instead of hard-coded capability lists.

### M4: Eval

Deliver quality measurement:

- Eval dataset and case tables.
- Eval run and result tables.
- RAG citation/hit metrics.
- Intent/tool metrics over run events.
- Regression report API and admin UI.

M4 should run against deterministic M1/M3 outputs first. LLM judge can be an adapter.

### M5: Customer Delivery Templates

Deliver customer initialization:

- Template registry for LLM Chat, Knowledge Base Chat, Agent Workspace, and Training App.
- Branding config.
- Default roles, menus, skills, connectors, plugins, triggers, and eval sets.
- Customer initialization wizard and deployment handoff docs.

M5 should generate configuration and seed data, not fork customer-specific code.

## Data and Runtime Boundaries

- PostgreSQL owns control-plane metadata, trace metadata, and local deterministic fallback indexes.
- Milvus remains the default production vector target, but M1 may run without it through an adapter.
- `backend` owns HTTP, RBAC, audit, storage, and orchestration.
- `crates/novex-rag` owns pure RAG domain logic: parse result, chunker, local scorer, citations, and trace model.
- `crates/novex-model` owns model route vocabulary and route resolution contracts.
- `services/parser-worker` documents and later hosts the out-of-process parser contract.

## Security Rules

- Every user-facing endpoint requires `CurrentUser` and permission checks.
- Every dataset/document/chunk query includes tenant filtering.
- Every RAG answer returns citations.
- Every retrieval/answer call records trace metadata.
- High-risk tools in M2/M3 default to dry-run or approval-required.

## Verification Strategy

- Rust domain logic uses unit tests first.
- Backend service and route handlers use focused unit/integration-style tests with lazy pools where possible.
- Migrations are checked by table/index/permission grep and backend compile tests.
- Admin API wrappers and pages use Vitest.
- Each slice runs `cargo test --workspace --offline`, `pnpm typecheck`, `pnpm test`, `pnpm lint`, and `pnpm build` before merge.
