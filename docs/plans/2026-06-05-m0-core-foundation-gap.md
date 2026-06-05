# M0 Core Foundation Gap Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the first architecture gap batch by adding control-plane/model-registry tables, model registry read APIs, and RAG trace model-route resolution.

**Architecture:** Additive migrations establish missing M0 contracts without changing current POC behavior. Backend exposes read-only model registry summaries under existing model permissions. RAG trace route names come from `novex-model` runtime config when available, with deterministic local fallbacks.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL, serde, existing Next.js admin remains unchanged in this first backend slice.

---

### Task 1: Control Plane Foundation Migration

**Files:**
- Create: `backend/migrations/202606050014_create_foundation_control_plane.sql`
- Modify: `backend/src/interfaces/http/ai/foundation.rs`

**Steps:**
1. Write a failing migration test that includes the new migration and asserts table names for tenant, ACL, quota, identity, OAuth, and secret contracts.
2. Run `cargo test -p backend-rust foundation_control_plane --offline`; expect missing file or missing table names.
3. Add the additive migration with default platform tenant/admin membership and required indexes.
4. Run the targeted test and commit.

### Task 2: Model Registry Migration

**Files:**
- Create: `backend/migrations/202606050015_create_ai_model_registry.sql`
- Modify: `backend/src/interfaces/http/ai/model.rs`

**Steps:**
1. Write a failing migration test that asserts all model registry table names and critical columns: `credential_ref`, `network_zone`, `model_kind`, `route_purpose`, `cost_spec`.
2. Run `cargo test -p backend-rust model_registry --offline`; expect missing file or missing content.
3. Add provider/deployment/profile/credential/route/health/usage tables and safe default seed metadata.
4. Run the targeted test and commit.

### Task 3: Model Registry Read API

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `backend/src/interfaces/http/ai/model.rs`

**Steps:**
1. Write failing service tests for a registry summary built from rows and for route registration requiring auth.
2. Add `ModelRegistrySummary`, row structs, SQL queries, and `GET /ai/models/registry`.
3. Ensure no raw secret values are returned.
4. Run `cargo test -p backend-rust model --offline` and commit.

### Task 4: RAG Trace Runtime Route Resolution

**Files:**
- Modify: `backend/src/application/ai/knowledge_service.rs`

**Steps:**
1. Write failing tests for route resolution:
   - env-backed config returns `runtime.embedding`, `runtime.reranker`, and `runtime.llm`;
   - missing env falls back to `local-keyword`, `none`, and `local-extractive`.
2. Replace fixed constants in trace creation with a small resolver using `novex_model::ModelRuntimeConfig`.
3. Run `cargo test -p backend-rust rag_ask --offline` and commit.

### Task 5: Verification

**Commands:**
- `cargo fmt -- --check`
- `cargo test --workspace --offline`
- `pnpm typecheck`
- `pnpm lint`
- `pnpm test`
- `pnpm build`

**Smoke:**
- Apply migrations through backend startup against the local Docker PostgreSQL.
- Call `/health`, `/ready`, `/ai/models/runtime-config`, `/ai/models/registry`.
- Confirm the worktree is clean and merge with `--ff-only`.
