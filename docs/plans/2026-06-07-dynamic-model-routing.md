# Dynamic Model Routing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make live model calls resolve from Novex model route data first, so tenant model registry data can flow into Chat, RAG, and live Eval execution.

**Architecture:** Add executable database-backed route resolution in `ModelRuntimeService`, with environment routes as fallback. Then route Chat and RAG model calls through the service instance bound to the request tenant, preserving existing local fallbacks where non-strict mode allows them.

**Tech Stack:** Rust, SQLx, `novex-model`, backend application services, gated live tests using real LLM/embedding/rerank endpoints from `backend/.env`.

---

### Task 1: Make Runtime Routes Carry Dynamic Route IDs

**Files:**
- Modify: `crates/novex-model/src/lib.rs`

**Step 1: Write failing tests**

Add tests for:

- constructing a route with a custom route id, such as `tenant42.rag_answer`;
- preserving existing env route ids, such as `runtime.llm`;
- parsing route purpose strings needed by database rows.

Run:

```bash
cargo test -p novex-model dynamic_route -- --nocapture
```

Expected: FAIL because `ModelRuntimeRoute` cannot currently be constructed with custom route ids and `ModelRoutePurpose` has no parser.

**Step 2: Implement minimal API**

Add:

- a `route_id: String` field to `ModelRuntimeRoute`;
- a public constructor that validates non-empty route id, endpoint, and api key;
- `ModelRoutePurpose::as_str` and `ModelRoutePurpose::parse`;
- `ModelKind::parse` and `ModelProviderType::parse` if needed by backend resolver.

Keep env-generated routes returning existing ids.

**Step 3: Run tests**

```bash
cargo test -p novex-model dynamic_route -- --nocapture
cargo test -p novex-model
```

Expected: PASS.

**Step 4: Commit**

```bash
git add crates/novex-model/src/lib.rs
git commit -m "feat: allow dynamic model runtime routes"
```

### Task 2: Resolve Executable Routes From Model Registry Tables

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add pure resolver tests for:

- joined registry rows build an executable route with `route_id = row.code`;
- `env:NAME` credential refs read from an injected env map and mask correctly;
- missing credential skips the DB route and allows environment fallback;
- route purpose maps to the expected runtime target.

Run:

```bash
cargo test -p backend application::ai::model_service::tests::dynamic_route -- --nocapture
```

Expected: FAIL because database route conversion and purpose resolver do not exist.

**Step 2: Implement row conversion helpers**

Add internal types and helpers:

- `ModelRuntimeRouteRow`;
- `ModelRuntimeRouteSelection`;
- `route_target_for_purpose`;
- `endpoint_from_deployment`;
- `resolve_credential_ref`;
- `runtime_route_from_registry_row`.

Only support `env:NAME` credentials in this iteration.

**Step 3: Implement DB resolver**

Add methods on `ModelRuntimeService`:

- `async fn resolve_route_for_purpose(&self, purpose: ModelRoutePurpose) -> Result<Option<ModelRuntimeRouteSelection>, AppError>`;
- `async fn effective_route_for_purpose(&self, purpose: ModelRoutePurpose) -> Result<Option<ModelRuntimeRoute>, AppError>`;
- `async fn effective_runtime_summary(&self) -> Result<ModelRuntimeSummary, AppError>`.

The resolver must query the current tenant and active rows, ordered by `priority ASC, id ASC`. If no usable DB route exists, use `ModelRuntimeConfig::from_env()` for the matching target.

**Step 4: Run tests**

```bash
cargo test -p backend application::ai::model_service::tests::dynamic_route -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: resolve model routes from registry"
```

### Task 3: Wire Dynamic Routes Into Chat, Runtime Config, And Health

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `backend/src/interfaces/http/ai/model.rs`

**Step 1: Write failing tests**

Add tests for:

- `ModelRuntimeService::chat_completion_for_source` uses purpose `chat` resolution;
- chat usage records look up the selected route by response route code;
- runtime config handler binds current tenant and calls tenant-scoped summary;
- health check handler binds current tenant before network.

Run:

```bash
cargo test -p backend application::ai::model_service::tests::model_chat_ application::ai::model_service::tests::dynamic_route interfaces::http::ai::model -- --nocapture
```

Expected: FAIL until chat and HTTP handlers use the tenant-bound service path.

**Step 2: Implement chat route selection**

Change instance chat methods to:

- normalize command;
- resolve `ModelRoutePurpose::Chat`;
- execute chat with the selected route;
- persist route id, conversation history, and usage using `response.route_id`.

Keep the static `ModelRuntimeService::chat_completion` as environment fallback for tests and legacy callers, but do not use it from tenant-bound Chat/RAG paths.

**Step 3: Implement runtime config and health**

Change HTTP handlers:

- `/ai/models/runtime-config` uses `ModelRuntimeService::for_tenant(...).effective_runtime_summary()`;
- `/ai/models/health-check` uses tenant-bound route resolution before network calls.

**Step 4: Run tests**

```bash
cargo test -p backend application::ai::model_service -- --nocapture
cargo test -p backend interfaces::http::ai::model -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/model_service.rs backend/src/interfaces/http/ai/model.rs
git commit -m "feat: route chat through tenant model registry"
```

### Task 4: Wire Dynamic Routes Into RAG And Live Eval

**Files:**
- Modify: `backend/src/application/ai/knowledge_service.rs`
- Test: `backend/src/application/ai/knowledge_service.rs`

**Step 1: Write failing tests**

Add tests for:

- chunk embedding enrichment can use an explicit route id from a dynamic route;
- RAG answer generation accepts a selected dynamic LLM route;
- trace records persist dynamic embedding/rerank/answer route ids.

Run:

```bash
cargo test -p backend application::ai::knowledge_service::tests::dynamic_route -- --nocapture
```

Expected: FAIL until RAG helpers accept tenant-bound route resolution.

**Step 2: Implement embedding route injection**

Change ingestion to resolve embedding with `ModelRuntimeService::for_tenant(self.db.clone(), tenant_id)` and `ModelRoutePurpose::Embedding`.

Keep local embedding fallback in non-strict mode.

**Step 3: Implement query embedding, rerank, and answer route injection**

Change ask path helpers to receive tenant-bound model service or resolved routes:

- query embedding uses purpose `embedding`;
- rerank uses purpose `rerank`;
- answer uses purpose `rag_answer`;
- strict live RAG mode fails if required routes cannot resolve or call.

**Step 4: Run tests**

```bash
cargo test -p backend application::ai::knowledge_service::tests::dynamic_route -- --nocapture
cargo test -p backend application::ai::knowledge_service -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/knowledge_service.rs
git commit -m "feat: route rag through tenant model registry"
```

### Task 5: Add Live Dynamic Routing Smoke Tests

**Files:**
- Modify: `backend/tests/live_rag_e2e.rs`
- Create: `backend/tests/live_model_routing.rs`

**Step 1: Write failing live assertions**

Extend `live_rag_e2e` to seed database route rows with distinct route codes:

- `live.dynamic.embedding`;
- `live.dynamic.rerank`;
- `live.dynamic.rag_answer`.

Assert `ai_rag_trace` stores those database route codes, not environment fallback ids.

Create `live_model_routing.rs` to:

- create a temporary DB;
- run migrations;
- update/seed chat route rows from `backend/.env`;
- call `ModelRuntimeService::for_tenant(...).chat_completion_with_usage`;
- assert the response route id is the DB route code.

Run:

```bash
set -a
. /path/to/Novex/backend/.env
set +a
NOVEX_LIVE_MODEL_ROUTING_TEST=1 cargo test -p backend --test live_model_routing -- --ignored --nocapture
NOVEX_LIVE_RAG_TEST=1 cargo test -p backend --test live_rag_e2e -- --ignored --nocapture
```

Expected: FAIL until dynamic DB route resolution is wired through.

**Step 2: Implement fixtures**

Add fixture helpers that update seeded model registry rows in the temporary test database:

- deployment endpoint from `LLM_BASE_URL`, `EMBEDDING_BASE_URL`, `RERANKER_BASE_URL`;
- profile model names from `LLM_MODEL`, `EMBEDDING_MODEL`, `RERANKER_MODEL`;
- credential refs remain `env:...`;
- route codes are distinct live dynamic codes.

**Step 3: Run live tests**

Expected: PASS with real model calls and dynamic route ids persisted.

**Step 4: Commit**

```bash
git add backend/tests/live_rag_e2e.rs backend/tests/live_model_routing.rs
git commit -m "test: prove live dynamic model routing"
```

### Task 6: Final Verification

**Files:**
- No planned edits.

**Step 1: Formatting and diff checks**

```bash
cargo fmt --check
git diff --check main..HEAD
```

Expected: PASS.

**Step 2: Offline regression**

```bash
cargo test --workspace --exclude backend
cargo test -p backend application::ai
cargo test -p backend interfaces::http::ai
PYTHONPATH=services/parser-worker python3 -m unittest discover -s services/parser-worker/tests
```

Expected: PASS.

**Step 3: Live model/RAG verification**

```bash
set -a
. /path/to/Novex/backend/.env
set +a
NOVEX_LIVE_MODEL_ROUTING_TEST=1 cargo test -p backend --test live_model_routing -- --ignored --nocapture
NOVEX_LIVE_RAG_TEST=1 cargo test -p backend --test live_rag_e2e -- --ignored --nocapture
NOVEX_LIVE_MINERU_TEST=1 PYTHONPATH=services/parser-worker python3 -m unittest services/parser-worker/tests/test_mineru_live.py
```

Expected: PASS.

**Step 4: Report**

Report:

- commits created;
- verification commands and pass/fail counts;
- live proof that DB route codes reached model calls/traces;
- remaining gaps outside this scope.
