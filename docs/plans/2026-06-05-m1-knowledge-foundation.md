# M1 Knowledge Foundation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the first M1 knowledge control-plane slice with dataset/document metadata tables, RAG domain types, backend list/create APIs, and a real admin Knowledge page.

**Architecture:** `novex-rag` defines reusable RAG vocabulary while `backend` owns HTTP orchestration, RBAC, validation, and SQL persistence. The implementation stores metadata in PostgreSQL only; parser, embedding, Milvus, and answer generation are deferred.

**Tech Stack:** Rust 2021, Cargo workspace, Axum, SQLx, PostgreSQL migrations, Next.js 16, React 19, TypeScript, Tailwind, Vitest.

---

### Task 1: Extend novex-rag Domain Types

**Files:**
- Modify: `crates/novex-rag/src/knowledge.rs`

**Step 1: Write failing tests**

Add tests for:

- `DatasetStatus::default() == DatasetStatus::Draft`
- `ResourceVisibility::default() == ResourceVisibility::Private`
- `RetrievalMode::default() == RetrievalMode::Hybrid`
- `DocumentParseStatus::default() == DocumentParseStatus::Pending`

**Step 2: Run test to verify it fails**

Run: `cargo test -p novex-rag --offline`

Expected: FAIL because the new enums/defaults do not exist.

**Step 3: Implement minimal types**

Add:

- `DatasetStatus`
- `ResourceVisibility`
- `DocumentParseStatus`
- `IngestionStatus`
- `impl Default` for the defaults above

Keep existing `RetrievalMode` and add `Default`.

**Step 4: Run tests**

Run: `cargo test -p novex-rag --offline`

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-rag/src/knowledge.rs
git commit -m "feat: extend rag knowledge domain types"
```

### Task 2: Add Knowledge Metadata Migration

**Files:**
- Create: `backend/migrations/202606050002_create_ai_knowledge.sql`
- Create: `backend/migrations/202606050003_seed_ai_knowledge_permissions.sql`

**Step 1: Add migration SQL**

Create `ai_dataset` and `ai_document` tables with:

- `id BIGINT PRIMARY KEY`
- `tenant_id BIGINT NOT NULL DEFAULT 1`
- `owner_id BIGINT NOT NULL`
- `visibility SMALLINT NOT NULL DEFAULT 1`
- `acl_policy JSONB NOT NULL DEFAULT '{}'::jsonb`
- status fields
- document/chunk counters
- create/update audit fields

Add indexes for tenant, owner, dataset, status, and created time.

**Step 2: Add permission seed SQL**

Add button permissions under existing Knowledge menu:

- `ai:knowledge:create`
- `ai:knowledge:get`
- `ai:knowledge:update`
- `ai:knowledge:delete`
- `ai:knowledge:document:list`

Grant them to admin role.

**Step 3: Run backend tests**

Run: `cargo test -p backend-rust --offline`

Expected: PASS.

**Step 4: Commit**

```bash
git add backend/migrations/202606050002_create_ai_knowledge.sql backend/migrations/202606050003_seed_ai_knowledge_permissions.sql
git commit -m "feat: add knowledge metadata schema"
```

### Task 3: Add Backend Knowledge Service And Repository

**Files:**
- Create: `backend/src/application/ai/knowledge_service.rs`
- Create: `backend/src/infrastructure/persistence/ai_knowledge_repository.rs`
- Modify: `backend/src/application/ai/mod.rs`
- Modify: `backend/src/infrastructure/persistence/mod.rs`

**Step 1: Write failing service tests**

Test:

- empty dataset name returns bad request
- valid create command normalizes whitespace and defaults tenant/status/visibility/retrieval mode
- list query clamps page/size through existing pagination conventions

**Step 2: Run tests to verify failure**

Run: `cargo test -p backend-rust knowledge_service --offline`

Expected: FAIL because the service does not exist.

**Step 3: Implement minimal service and repository contracts**

Service:

- `KnowledgeService::list_datasets`
- `KnowledgeService::create_dataset`
- `KnowledgeService::list_documents`

Repository:

- SQL insert into `ai_dataset`
- SQL select paginated datasets
- SQL select documents by dataset

Use `next_id()`, existing `AppError`, and `PageResp`.

**Step 4: Run tests**

Run: `cargo test -p backend-rust knowledge_service --offline`

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai backend/src/infrastructure/persistence
git commit -m "feat: add knowledge service and repository"
```

### Task 4: Add Backend Knowledge HTTP Routes

**Files:**
- Create: `backend/src/interfaces/http/ai/knowledge.rs`
- Modify: `backend/src/interfaces/http/ai/mod.rs`

**Step 1: Write failing handler tests**

Test:

- `create_dataset` rejects missing `ai:knowledge:create`
- `list_datasets` rejects missing `ai:knowledge:list`
- registered route without auth returns `401`

**Step 2: Run tests to verify failure**

Run: `cargo test -p backend-rust ai::knowledge --offline`

Expected: FAIL because routes do not exist.

**Step 3: Implement routes**

Add:

- `GET /ai/knowledge/datasets`
- `POST /ai/knowledge/datasets`
- `GET /ai/knowledge/datasets/:id/documents`

Use `require_permission`.

**Step 4: Run backend tests**

Run: `cargo test -p backend-rust --offline`

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/interfaces/http/ai
git commit -m "feat: expose knowledge dataset APIs"
```

### Task 5: Add Admin Knowledge API Client

**Files:**
- Create: `admin/src/types/ai.ts`
- Create: `admin/src/api/ai/knowledge.ts`
- Create: `admin/src/api/ai/knowledge.test.ts`

**Step 1: Write failing API tests**

Use existing fetch mock style to assert:

- `listKnowledgeDatasets` calls `/ai/knowledge/datasets`
- `createKnowledgeDataset` posts to `/ai/knowledge/datasets`
- `listKnowledgeDocuments` calls `/ai/knowledge/datasets/:id/documents`

**Step 2: Run test to verify failure**

Run: `pnpm test src/api/ai/knowledge.test.ts`

Expected: FAIL because client does not exist.

**Step 3: Implement API client and types**

Add dataset/document response types and create command type.

**Step 4: Run tests**

Run: `pnpm test src/api/ai/knowledge.test.ts`

Expected: PASS.

**Step 5: Commit**

```bash
git add admin/src/api/ai admin/src/types/ai.ts
git commit -m "feat: add knowledge admin api client"
```

### Task 6: Replace Knowledge Placeholder With Dataset Page

**Files:**
- Modify: `admin/app/(main)/ai/knowledge/page.tsx`

**Step 1: Implement page**

Add a client component page that:

- loads datasets
- filters by name
- renders a table
- opens a create dialog
- uses permission gates for create action

**Step 2: Run frontend checks**

Run: `pnpm typecheck`

Expected: PASS.

Run: `pnpm test`

Expected: PASS.

Run: `pnpm build`

Expected: PASS and `/ai/knowledge` route generated.

**Step 3: Commit**

```bash
git add admin/app/'(main)'/ai/knowledge/page.tsx
git commit -m "feat: build knowledge dataset admin page"
```

### Task 7: Final Verification And Merge

**Files:**
- No new files.

**Step 1: Run full verification**

Run:

- `cargo test --workspace --offline`
- `pnpm typecheck`
- `pnpm test`
- `pnpm build`

Expected: all pass.

**Step 2: Merge locally**

Merge feature branch back to `main` with fast-forward if possible.

**Step 3: Verify on merged main**

Run:

- `cargo test --workspace --offline`
- `pnpm typecheck`
- `pnpm test`

Expected: all pass.

**Step 4: Report outcome**

Summarize changes, commits, and verification output.
