# M4 Eval Runtime Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a minimal evaluation runtime that can store eval datasets/cases, run deterministic RAG/intent/tool checks, and produce a regression report.

**Architecture:** Keep metric calculation and deterministic runner contracts in `novex-eval`. Keep HTTP, RBAC, persistence, seeded POC cases, and report orchestration in `backend`. Admin consumes the eval APIs to run the seeded training eval set and inspect metrics; no model judge or external API call is introduced in M4.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL migrations, Next.js Admin, Vitest.

---

### Task 1: Eval Domain Kernel

**Files:**
- Modify: `crates/novex-eval/src/lib.rs`

**Step 1: Write failing tests**

Add tests for:

- `score_rag_case` passes when expected citation appears and answer contains expected text.
- `score_intent_case` passes only when actual intent equals expected intent.
- `score_tool_case` passes when actual tool equals expected tool.
- `build_regression_report` computes pass count, fail count, average score, and metric breakdown.

Run:

```bash
cargo test -p novex-eval eval_runtime --offline
```

Expected: fail because scoring contracts do not exist.

**Step 2: Implement deterministic scoring**

Add:

- `EvalCaseInput`
- `EvalCaseExpected`
- `EvalCaseActual`
- `EvalCaseScore`
- `RegressionReport`
- `score_case`
- `score_rag_case`
- `score_intent_case`
- `score_tool_case`
- `build_regression_report`

**Step 3: Verify and commit**

```bash
cargo test -p novex-eval --offline
cargo test --workspace --offline
git add crates/novex-eval/src/lib.rs
git commit -m "feat: add eval runtime domain kernel"
```

### Task 2: Eval Schema, Permissions, and Seed Cases

**Files:**
- Create: `backend/migrations/202606050010_create_ai_eval_runtime.sql`
- Create: `backend/migrations/202606050011_seed_ai_eval_permissions.sql`

**Step 1: Add schema and seed data**

Create tables:

- `ai_eval_dataset`
- `ai_eval_case`
- `ai_eval_run`
- `ai_eval_result`

Seed:

- Dataset `training_regression`.
- At least 20 POC cases across `rag`, `intent`, and `tool` targets.
- Permissions `ai:eval:run`, `ai:eval:case:list`, `ai:eval:report`.

**Step 2: Verify**

```bash
rg "CREATE TABLE IF NOT EXISTS ai_eval_dataset|CREATE TABLE IF NOT EXISTS ai_eval_case|CREATE TABLE IF NOT EXISTS ai_eval_result" backend/migrations/202606050010_create_ai_eval_runtime.sql
rg "ai:eval:run|ai:eval:case:list|ai:eval:report" backend/migrations/202606050011_seed_ai_eval_permissions.sql
cargo test -p backend-rust --offline
```

**Step 3: Commit**

```bash
git add backend/migrations/202606050010_create_ai_eval_runtime.sql backend/migrations/202606050011_seed_ai_eval_permissions.sql
git commit -m "feat: add ai eval runtime schema"
```

### Task 3: Backend Eval API

**Files:**
- Create: `backend/src/infrastructure/persistence/ai_eval_repository.rs`
- Create: `backend/src/application/ai/eval_service.rs`
- Create: `backend/src/interfaces/http/ai/eval.rs`
- Modify: `backend/src/infrastructure/persistence/mod.rs`
- Modify: `backend/src/application/ai/mod.rs`
- Modify: `backend/src/interfaces/http/ai/mod.rs`

**Step 1: Write failing tests**

Add tests for:

- Eval run command rejects missing dataset code.
- Running seeded cases returns report fields and stores result payloads.
- Report list defaults to latest run first.
- Route registration requires authentication.
- Handlers reject missing `ai:eval:*` permissions.

Run:

```bash
cargo test -p backend-rust eval_runtime --offline
```

Expected: fail because eval modules and routes do not exist.

**Step 2: Implement service and endpoints**

Add endpoints:

- `GET /ai/evals/datasets`
- `GET /ai/evals/datasets/:dataset_id/cases`
- `POST /ai/evals/runs`
- `GET /ai/evals/runs`
- `GET /ai/evals/runs/:run_id`
- `GET /ai/evals/runs/:run_id/results`

Runtime behavior:

- Resolve dataset by code or ID.
- Load enabled cases.
- Generate deterministic actual outputs from expected payloads for POC cases.
- Score with `novex-eval`.
- Persist `ai_eval_run` and `ai_eval_result`.
- Return pass/fail totals, average score, and metric breakdown.

**Step 3: Verify and commit**

```bash
cargo test -p backend-rust eval_runtime --offline
cargo test -p backend-rust --offline
git add backend/src/infrastructure/persistence/ai_eval_repository.rs backend/src/application/ai/eval_service.rs backend/src/interfaces/http/ai/eval.rs backend/src/infrastructure/persistence/mod.rs backend/src/application/ai/mod.rs backend/src/interfaces/http/ai/mod.rs
git commit -m "feat: add eval runtime api"
```

### Task 4: Admin Eval Report Page

**Files:**
- Create: `admin/src/types/ai-eval.ts`
- Create: `admin/src/api/ai/eval.ts`
- Create: `admin/src/api/ai/eval.test.ts`
- Modify: `admin/app/(main)/ai/evals/page.tsx`

**Step 1: Write failing tests**

Test API wrappers for:

- `listEvalDatasets`
- `listEvalCases`
- `runEvalDataset`
- `listEvalRuns`
- `getEvalRun`
- `listEvalResults`

Run:

```bash
pnpm vitest run src/api/ai/eval.test.ts
```

Expected: fail because wrappers do not exist.

**Step 2: Implement Admin page**

Replace placeholder with:

- Dataset selector.
- Run button for `training_regression`.
- Latest report summary with pass/fail/average score.
- Metric breakdown bands for RAG, intent, and tool accuracy.
- Case/result list.

**Step 3: Verify and commit**

```bash
pnpm typecheck
pnpm vitest run src/api/ai/eval.test.ts
pnpm lint
git add admin/src/types/ai-eval.ts admin/src/api/ai/eval.ts admin/src/api/ai/eval.test.ts 'admin/app/(main)/ai/evals/page.tsx'
git commit -m "feat: add eval report admin page"
```

### Task 5: M4 Verification and Smoke

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd admin && pnpm typecheck && pnpm vitest run && pnpm lint && pnpm build
```

After merging to `main`, restart backend with `DB_AUTO_MIGRATE=true`, then smoke:

```bash
curl http://localhost:4398/health
curl http://localhost:4398/ready
curl http://localhost:4399/ai/evals
```

With an admin JWT:

```bash
curl -H "Authorization: Bearer $TOKEN" http://localhost:4398/ai/evals/datasets
curl -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -d '{"datasetCode":"training_regression"}' http://localhost:4398/ai/evals/runs
curl -H "Authorization: Bearer $TOKEN" http://localhost:4398/ai/evals/runs/{runId}/results
```
