# Studio Mind Map Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a reusable Novex Studio Artifact runtime and ship `mind_map.generate` for knowledge notebooks.

**Architecture:** Backend owns Studio action discovery, artifact generation, persistence, permissions, and citations. Frontend consumes action/artifact APIs and renders mind-map artifacts in the right Studio panel. Plugins can later declare Studio actions, but this slice seeds a built-in action and keeps execution deterministic enough for tests.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL migrations, Next.js, React, TypeScript, Vitest, Tailwind.

---

### Task 1: Backend Studio Contract Tests

**Files:**
- Create: `backend/src/interfaces/http/ai/studio.rs`
- Modify: `backend/src/interfaces/http/ai/mod.rs`
- Create: `backend/migrations/202606090004_create_ai_studio_artifact.sql`
- Create: `backend/migrations/202606090005_seed_ai_studio_permissions.sql`

**Steps:**
1. Write a failing Rust test that asserts the Studio migration defines `ai_studio_action`, `ai_studio_artifact`, `mind_map.generate`, `artifact_type`, `content_json`, and `ai:studio:artifact:create`.
2. Write a failing handler test that `generate_artifact` rejects a user missing `ai:studio:artifact:create`.
3. Run `cargo test -p backend studio --offline` and confirm it fails because the Studio module is missing.

### Task 2: Backend Studio Repository and Service

**Files:**
- Create: `backend/src/infrastructure/persistence/ai_studio_repository.rs`
- Create: `backend/src/application/ai/studio_service.rs`
- Modify: `backend/src/infrastructure/persistence/mod.rs`
- Modify: `backend/src/application/ai/mod.rs`
- Modify: `backend/src/interfaces/http/ai/studio.rs`

**Steps:**
1. Write service tests for action command normalization and deterministic mind-map fallback.
2. Run `cargo test -p backend studio_service --offline` and confirm it fails.
3. Implement repository DTOs for action list, artifact list/get, and insert.
4. Implement `StudioService` with `list_actions`, `list_dataset_artifacts`, `get_artifact`, and `generate_artifact`.
5. Run targeted backend tests.

### Task 3: Frontend API and Types

**Files:**
- Create: `apps/chat-web/src/types/studio.ts`
- Create: `apps/chat-web/src/api/studio.ts`
- Create: `apps/chat-web/src/api/studio.test.ts`

**Steps:**
1. Write failing Vitest API tests for action list, artifact list, artifact get, and generation endpoints.
2. Run `pnpm vitest run src/api/studio.test.ts`.
3. Implement Studio types and API wrappers.
4. Re-run the API test.

### Task 4: Chat Web Studio Panel

**Files:**
- Modify: `apps/chat-web/src/app-client.tsx`
- Modify: `apps/chat-web/app/page.test.tsx`

**Steps:**
1. Extend the page test mocks and write a failing test that clicks “思维导图”, calls `generateStudioArtifact`, and renders returned node labels.
2. Run `pnpm vitest run app/page.test.tsx`.
3. Implement Studio action/artifact state, loading, generation, and mind-map rendering.
4. Re-run the page test.

### Task 5: Verification

**Commands:**
- `cd backend && cargo test -p backend studio --offline`
- `cd apps/chat-web && pnpm vitest run src/api/studio.test.ts app/page.test.tsx`
- If targeted checks pass, run `cd apps/chat-web && pnpm typecheck`

