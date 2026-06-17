# Agent Frontend Model Delta Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Display persisted `model_delta` run events as readable streaming model output in the Agent workspace and Codex POC.

**Architecture:** Add lightweight event-presentation helpers inside each frontend app, then render the helper summary where users inspect a run. The helper reads durable run-event payloads and keeps final answers separate from live model-output evidence.

**Tech Stack:** Next.js, React, TypeScript, Vitest, existing Novex Agent run-event API.

## Global Constraints

- Do not change backend run-event schemas in this slice.
- Do not change SSE/WebSocket transport in this slice.
- Preserve raw delta `content` whitespace.
- Do not treat `model_delta` chunks as final answers.
- Keep UI changes scoped to existing Agent workspace and Codex POC surfaces.

---

### Task 1: Agent Workspace Delta Presentation Helper

**Files:**
- Create: `apps/agent-workspace/src/lib/agent-events.ts`
- Create: `apps/agent-workspace/src/lib/agent-events.test.ts`

**Interfaces:**
- Consumes: `AgentRunEventResp[]`.
- Produces: `summarizeModelDeltas(events): ModelDeltaSummary | null`.

- [ ] **Step 1: Write the failing test**

Create `apps/agent-workspace/src/lib/agent-events.test.ts` with a test that passes three events: one ignored tool event, one `model_delta` with `deltaIndex: 1` and content `" world"`, and one `model_delta` with `deltaIndex: 0` and content `"Hello"`. Assert:

```ts
expect(summary?.text).toBe("Hello world");
expect(summary?.chunkCount).toBe(2);
expect(summary?.routeId).toBe("runtime.llm.code_agent");
expect(summary?.model).toBe("gpt-compatible");
```

- [ ] **Step 2: Run red verification**

Run:

```bash
cd apps/agent-workspace && pnpm test -- src/lib/agent-events.test.ts
```

Expected: FAIL because `src/lib/agent-events.ts` does not exist.

- [ ] **Step 3: Implement minimal helper**

Create `summarizeModelDeltas` with these types:

```ts
export interface ModelDeltaSummary {
  text: string;
  chunkCount: number;
  routeId?: string;
  provider?: string;
  model?: string;
}
```

The implementation should unwrap `payload.item` when present, accept direct payloads for compatibility, filter `type === "model_delta"`, sort by numeric `deltaIndex` then `sequenceNo`, join raw string `content`, and return `null` when no chunks exist.

- [ ] **Step 4: Run green verification**

Run:

```bash
cd apps/agent-workspace && pnpm test -- src/lib/agent-events.test.ts
```

Expected: PASS.

### Task 2: Agent Workspace Live Output Panel

**Files:**
- Modify: `apps/agent-workspace/src/app-client.tsx`
- Modify: `apps/agent-workspace/app/page.test.tsx`

**Interfaces:**
- Consumes: `summarizeModelDeltas(events)`.
- Produces: a visible "Live model output" panel with text and chunk count.

- [ ] **Step 1: Write the failing page test**

Add a test that makes `listAgentRunEvents` return two `model_delta` events and verifies the page renders:

```ts
expect(await screen.findByText("Live model output")).toBeTruthy();
expect(await screen.findByText("Hello world")).toBeTruthy();
expect(await screen.findByText("2 chunks")).toBeTruthy();
```

- [ ] **Step 2: Run red verification**

Run:

```bash
cd apps/agent-workspace && pnpm test -- app/page.test.tsx
```

Expected: FAIL because the panel is not rendered.

- [ ] **Step 3: Implement panel**

Import `summarizeModelDeltas`, derive a memoized summary from `events`, and render a compact panel above the workflow event list when a summary exists. The panel should show chunk count and optional route/model metadata without replacing raw event cards.

- [ ] **Step 4: Run green verification**

Run:

```bash
cd apps/agent-workspace && pnpm test -- app/page.test.tsx src/lib/agent-events.test.ts
```

Expected: PASS.

### Task 3: Codex POC Event Snapshot and Delta Rendering

**Files:**
- Create: `apps/codex-app-poc/src/lib/agent-events.ts`
- Create: `apps/codex-app-poc/src/lib/agent-events.test.ts`
- Modify: `apps/codex-app-poc/src/types/agent.ts`
- Modify: `apps/codex-app-poc/src/api/agent.ts`
- Modify: `apps/codex-app-poc/src/api/agent.test.ts`
- Modify: `apps/codex-app-poc/src/app-client.tsx`
- Modify: `apps/codex-app-poc/app/page.test.tsx`

**Interfaces:**
- Consumes: `/ai/agents/runs/:id/events?page=1&size=100`.
- Produces: Codex POC run result with a live model-output panel when delta events exist.

- [ ] **Step 1: Write failing helper and page/API tests**

Add the same helper test as Task 1 for the POC app. Extend API tests to expect `listAgentRunEvents(42, { page: 1, size: 100 })` hits `/ai/agents/runs/42/events?page=1&size=100`. Extend the page submit test fetch mock so the second response returns two `model_delta` events and assert the UI renders "Live model output" plus "Hello world".

- [ ] **Step 2: Run red verification**

Run:

```bash
cd apps/codex-app-poc && pnpm test -- src/lib/agent-events.test.ts src/api/agent.test.ts app/page.test.tsx
```

Expected: FAIL because helper/API/UI support is missing.

- [ ] **Step 3: Implement POC support**

Add `AgentRunEventResp`, `AgentRunEventQuery`, and `PageResult`-shaped response typing as needed. Add `listAgentRunEvents` to the POC API module. In the composer, after `createConfiguredModelAgentRun` succeeds, fetch run events and render the summary panel when chunks exist. Event-fetch failure should not hide the run result; it should leave the delta panel absent.

- [ ] **Step 4: Run green verification**

Run:

```bash
cd apps/codex-app-poc && pnpm test -- src/lib/agent-events.test.ts src/api/agent.test.ts app/page.test.tsx
```

Expected: PASS.

### Task 4: Matrix and Integration

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Updates Runtime loop POC evidence and narrows remaining streaming work to stream-native runtime API, Responses output-text deltas, and partial tool-call JSON parsing.

- [ ] **Step 1: Update matrix**

Move Runtime loop to `slice-50 implemented`, mention frontend model-delta rendering in Runtime loop and Runtime loop POC rows, and add this plan link.

- [ ] **Step 2: Run full verification**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
cd apps/agent-workspace && pnpm test -- src/api/agent.test.ts app/page.test.tsx src/lib/agent-events.test.ts && pnpm typecheck
cd ../codex-app-poc && pnpm test -- src/api/agent.test.ts app/page.test.tsx src/lib/agent-events.test.ts scripts/agent-live-smoke.test.mjs && pnpm typecheck
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 3: Commit and integrate**

Commit the feature branch, merge it to `main` with `--no-ff`, verify on `main`, run `cargo clean` in both worktrees, and fast-forward the feature branch to `main`.
