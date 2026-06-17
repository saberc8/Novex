# Agent Provider Embedding/Rerank Lease Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Record tenant-bound provider-call leases for knowledge-base embedding and rerank provider calls.

**Architecture:** Keep raw static provider helpers for health checks and parser-level tests. Add tenant-bound `ModelRuntimeService` wrapper methods that create provider-call lease rows, reuse heartbeat refresh, call the raw provider helper, and complete the lease with sanitized request/response metadata. Update knowledge retrieval to call the wrapper methods.

**Tech Stack:** Rust, SQLx, Tokio, PostgreSQL, existing Novex model runtime and knowledge service.

---

### Task 1: Lease Record And Source Contract Tests

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add tests for:
- embedding provider request lease record maps tenant, route, purpose, request kind, source, input counts, and no API key content;
- model runtime source contract exposes embedding/rerank lease wrappers and uses the shared begin/complete lease path;
- knowledge service source contract calls tenant-bound wrapper methods instead of raw static provider helpers.

**Step 2: Run red tests**

Run: `cargo test -p backend-rust provider_call_lease --offline`

Expected: FAIL because `model_provider_call_lease_record_from_provider_request`, `embed_texts_for_source`, and `rerank_documents_for_source` do not exist yet.

### Task 2: Tenant-Bound Embedding/Rerank Wrappers

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Implement wrapper and sanitized metadata**

Add:
- `ModelRuntimeService::embed_texts_for_source`
- `ModelRuntimeService::rerank_documents_for_source`
- `ModelRuntimeService::execute_provider_call_with_lease`
- `model_provider_call_lease_record_from_provider_request`
- `model_provider_call_lease_provider_request_payload`
- `model_provider_call_lease_completion_from_provider_payload`

**Step 2: Run green tests**

Run: `cargo test -p backend-rust provider_call_lease --offline`

Expected: PASS.

### Task 3: Knowledge Service Integration

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/knowledge_service.rs`

**Step 1: Replace raw production provider calls**

Use:
- `model_runtime.embed_texts_for_source(&route, &texts, "ai.knowledge.embedding")`
- `model_runtime.embed_texts_for_source(&route, &[question.to_owned()], "ai.knowledge.query_embedding")`
- `model_runtime.rerank_documents_for_source(&route, question, &documents, "ai.knowledge.rerank")`

**Step 2: Run focused tests**

Run:
- `cargo test -p backend-rust runtime_embedding --offline`
- `cargo test -p backend-rust rerank_ --offline`

Expected: PASS.

### Task 4: Docs, Verification, Merge

Status: Completed.

**Files:**
- Create: `docs/plans/2026-06-17-agent-provider-embedding-rerank-lease-design.md`
- Create: `docs/plans/2026-06-17-agent-provider-embedding-rerank-lease.md`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update migration matrix**

Move embedding/rerank provider leases into implemented runtime-loop evidence. Keep media provider leases as a separate remaining slice.

**Step 2: Run verification**

Run:
- `cargo fmt -- --check`
- `cargo test --workspace --offline`

Expected: PASS.

**Step 3: Commit, merge, clean**

Commit feature work, merge `feat/enterprise-agent-foundation` into `main`, rerun full verification on `main`, run `cargo clean` in both worktrees, and sync feature to main.

**Verification evidence so far:**
- Red: `cargo test -p backend-rust provider_call_lease --offline` failed on missing `model_provider_call_lease_record_from_provider_request`.
- Green: `cargo test -p backend-rust provider_call_lease --offline`
- Green: `cargo test -p backend-rust runtime_embedding --offline`
- Green: `cargo test -p backend-rust rerank_ --offline`
- Green: `cargo fmt -- --check`
- Green: `cargo test --workspace --offline` passed with 739 backend unit tests, workspace crate tests/doc-tests, and one ignored live RAG e2e infra test.
