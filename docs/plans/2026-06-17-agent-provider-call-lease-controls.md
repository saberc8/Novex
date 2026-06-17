# Agent Provider Call Lease Controls Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add tenant-scoped provider-call lease list and stale-expire controls so Novex can operate the durable lease table introduced for Agent provider calls.

**Architecture:** Reuse `ModelRuntimeService` and `backend/src/interfaces/http/ai/model.rs`. Add response/query DTOs, list/expire SQL helpers, permission-gated HTTP routes, permission seed migration, source-contract tests, and migration-matrix evidence.

**Tech Stack:** Rust, SQLx, PostgreSQL migrations, Axum, existing Novex permission middleware.

---

### Task 1: Permission And HTTP Contract

Status: Completed.

**Files:**
- Create: `backend/migrations/202606170009_seed_ai_model_provider_call_lease_permissions.sql`
- Modify: `backend/src/interfaces/http/ai/model.rs`

**Step 1: Write failing tests**

Add tests for:
- permission seed contains `ai:model:providerCallLease:list` and `ai:model:providerCallLease:expire`;
- routes contain `/ai/models/provider-call-leases` and `/ai/models/provider-call-leases/expire-stale`;
- list and expire handlers reject missing permissions before service work.

**Step 2: Run red tests**

Run: `cargo test -p backend-rust provider_call_lease_controls --offline`

Expected: FAIL.

**Step 3: Implement permissions and handlers**

Add constants, routes, handler functions, and migration seed.

**Step 4: Run green tests**

Run: `cargo test -p backend-rust provider_call_lease_controls --offline`

Expected: PASS.

### Task 2: Service DTOs And Query Mapping

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add tests for:
- query normalization clamps `limit` and accepts only known statuses;
- row mapping exposes lifecycle fields and omits request/response payload bodies;
- source contract reads `FROM ai_model_provider_call_lease` with `tenant_id = $1`.

**Step 2: Run red tests**

Run: `cargo test -p backend-rust provider_call_lease_controls --offline`

Expected: FAIL.

**Step 3: Implement DTOs and list method**

Add:
- `ModelProviderCallLeaseQuery`
- `ModelProviderCallLeaseResp`
- `ModelProviderCallLeaseSweepResp`
- `ModelRuntimeService::list_provider_call_leases`

**Step 4: Run green tests**

Run: `cargo test -p backend-rust provider_call_lease_controls --offline`

Expected: PASS.

### Task 3: Stale Expire Control

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add source-contract tests for:
- `UPDATE ai_model_provider_call_lease`
- `status = 'expired'`
- `lease_expires_at < $2`
- `status = 'running'`
- `update_user = $3`

**Step 2: Run red tests**

Run: `cargo test -p backend-rust provider_call_lease_controls --offline`

Expected: FAIL.

**Step 3: Implement expire method**

Add `ModelRuntimeService::expire_stale_provider_call_leases(user_id)`.

**Step 4: Run green tests**

Run: `cargo test -p backend-rust provider_call_lease_controls --offline`

Expected: PASS.

### Task 4: Docs, Verification, Merge

Status: Completed.

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-provider-call-lease-controls.md`

**Step 1: Update migration matrix**

Move lease list/expire controls into implemented runtime-loop evidence. Keep provider-native cancel, streaming heartbeat refresh, and embedding/rerank/media leases as next.

**Step 2: Run verification**

Run:
- `cargo fmt -- --check`
- `cargo test --workspace --offline`

Expected: PASS.

**Step 3: Commit, merge, clean**

Commit feature work, merge `feat/enterprise-agent-foundation` into `main`, rerun full verification on `main`, run `cargo clean` in both worktrees, and sync feature to main.

**Verification evidence:**
- `cargo test -p backend-rust provider_call_lease_controls --offline`
- `cargo fmt -- --check`
- `cargo test --workspace --offline`
