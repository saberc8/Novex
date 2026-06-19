# Customer App Foundation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the first customer-facing Novex app foundation by turning `apps/training-web` from a README placeholder into a runnable employee training workspace backed by existing knowledge APIs.

**Architecture:** Admin remains the control plane. Customer apps live under `apps/*`, use the same backend API envelope and auth token model, and expose business workflows rather than admin tables. The first slice follows the updated architecture: `training-web` is the POC entry, with FastGPT/Dify-style workspace layout and Codex-style run/event terminology reserved for later Agent views.

**Tech Stack:** Next.js 16, React 19, TypeScript, Tailwind CSS, Vitest, Testing Library, existing Novex Rust/Axum backend APIs.

---

## Reference Patterns

- Codex: model visible lifecycle as thread/turn/item events. Novex Agent C-side views should show run steps, tool calls, file changes, errors, and final messages as event items, not opaque logs.
- Dify: app lifecycle is create app, configure workflow/chatflow, run, publish, share. Novex should keep this as delivery flow, but POC only displays run/publish state and avoids a drag workflow builder.
- FastGPT: application UX groups chat, knowledge, workflow, tools, and publishing in a workspace layout. Novex C-side apps should use dense work surfaces with left navigation, central task area, and right context/citation/status panels.

## Task 1: Scaffold Training Web App

**Files:**
- Create: `apps/training-web/package.json`
- Create: `apps/training-web/next.config.mjs`
- Create: `apps/training-web/tsconfig.json`
- Create: `apps/training-web/vitest.config.ts`
- Create: `apps/training-web/postcss.config.mjs`
- Create: `apps/training-web/tailwind.config.ts`
- Create: `apps/training-web/app/layout.tsx`
- Create: `apps/training-web/app/globals.css`
- Modify: `apps/training-web/README.md`

**Step 1: Write the failing test**

Create `apps/training-web/src/lib/navigation.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { trainingNavItems } from "./navigation";

describe("training navigation", () => {
  it("keeps the POC customer app sections in the expected order", () => {
    expect(trainingNavItems.map((item) => item.href)).toEqual([
      "/",
      "/ask",
      "/quiz",
      "/records",
      "/notifications"
    ]);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `cd apps/training-web && pnpm test`

Expected: FAIL because `src/lib/navigation.ts` does not exist.

**Step 3: Implement minimal scaffold**

Add the app config files by mirroring the admin toolchain but using port `4401`. Add `src/lib/navigation.ts` with stable nav metadata.

**Step 4: Run test**

Run: `cd apps/training-web && pnpm test`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/training-web
git commit -m "feat: scaffold training customer app"
```

## Task 2: Add Training Web API Client

**Files:**
- Create: `apps/training-web/src/lib/api.ts`
- Create: `apps/training-web/src/lib/auth.ts`
- Create: `apps/training-web/src/api/knowledge.ts`
- Create: `apps/training-web/src/types/api.ts`
- Create: `apps/training-web/src/types/knowledge.ts`
- Create: `apps/training-web/src/api/knowledge.test.ts`

**Step 1: Write API tests**

Test that:

- `listDatasets()` sends `GET /ai/knowledge/datasets`.
- `askDataset(datasetId, command)` sends `POST /ai/knowledge/datasets/:id/ask`.
- `getAuthToken()` reads `novex_token` from local storage and does not throw during SSR.

**Step 2: Run tests to verify failure**

Run: `cd apps/training-web && pnpm test`

Expected: FAIL because API files are missing.

**Step 3: Implement API client**

Mirror the admin envelope parsing behavior:

- Use `NEXT_PUBLIC_API_BASE_URL || "http://localhost:4398"`.
- Send `Authorization: Bearer <token>` when present.
- Parse `{ code, success, data, msg }`.
- Throw clean user-facing errors without exposing raw response bodies.

**Step 4: Run tests**

Run: `cd apps/training-web && pnpm test`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/training-web/src
git commit -m "feat: add training app api client"
```

## Task 3: Build Training Workspace UI

**Files:**
- Create: `apps/training-web/src/components/training-shell.tsx`
- Create: `apps/training-web/src/components/metric-strip.tsx`
- Create: `apps/training-web/src/components/citation-list.tsx`
- Create: `apps/training-web/src/app-client.tsx`
- Create: `apps/training-web/app/page.tsx`
- Create: `apps/training-web/app/page.test.tsx`

**Step 1: Write UI test**

Test that the first screen renders:

- `AI 员工培训`
- `待学习任务`
- `知识库问答`
- `测验与错题`
- `引用来源`

**Step 2: Run test to verify failure**

Run: `cd apps/training-web && pnpm test`

Expected: FAIL because the page/components are missing.

**Step 3: Implement UI**

Build a dense customer workbench:

- Left navigation: learning, ask, quiz, records, notifications.
- Main panel: learning task list and question input.
- Right panel: citations, model route/status, progress.
- Use icons from `lucide-react`.
- No hero/marketing layout, no nested cards, no decorative orbs.
- Use fixture data when API calls fail or token is absent.

**Step 4: Run tests**

Run: `cd apps/training-web && pnpm test`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/training-web
git commit -m "feat: build training workspace page"
```

## Task 4: Wire Live Knowledge Ask Flow

**Files:**
- Modify: `apps/training-web/src/app-client.tsx`
- Modify: `apps/training-web/app/page.test.tsx`

**Step 1: Extend tests**

Mock `listDatasets` and `askDataset`. Assert:

- The app loads the first dataset.
- Submitting a question calls `askDataset`.
- Citations from the response render in the right panel.

**Step 2: Run test to verify failure**

Run: `cd apps/training-web && pnpm test`

Expected: FAIL because the page is static.

**Step 3: Implement live flow**

On mount:

- Load datasets.
- Select the first dataset with documents.
- Show fallback state when unauthenticated.

On ask:

- Submit `question` with `limit: 5`.
- Render answer, citations, trace id, hit count, and strategy.
- Keep errors in UI state.

**Step 4: Run tests**

Run: `cd apps/training-web && pnpm test`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/training-web
git commit -m "feat: connect training app knowledge ask"
```

## Task 5: Update Delivery Template Metadata

**Files:**
- Modify: `templates/training-app/template.json`
- Modify: `templates/training-app/README.md`
- Modify: `apps/training-web/README.md`

**Step 1: Write existing backend template test expectation**

Extend the existing template manifest test to assert the training template includes:

- `frontendApp: "training-web"`
- pages `learn`, `ask`, `quiz`, `records`, `notifications`
- smoke command for `apps/training-web`

**Step 2: Run test to verify failure**

Run: `cargo test -p backend delivery_template_manifest --offline`

Expected: FAIL until template metadata is updated.

**Step 3: Update manifests**

Add C-side page list and smoke commands while keeping existing template fields intact.

**Step 4: Run tests**

Run: `cargo test -p backend delivery_template_manifest --offline`

Expected: PASS.

**Step 5: Commit**

```bash
git add templates/training-app apps/training-web backend/src/application/ai/template_service.rs
git commit -m "feat: register training frontend template"
```

## Task 6: Verify and Smoke Test

**Files:**
- No source changes expected.

**Step 1: Run frontend app checks**

Run:

```bash
cd apps/training-web
pnpm typecheck
pnpm lint
pnpm test
pnpm build
```

Expected: all pass.

**Step 2: Run Rust checks**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
```

Expected: all pass.

**Step 3: Start app**

Run:

```bash
cd apps/training-web
NEXT_PUBLIC_API_BASE_URL=http://localhost:4398 pnpm dev -- -p 4401
```

Expected: app serves at `http://localhost:4401`.

**Step 4: Browser verification**

Open `http://localhost:4401` and verify:

- Desktop and mobile layout are non-overlapping.
- The first viewport is the training workbench.
- Question flow renders fallback data without auth and live data when `novex_token` exists.

**Step 5: Merge**

Fast-forward merge into `main` only after checks pass.
