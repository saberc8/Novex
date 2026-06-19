# Agent Provider Call Lease Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a durable provider-call lease boundary for tenant-bound model calls so Novex can observe and later control in-flight Agent provider work.

**Architecture:** Create `ai_model_provider_call_lease`, add a serde-skipped local provider-call context to `ModelChatCommand`, wrap tenant-bound `ModelRuntimeService` chat provider awaits with begin/complete lease persistence, and surface the lease id in Agent model inference trace payloads.

**Tech Stack:** Rust, SQLx, PostgreSQL migrations, serde, existing Novex model runtime and Agent runtime services.

---

### Task 1: Provider Call Lease Migration

Status: Completed.

**Files:**
- Create: `backend/migrations/202606170008_create_ai_model_provider_call_lease.sql`
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write the failing migration contract test**

Add a test named `provider_call_lease_migration_defines_runtime_contract`.

It must read the new migration and assert it contains:
- `CREATE TABLE IF NOT EXISTS ai_model_provider_call_lease`
- `tenant_id`
- `run_id`
- `route_code`
- `route_purpose`
- `provider_type`
- `request_kind`
- `lease_owner`
- `lease_expires_at`
- `heartbeat_at`
- `status`
- `error_kind`
- `idx_ai_model_provider_call_lease_active`
- `idx_ai_model_provider_call_lease_run`

**Step 2: Run test to verify it fails**

Run: `cargo test -p backend provider_call_lease_migration --offline`

Expected: FAIL because the migration file does not exist or lacks fields.

**Step 3: Add migration**

Create `backend/migrations/202606170008_create_ai_model_provider_call_lease.sql` with the table and indexes.

**Step 4: Run test to verify it passes**

Run: `cargo test -p backend provider_call_lease_migration --offline`

Expected: PASS.

### Task 2: Lease Record Mapping

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing unit tests**

Add tests:
- `provider_call_context_is_local_only_and_not_serialized`
- `provider_call_lease_record_maps_route_context_and_request_kind`
- `provider_call_lease_completion_maps_success_usage_and_cost`
- `provider_call_lease_completion_maps_failure_class`
- `provider_call_lease_completion_maps_cancelled_status`

**Step 2: Run tests to verify they fail**

Run: `cargo test -p backend provider_call_lease --offline`

Expected: FAIL because types and helpers do not exist.

**Step 3: Implement data structs and pure mapping helpers**

Add:
- `ModelProviderCallContext`
- optional skipped `provider_call_context` on `ModelChatCommand`
- optional `provider_call_lease_id` on `ModelChatResp`
- internal `ModelProviderCallLeaseRecord`
- internal `ModelProviderCallLeaseCompletion`
- pure helpers that map command/route/result/error into records.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p backend provider_call_lease --offline`

Expected: PASS.

### Task 3: Tenant-Bound Lease Persistence

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing source-contract tests**

Add tests:
- `provider_call_lease_source_contract_wraps_tenant_bound_chat_calls`
- `provider_call_lease_source_contract_persists_begin_and_completion`

They must prove:
- tenant-bound `chat_completion_for_source` and `chat_completion_for_purpose` route through the lease wrapper;
- fallback attempts pass `attempt_kind`;
- wrapper calls `begin_model_provider_call_lease`;
- wrapper calls `complete_model_provider_call_lease`;
- the success path sets `response.provider_call_lease_id`.

**Step 2: Run tests to verify they fail**

Run: `cargo test -p backend provider_call_lease_source_contract --offline`

Expected: FAIL because wrapper and persistence helpers do not exist.

**Step 3: Implement persistence and wrapper**

Add SQLx helpers:
- `begin_model_provider_call_lease`
- `complete_model_provider_call_lease`

Add `ModelRuntimeService::execute_normalized_chat_completion_with_provider_call_lease`.

Update tenant-bound chat methods to use it.

**Step 4: Run focused tests**

Run:
- `cargo test -p backend provider_call_lease --offline`
- `cargo test -p backend model_chat_payload --offline`
- `cargo test -p backend provider_compact_transport --offline`
- `cargo test -p backend provider_lifecycle --offline`

Expected: PASS.

### Task 4: Agent Runtime Context And Trace Link

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add or extend tests:
- model-loop source contract includes `provider_call_context` with run id for model calls;
- context compaction source contract includes `provider_call_context` with run id and source;
- `model_inference_event_payload` includes `providerCallLeaseId` when present.

**Step 2: Run tests to verify they fail**

Run:
- `cargo test -p backend agent_provider_call_lease_context_contract_links_run_and_source --offline`
- `cargo test -p backend model_inference_event_payload_links_provider_call_lease --offline`

Expected: FAIL before context wiring.

**Step 3: Wire Agent context**

Update model-loop sampling, context compaction, and Guardian review model calls where run context is available.

**Step 4: Run focused tests**

Run:
- `cargo test -p backend provider_call_lease --offline`
- `cargo test -p backend model_loop --offline`
- `cargo test -p backend remote_compaction --offline`
- `cargo test -p backend guardian_model_review --offline`

Expected: PASS.

### Task 5: Migration Matrix And Full Verification

Status: Completed.

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-provider-call-lease.md`

**Step 1: Update docs**

Move provider-call lease tables from runtime-loop remaining work into implemented evidence, while leaving provider-native cancel endpoints and heartbeat/native cancel controls as next.

**Step 2: Run full verification**

Run:
- `cargo fmt -- --check`
- `cargo test --workspace --offline`

Expected: PASS.

**Step 3: Commit, merge, clean**

Commit feature work, merge `feat/enterprise-agent-foundation` into `main`, rerun full verification on `main`, run `cargo clean` in both worktrees, and sync feature to main.

**Verification evidence:**
- `cargo test -p backend provider_call_lease --offline`
- `cargo fmt -- --check`
- `cargo test --workspace --offline`
