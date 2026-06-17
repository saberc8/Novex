# Codex App POC Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a standalone `apps/codex-app-poc` Next.js web app that closely reproduces the Codex desktop app workbench UI for demo use.

**Architecture:** The POC is a front-end-only Next app with static local data and client-side interaction state. It reuses the repository's existing Next/Tailwind/Vitest patterns from `apps/agent-workspace` but keeps all code inside the new app.

**Tech Stack:** Next.js 16, React 19, TypeScript, Tailwind CSS, lucide-react, Vitest, React Testing Library.

**Progress 2026-06-17:** The composer now submits real Agent run requests with `runtimeMode=model_loop`. It can include `NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID` as `modelRouteId`, allowing demos to pin a configured backend `CodeAgent` route while keeping the backend model registry/env fallback in control.

---

### Task 1: App Scaffold

**Files:**
- Create: `apps/codex-app-poc/package.json`
- Create: `apps/codex-app-poc/tsconfig.json`
- Create: `apps/codex-app-poc/next.config.mjs`
- Create: `apps/codex-app-poc/postcss.config.mjs`
- Create: `apps/codex-app-poc/tailwind.config.ts`
- Create: `apps/codex-app-poc/eslint.config.mjs`
- Create: `apps/codex-app-poc/vitest.config.ts`
- Create: `apps/codex-app-poc/next-env.d.ts`
- Create: `apps/codex-app-poc/README.md`

**Step 1: Copy the minimal configuration shape from `apps/agent-workspace`**

Use matching dependency versions and scripts, changing only the package name and dev port.

**Step 2: Install dependencies**

Run: `pnpm install`

Expected: lockfile and `node_modules` are created for the standalone app.

**Step 3: Verify scaffold commands are available**

Run: `pnpm typecheck`

Expected: fails only because no source files exist yet, or passes after Next types are recognized.

### Task 2: Failing Render Test

**Files:**
- Create: `apps/codex-app-poc/app/page.test.tsx`
- Create: `apps/codex-app-poc/app/page.tsx`

**Step 1: Write the failing test**

Test that the page renders:

- `我们应该在当前项目中做些什么？`
- `新对话`
- `搜索`
- `插件`
- `自动化`
- `完全访问`
- `5.5`
- `超高`

**Step 2: Run test to verify it fails**

Run: `pnpm test -- app/page.test.tsx`

Expected: FAIL because the page implementation is still missing or incomplete.

### Task 3: Minimal Workbench UI

**Files:**
- Create: `apps/codex-app-poc/src/app-client.tsx`
- Modify: `apps/codex-app-poc/app/page.tsx`
- Create: `apps/codex-app-poc/app/layout.tsx`
- Create: `apps/codex-app-poc/app/globals.css`

**Step 1: Implement the static shell**

Create the sidebar, main panel, title, composer, toolbar, directory row, and suggestions using static arrays and lucide icons.

**Step 2: Run the render test**

Run: `pnpm test -- app/page.test.tsx`

Expected: PASS.

### Task 4: Failing Command Menu Test

**Files:**
- Modify: `apps/codex-app-poc/app/page.test.tsx`

**Step 1: Add command menu behavior tests**

Test that typing `/` into the composer opens the menu and shows:

- `MCP`
- `个性`
- `推理模式`
- `模型`
- `状态`
- `记忆`

Test that pressing Escape closes the menu.

**Step 2: Run test to verify it fails**

Run: `pnpm test -- app/page.test.tsx`

Expected: FAIL because command menu behavior is not implemented yet.

### Task 5: Command Menu Interaction

**Files:**
- Modify: `apps/codex-app-poc/src/app-client.tsx`

**Step 1: Implement command menu state**

Open the menu when the text area contains `/`. Support ArrowDown, ArrowUp, Enter, and Escape.

**Step 2: Run tests**

Run: `pnpm test -- app/page.test.tsx`

Expected: PASS.

### Task 6: Agent API Runtime Wiring

Status: Completed.

**Files:**
- Modify: `apps/codex-app-poc/src/api/agent.ts`
- Modify: `apps/codex-app-poc/src/types/agent.ts`
- Modify: `apps/codex-app-poc/src/app-client.tsx`
- Modify: `apps/codex-app-poc/src/api/agent.test.ts`
- Modify: `apps/codex-app-poc/app/page.test.tsx`

**Implemented:**

- `createConfiguredModelAgentRun` centralizes the POC model-loop payload.
- `NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID` is trimmed and sent as `modelRouteId` only when nonblank.
- Composer submit uses the configured-model helper.
- API and page tests cover route-id inclusion and omission.

### Task 7: Verification

**Files:**
- Modify if needed: app files under `apps/codex-app-poc`

**Step 1: Run full checks**

Run:

```bash
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Expected: all commands exit 0.

**Step 2: Start local dev server**

Run: `pnpm dev`

Expected: Next starts on port `4413`.

**Step 3: Open the app in the browser**

Open: `http://localhost:4413`

Expected: the UI matches the approved Codex-like workbench design without overlap at desktop size.
