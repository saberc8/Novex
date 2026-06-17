# Agent POC Configured Model Route Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow POC and API callers to pin a configured `CodeAgent` model route for real Agent model-loop execution.

**Architecture:** Extend the Agent command DTO with optional `modelRouteId`, normalize it in the service layer, pass it through the shared inline/queued model-loop executor into `ModelChatCommand.route_id`, and expose a POC helper that reads `NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID`.

**Tech Stack:** Rust, SQLx/Postgres service tests, Novex model runtime, Next.js/TypeScript/Vitest.

---

### Task 1: Backend Red Tests

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add tests proving:

- `AgentRunCommand` accepts `modelRouteId` and trims whitespace.
- overlong route ids are rejected.
- model-loop source contains `route_id: command.model_route_id.clone()`.
- compaction source also carries the same route id when it calls the configured CodeAgent model.

**Step 2: Verify red**

Run:

```bash
cargo test -p backend-rust agent_poc_configured_model_route --offline
```

Expected: FAIL until the DTO field and route propagation exist.

### Task 2: Frontend Red Tests

**Files:**
- Modify: `apps/codex-app-poc/src/api/agent.test.ts`
- Modify: `apps/codex-app-poc/src/types/agent.ts`
- Modify: `apps/codex-app-poc/src/api/agent.ts`
- Modify: `apps/codex-app-poc/src/app-client.tsx`

**Step 1: Write failing tests**

Add tests proving:

- `createConfiguredModelAgentRun` includes `modelRouteId` from `NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID`.
- the helper omits `modelRouteId` when the env value is blank.
- the composer submit path uses the configured-model helper instead of duplicating payload construction.

**Step 2: Verify red**

Run:

```bash
cd apps/codex-app-poc
pnpm test -- src/api/agent.test.ts app/page.test.tsx
```

Expected: FAIL until the helper and type field exist.

### Task 3: Backend Implementation

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Add command field**

Add `model_route_id: Option<String>` with `#[serde(default)]` to `AgentRunCommand`.

**Step 2: Normalize**

Trim the optional field and enforce the existing 128-character model route bound.

**Step 3: Propagate**

Set `route_id: command.model_route_id.clone()` for CodeAgent model calls in the model loop and context compaction.

**Step 4: Verify**

Run:

```bash
cargo test -p backend-rust agent_poc_configured_model_route --offline
cargo test -p backend-rust model_loop --offline
cargo test -p backend-rust queued_model_loop --offline
```

### Task 4: POC Implementation

**Files:**
- Modify: `apps/codex-app-poc/src/types/agent.ts`
- Modify: `apps/codex-app-poc/src/api/agent.ts`
- Modify: `apps/codex-app-poc/src/app-client.tsx`

**Step 1: Extend type**

Add `modelRouteId?: string` to `AgentRunCommand`.

**Step 2: Add helper**

Create `createConfiguredModelAgentRun(input)` that builds the current model-loop payload and includes `modelRouteId` only when `NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID` is nonblank.

**Step 3: Wire composer**

Use the helper from the submit handler.

**Step 4: Verify**

Run:

```bash
cd apps/codex-app-poc
pnpm test -- src/api/agent.test.ts app/page.test.tsx
```

### Task 5: Docs, Full Verify, Commit, Merge

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-08-codex-app-poc.md`

**Step 1: Update docs**

Record that POC configured route selection is implemented and that live E2E still depends on provider env/infra.

**Step 2: Verify feature branch**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd apps/codex-app-poc && pnpm test -- src/api/agent.test.ts app/page.test.tsx
cd ../agent-workspace && pnpm test -- src/api/agent.test.ts
```

**Step 3: Commit and merge**

Commit feature branch, merge into `main`, verify `main`, then run `cargo clean` in both worktrees.
