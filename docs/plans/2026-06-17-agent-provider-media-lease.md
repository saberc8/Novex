# Agent Provider Media Lease Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Record tenant-bound provider-call leases for Agent media image generation provider calls.

**Architecture:** Keep dry-run and media job persistence in `agent_service.rs`, but move the live provider HTTP call into `ModelRuntimeService`. The runtime wrapper reuses the existing provider-call lease helper, heartbeat refresh, sanitized request metadata, and success/failure completion path.

**Tech Stack:** Rust, SQLx, Tokio, reqwest, PostgreSQL, existing Novex model runtime, Agent tool runtime, and `novex-tools` media payload parser.

## Global Constraints

- Do not persist API keys, prompt text, generated image bytes, or raw secret-bearing payloads in provider-call lease request metadata.
- Preserve existing Agent media dry-run behavior when no runtime or endpoint is configured.
- Preserve existing media job/asset persistence payload fields on successful image generation.
- Use the existing `ai_model_provider_call_lease` table and list/expire controls; do not add new HTTP endpoints.
- Follow TDD: write and run failing tests before production code.

---

### Task 1: Media Lease Record And Source Contract Tests

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `backend/src/application/ai/agent_service.rs`

**Interfaces:**
- Consumes: existing `model_provider_call_lease_record_from_provider_request`.
- Produces: expected future method names `ModelRuntimeService::generate_media_image` and `ModelRuntimeService::generate_media_image_for_source`.

- [x] **Step 1: Write failing tests**

Add tests for:
- media provider request lease record maps tenant, route, purpose, request kind, source, prompt length, size/count, and no API key content;
- model runtime source contract exposes media generation wrappers and uses the shared lease begin/heartbeat/complete path;
- Agent media tool source contract calls `generate_media_image_for_source` and does not create direct live provider `reqwest` calls in `execute_media_image_tool`.

- [x] **Step 2: Run red tests**

Run: `cargo test -p backend provider_call_lease --offline`

Expected: FAIL because `generate_media_image_for_source` does not exist yet and Agent media tool still performs the direct HTTP call.

### Task 2: Tenant-Bound Media Runtime Wrapper

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Produces:
  - `pub struct ModelMediaImageGenerationResp`
  - `pub async fn ModelRuntimeService::generate_media_image(route: &ModelRuntimeRoute, request: &MediaImageGenerationRequest) -> Result<ModelMediaImageGenerationResp, AppError>`
  - `pub async fn ModelRuntimeService::generate_media_image_for_source(&self, route: &ModelRuntimeRoute, request: &MediaImageGenerationRequest, source: &str) -> Result<ModelMediaImageGenerationResp, AppError>`

- [x] **Step 1: Implement raw media provider adapter**

Move the live HTTP call shape from `execute_media_image_tool` into `ModelRuntimeService::generate_media_image`, preserving timeout, auth headers, provider payload, HTTP status error, and response parsing.

- [x] **Step 2: Implement lease wrapper**

Add `generate_media_image_for_source` using `execute_provider_call_with_lease` with:
- `user_id = MODEL_RUNTIME_SYSTEM_USER_ID`
- `purpose = ModelRoutePurpose::MediaGeneration`
- `request_kind = "media_image_generation"`
- sanitized request metadata: `promptCharCount`, `size`, `count`
- sanitized response metadata: `routeId`, `provider`, `model`, `latencyMs`, `assetUrlPresent`, `providerAssetIdPresent`

- [x] **Step 3: Run green provider lease tests**

Run: `cargo test -p backend provider_call_lease --offline`

Expected: PASS.

### Task 3: Agent Media Tool Integration

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Interfaces:**
- Consumes: `ModelRuntimeService::generate_media_image_for_source`.

- [x] **Step 1: Replace direct live HTTP call**

In `execute_media_image_tool`, keep route resolution and dry-run branches, then call:

```rust
model_runtime
    .generate_media_image_for_source(&route, &request, "ai.agent.media.image")
    .await
```

Build the existing success payload from the returned provider response.

- [x] **Step 2: Run focused media tests**

Run: `cargo test -p backend media_ --offline`

Expected: PASS.

### Task 4: Docs, Matrix, Verification, Merge

Status: Completed.

**Files:**
- Create: `docs/plans/2026-06-17-agent-provider-media-lease-design.md`
- Create: `docs/plans/2026-06-17-agent-provider-media-lease.md`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

- [x] **Step 1: Update migration matrix**

Move media provider leases into implemented runtime-loop evidence and remove them from the remaining runtime loop gap list.

- [x] **Step 2: Run full verification**

Run:
- `cargo fmt -- --check`
- `cargo test --workspace --offline`
- `git diff --check`

Expected: PASS.

- [x] **Step 3: Commit, merge, clean**

Commit feature work, merge `feat/enterprise-agent-foundation` into `main`, rerun full verification on `main`, run `cargo clean` in both worktrees, and sync feature to `main`.

**Verification evidence so far:**
- Red: `cargo test -p backend provider_call_lease --offline` failed on missing `generate_media_image`.
- Green: `cargo test -p backend provider_call_lease --offline`
- Green: `cargo test -p backend media_ --offline`
- Green: `cargo fmt -- --check`
- Green: `cargo test --workspace --offline` passed with 741 backend unit tests, workspace crate tests/doc-tests, and one ignored live RAG e2e infra test.
- Green: `git diff --check`
