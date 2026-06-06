# POC Gap Closure Batch 1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the first acceptance-critical gaps between the current Novex implementation and `docs/ARCHITECTURE.md` by adding RAG vector persistence and tenant-aware AI request entry points.

**Architecture:** Keep the existing POC services and APIs, then add durable vector metadata tables beside the current chunk metadata so Milvus/default vector collection mapping has a stable contract. Extend AI request handling to carry tenant identity from `CurrentUser` into knowledge operations while preserving default tenant behavior for existing tests and local POC data.

**Tech Stack:** Rust, Axum, SQLx/PostgreSQL migrations, existing Novex backend unit tests.

---

### Task 1: RAG Vector Persistence Contract

**Files:**
- Create: `backend/migrations/202606060001_create_ai_vector_persistence.sql`
- Modify: `backend/src/infrastructure/persistence/ai_knowledge_repository.rs`

**Steps:**
1. Add failing repository tests that require `ai_vector_collection`, `ai_embedding`, tenant indexes, dataset filters, embedding refs, dimensions, and JSONB vector payload columns.
2. Run the focused repository test and confirm it fails because the migration does not exist.
3. Add the migration with collection and embedding tables.
4. Insert a default vector collection when creating a dataset.
5. Persist one embedding record per indexed chunk when chunk metadata contains an embedding vector.
6. Re-run the focused repository tests.

### Task 2: Tenant-Aware Knowledge Entry Points

**Files:**
- Modify: `backend/src/domain/auth/model.rs`
- Modify: `backend/src/infrastructure/persistence/user_repository.rs`
- Modify: `backend/src/application/ai/knowledge_service.rs`
- Modify: `backend/src/interfaces/http/ai/knowledge.rs`
- Modify: affected test constructors using `CurrentUser`

**Steps:**
1. Add failing tests showing `CurrentUser` carries a tenant id and knowledge HTTP handlers pass it to service methods.
2. Add `tenant_id` to `CurrentUser`, defaulting to tenant 1 when no active tenant membership is found.
3. Add tenant-aware knowledge service methods while keeping existing default-tenant wrappers.
4. Update knowledge HTTP handlers to call tenant-aware methods.
5. Re-run focused backend tests for auth/current-user construction and AI knowledge handlers.

### Task 3: Verification

**Steps:**
1. Run `cargo fmt --check`.
2. Run focused backend tests for knowledge repository/service/routes.
3. Run `cargo test -p backend-rust application::ai::knowledge_service interfaces::http::ai::knowledge infrastructure::persistence::ai_knowledge_repository`.
4. If focused tests pass, run broader `cargo test -p backend-rust`.
