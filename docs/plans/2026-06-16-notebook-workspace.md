# Notebook Workspace Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a NotebookLM-like source workspace on top of Novex RAG, citations, agent traces, and generated artifacts.

**Architecture:** Notebook is a workspace layer over existing datasets, documents, parser jobs, chunks, citations, and model routes. It must not become a separate RAG stack. The agent runtime should treat notebook tools as normal low/medium-risk tools: source listing/reading is low risk, note/artifact creation is medium risk, and all answers must preserve citation anchors.

**Tech Stack:** Rust/Axum/SQLx, `novex-rag`, `KnowledgeService`, `StudioService` artifacts where useful, new backend notebook service/repository, optional future `apps/notebook-workspace`.

---

## Current Status

Backend slice is implemented and verified.

- Persistence: `ai_notebook_workspace`, `ai_notebook_source`, `ai_notebook_artifact`.
- API: workspace create/list, source add/list, artifact list/generate, cited ask.
- Permissions: `ai:notebook:list`, `ai:notebook:create`, `ai:notebook:source`, `ai:notebook:artifact`, `ai:notebook:ask`.
- Retrieval: Notebook ask builds on `KnowledgeService` and constrains local/Milvus retrieval with workspace source document filters.
- Artifacts: `summary`, `faq`, `study_guide`, and `note` generation save `citation_payload` and `source_trace_id`.

Verification evidence:

```bash
cargo test -p backend notebook_migration_defines_workspace_source_and_artifact_tables --offline
cargo test -p backend notebook_ --offline
cargo test -p backend notebook_ask --offline
cargo test -p backend knowledge_service --offline
cargo test -p backend notebook_artifact --offline
cargo test -p backend --offline
cargo fmt -- --check
cargo test --workspace --offline
```

Remaining product work:

- Build a dedicated Notebook UI or wire these APIs into an existing POC screen.
- Add collaborative editing, PDF visual annotation, and multi-user comments when the product asks for them.
- Upgrade multi-dataset answer synthesis beyond the current best-trace response selection.

## Scope

In scope:

- Notebook workspace and source-set tables.
- Source import from existing knowledge datasets/documents.
- Source-aware retrieval plan.
- Cited Q&A endpoint.
- Generated notes, FAQ, summary, study guide artifacts.
- Eval/feedback capture for cited answers.

Out of scope:

- Collaborative editing.
- Full PDF visual annotation UI.
- Replacing existing knowledge dataset management.
- Multi-user real-time cursor or comments.

## Task 1: Persist Notebook Workspace and Source Sets

**Files:**
- Add: `backend/src/infrastructure/persistence/ai_notebook_repository.rs`
- Modify: `backend/src/infrastructure/persistence/mod.rs`
- Create: `backend/migrations/202606160003_create_ai_notebook_workspace.sql`
- Add: `backend/src/application/ai/notebook_service.rs`
- Modify: `backend/src/application/ai/mod.rs`

**Step 1: Write failing migration test**

Add:

```rust
#[test]
fn notebook_migration_defines_workspace_source_and_artifact_tables() {
    let migration = include_str!("../../../migrations/202606160003_create_ai_notebook_workspace.sql");

    assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_notebook_workspace"));
    assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_notebook_source"));
    assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_notebook_artifact"));
    assert!(migration.contains("knowledge_dataset_id"));
    assert!(migration.contains("citation_payload"));
}
```

**Step 2: Run failing test**

Run:

```bash
cargo test -p backend notebook_migration_defines_workspace_source_and_artifact_tables --offline
```

Expected: FAIL.

**Step 3: Add schema**

Tables:

- `ai_notebook_workspace`: tenant, owner, name, description, metadata, status.
- `ai_notebook_source`: workspace, dataset/document refs, source type, title, citation metadata, status.
- `ai_notebook_artifact`: workspace, artifact kind, title, content, citation payload, source trace id, status.

**Step 4: Add repository/service skeleton**

Add create/list/get methods:

- `create_workspace`
- `list_workspaces`
- `add_source`
- `list_sources`
- `create_artifact`
- `list_artifacts`

**Step 5: Verify**

Run:

```bash
cargo test -p backend notebook_migration_defines_workspace_source_and_artifact_tables --offline
cargo test -p backend notebook_service --offline
```

Expected: PASS.

**Step 6: Commit**

```bash
git add backend/migrations/202606160003_create_ai_notebook_workspace.sql backend/src/infrastructure/persistence/ai_notebook_repository.rs backend/src/infrastructure/persistence/mod.rs backend/src/application/ai/notebook_service.rs backend/src/application/ai/mod.rs
git commit -m "feat: add notebook workspace persistence"
```

## Task 2: Add Notebook HTTP API and Permissions

**Files:**
- Add: `backend/src/interfaces/http/ai/notebook.rs`
- Modify: `backend/src/interfaces/http/ai/mod.rs`
- Create: `backend/migrations/202606160004_seed_ai_notebook_permissions.sql`

**Step 1: Write failing route tests**

Add tests:

- `notebook_workspace_route_is_registered_and_requires_auth`
- `notebook_handlers_bind_runtime_to_current_tenant`
- `notebook_permission_seed_contains_route_permissions`

**Step 2: Run failing test**

Run:

```bash
cargo test -p backend notebook_workspace_route_is_registered_and_requires_auth --offline
```

Expected: FAIL.

**Step 3: Implement routes**

Routes:

- `POST /ai/notebooks/workspaces`
- `GET /ai/notebooks/workspaces`
- `POST /ai/notebooks/workspaces/:workspaceId/sources`
- `GET /ai/notebooks/workspaces/:workspaceId/sources`
- `GET /ai/notebooks/workspaces/:workspaceId/artifacts`

Permissions:

- `ai:notebook:list`
- `ai:notebook:create`
- `ai:notebook:source`
- `ai:notebook:artifact`
- `ai:notebook:ask`

**Step 4: Verify**

Run:

```bash
cargo test -p backend notebook_ --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/interfaces/http/ai/notebook.rs backend/src/interfaces/http/ai/mod.rs backend/migrations/202606160004_seed_ai_notebook_permissions.sql
git commit -m "feat: expose notebook workspace api"
```

## Task 3: Add Source-aware Retrieval and Cited Ask

**Files:**
- Modify: `backend/src/application/ai/notebook_service.rs`
- Modify: `backend/src/application/ai/knowledge_service.rs`
- Modify: `crates/novex-rag/src/retrieval.rs`

**Step 1: Write failing tests**

Add:

- `notebook_ask_retrieves_only_workspace_sources`
- `notebook_answer_preserves_source_citation_labels`
- `notebook_ask_records_agent_trace_and_feedback_target`

**Step 2: Run failing test**

Run:

```bash
cargo test -p backend notebook_ask_retrieves_only_workspace_sources --offline
```

Expected: FAIL.

**Step 3: Implement ask command**

Add:

```rust
pub struct NotebookAskCommand {
    pub question: String,
    pub limit: Option<u64>,
    pub generation_profile: Option<String>,
}
```

Behavior:

1. Load workspace source document ids.
2. Call existing retrieval with a source filter.
3. Use `runtime.llm.rag_answer`.
4. Return answer, citations, trace id, and source ids.
5. Persist trace/eval candidate metadata.

**Step 4: Add route**

`POST /ai/notebooks/workspaces/:workspaceId/ask`

**Step 5: Verify**

Run:

```bash
cargo test -p backend notebook_ask --offline
cargo test -p backend knowledge_service --offline
```

Expected: PASS.

**Step 6: Commit**

```bash
git add backend/src/application/ai/notebook_service.rs backend/src/application/ai/knowledge_service.rs crates/novex-rag/src/retrieval.rs backend/src/interfaces/http/ai/notebook.rs
git commit -m "feat: answer notebook questions with citations"
```

## Task 4: Generate Notebook Artifacts

**Files:**
- Modify: `backend/src/application/ai/notebook_service.rs`
- Modify: `backend/src/interfaces/http/ai/notebook.rs`

**Step 1: Write failing tests**

Add:

- `notebook_artifact_command_accepts_summary_faq_and_study_guide`
- `notebook_artifact_generation_uses_workspace_sources`
- `notebook_artifact_records_citation_payload`

**Step 2: Run failing test**

Run:

```bash
cargo test -p backend notebook_artifact_command_accepts_summary_faq_and_study_guide --offline
```

Expected: FAIL.

**Step 3: Implement artifact generation**

Add artifact kinds:

- `summary`
- `faq`
- `study_guide`
- `note`

Prompt must include source citations and instruct the model to cite or state missing evidence.

**Step 4: Verify**

Run:

```bash
cargo test -p backend notebook_artifact --offline
cargo test -p backend --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/notebook_service.rs backend/src/interfaces/http/ai/notebook.rs
git commit -m "feat: generate notebook source artifacts"
```

## Task 5: Full Verification

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
```

Expected: PASS.

Acceptance is met only when a user can create a notebook workspace, attach existing knowledge sources, ask a cited question constrained to those sources, and generate at least one cited artifact.
