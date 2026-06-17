# Agent Provider Call Lease Heartbeat Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refresh active provider-call leases while a model provider call is still running so long awaits and future streaming calls are not incorrectly expired.

**Architecture:** Reuse `ai_model_provider_call_lease.heartbeat_at` and `lease_expires_at`. Add a small heartbeat task inside `ModelRuntimeService::execute_normalized_chat_completion_with_provider_call_lease`, scoped by tenant and lease id.

**Tech Stack:** Rust, Tokio, SQLx, PostgreSQL.

---

### Task 1: Heartbeat Contract Tests

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add tests for:
- heartbeat expiry calculation;
- source contract proving heartbeat starts around provider calls;
- source contract proving SQL refreshes `heartbeat_at` and `lease_expires_at` only for `running` rows in the current tenant.

**Step 2: Run red tests**

Run: `cargo test -p backend-rust provider_call_lease_heartbeat --offline`

Expected: FAIL because heartbeat helper functions do not exist yet.

### Task 2: Heartbeat Runtime

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Implement heartbeat**

Add:
- `MODEL_PROVIDER_CALL_LEASE_HEARTBEAT_SECONDS`
- `model_provider_call_lease_expiry_from_heartbeat`
- `ModelProviderCallLeaseHeartbeat`
- `start_model_provider_call_lease_heartbeat`
- `refresh_model_provider_call_lease_heartbeat`

Start heartbeat after lease creation, stop it after provider await returns, and write terminal completion as before.

**Step 2: Run green tests**

Run:
- `cargo test -p backend-rust provider_call_lease_heartbeat --offline`
- `cargo test -p backend-rust provider_call_lease --offline`

Expected: PASS.

### Task 3: Docs, Verification, Merge

Status: Completed.

**Files:**
- Create: `docs/plans/2026-06-17-agent-provider-call-lease-heartbeat-design.md`
- Create: `docs/plans/2026-06-17-agent-provider-call-lease-heartbeat.md`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update migration matrix**

Move streaming/provider-call lease heartbeat refresh into implemented runtime-loop evidence while keeping provider-native cancel endpoints and embedding/rerank/media leases as next work.

**Step 2: Run verification**

Run:
- `cargo fmt -- --check`
- `cargo test --workspace --offline`

Expected: PASS.

**Step 3: Commit, merge, clean**

Commit feature work, merge `feat/enterprise-agent-foundation` into `main`, rerun full verification on `main`, run `cargo clean` in both worktrees, and sync feature to main.

**Verification evidence so far:**
- Red: `cargo test -p backend-rust provider_call_lease_heartbeat --offline` failed on missing `model_provider_call_lease_expiry_from_heartbeat`.
- Green: `cargo test -p backend-rust provider_call_lease_heartbeat --offline`
- Green: `cargo test -p backend-rust provider_call_lease --offline`
- Green: `cargo fmt -- --check`
- Green: `cargo test --workspace --offline`
