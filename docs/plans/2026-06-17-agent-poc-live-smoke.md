# Agent POC Live Smoke Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a repeatable live smoke command that proves the POC can run a configured-model Agent loop through the real backend API.

**Architecture:** Implement a Node ESM smoke runner in `apps/codex-app-poc/scripts/agent-live-smoke.mjs`, test its pure request/poll/assert helpers with Vitest, expose it as `pnpm smoke:agent-live`, and document the required environment.

**Tech Stack:** Node ESM, built-in `fetch`, Vitest, existing Novex Agent HTTP API.

---

### Task 1: Red Tests

Status: Pending.

**Files:**
- Create: `apps/codex-app-poc/scripts/agent-live-smoke.test.mjs`

**Step 1: Write failing tests**

Cover:

- skip when `NOVEX_LIVE_AGENT_SMOKE` is not `1`;
- request payload includes `runtimeMode=model_loop`, bounded budget, and trimmed `modelRouteId`;
- poll loop succeeds when events contain terminal status and `model_inference`;
- route mismatch fails when a requested route differs from inference evidence;
- timeout fails with last status.

**Step 2: Verify red**

Run:

```bash
cd apps/codex-app-poc
pnpm test -- scripts/agent-live-smoke.test.mjs
```

Expected: FAIL because `scripts/agent-live-smoke.mjs` does not exist.

### Task 2: Smoke Runner

Status: Pending.

**Files:**
- Create: `apps/codex-app-poc/scripts/agent-live-smoke.mjs`
- Modify: `apps/codex-app-poc/package.json`

**Step 1: Implement helpers**

Add:

- `smokeConfigFromEnv(env)`;
- `agentRunPayload(config)`;
- `createAgentRun(fetch, config)`;
- `listAgentEvents(fetch, config, runId)`;
- `waitForAgentRunEvidence(fetch, config, run)`;
- `assertAgentSmokeEvidence(config, run, events)`;
- `runAgentLiveSmoke({ env, fetch, logger })`.

**Step 2: Add package script**

Add:

```json
"smoke:agent-live": "node scripts/agent-live-smoke.mjs"
```

**Step 3: Verify green**

Run:

```bash
cd apps/codex-app-poc
pnpm test -- scripts/agent-live-smoke.test.mjs
```

### Task 3: Documentation

Status: Pending.

**Files:**
- Modify: `apps/codex-app-poc/README.md`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-poc-live-smoke.md`

**Step 1: Document live command**

Explain required backend/provider setup and command:

```bash
NOVEX_LIVE_AGENT_SMOKE=1 \
NEXT_PUBLIC_API_BASE_URL=http://localhost:4398 \
NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID=runtime.llm \
NOVEX_AGENT_SMOKE_TOKEN=<jwt-if-needed> \
pnpm smoke:agent-live
```

**Step 2: Update plan statuses**

Mark tasks complete after verification.

### Task 4: Verify, Commit, Merge

Status: Pending.

**Step 1: Verify feature branch**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd apps/codex-app-poc && pnpm test -- src/api/agent.test.ts app/page.test.tsx scripts/agent-live-smoke.test.mjs && pnpm typecheck
cd ../agent-workspace && pnpm test -- src/api/agent.test.ts
```

**Step 2: Commit and merge**

Commit feature branch, merge into `main`, rerun the same verification on `main`, then run `cargo clean` in main and feature worktrees.
