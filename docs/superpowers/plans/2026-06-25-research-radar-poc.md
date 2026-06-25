# Research Radar POC Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `apps/research-radar-poc`, a standalone Next.js app that submits real Novex Agent `model_loop` research scans and renders a polished research radar UI.

**Architecture:** The app is frontend-only and calls the existing `/ai/agents/runs` API. It reuses the Codex POC's API/event patterns while presenting a research-specific workflow, report parser, and evidence rail. UI styling is inspired by MIT/open-source community dashboard patterns from shadcn/ui blocks and Tremor, adapted into local Tailwind/React code instead of wholesale copying a full template.

**Tech Stack:** Next.js app router, React, TypeScript, Tailwind CSS, Vitest, Testing Library, lucide-react, existing Novex Agent HTTP API.

## Global Constraints

- Create a standalone app at `apps/research-radar-poc`.
- Use the existing Agent API directly; do not add a dedicated `/ai/research-radar/*` backend API in this slice.
- Submit scans with `runtimeMode: "model_loop"` and `workbenchContext.webSearchEnabled: true`.
- Parse model output by exact markdown headings, and fall back to raw report rendering when headings do not match.
- Keep prior scans visible when later scans fail.
- Use a real workbench first screen, not a marketing landing page.
- Use lucide-react icons and Tailwind CSS.
- Do not modify unrelated existing user changes such as `apps/codex-app-poc/app/layout.tsx`.

---

## File Structure

- Create `apps/research-radar-poc/package.json`: scripts and dependencies matching the other POC apps.
- Create `apps/research-radar-poc/tsconfig.json`, `next.config.mjs`, `postcss.config.mjs`, `tailwind.config.ts`, `eslint.config.mjs`, `vitest.config.ts`, `next-env.d.ts`: local app tooling.
- Create `apps/research-radar-poc/app/layout.tsx`: metadata and root layout.
- Create `apps/research-radar-poc/app/page.tsx`: server entry that renders the client app.
- Create `apps/research-radar-poc/app/globals.css`: Tailwind base and product theme.
- Create `apps/research-radar-poc/src/types/agent.ts`: Agent API DTOs.
- Create `apps/research-radar-poc/src/types/research.ts`: research scan, filters, rankings, report section DTOs.
- Create `apps/research-radar-poc/src/lib/auth.ts`: browser auth token lookup matching POC conventions.
- Create `apps/research-radar-poc/src/lib/api.ts`: API URL, JSON request, and error handling helpers.
- Create `apps/research-radar-poc/src/lib/research-report.ts`: prompt builder and markdown section parser.
- Create `apps/research-radar-poc/src/lib/agent-events.ts`: readable model delta and evidence summaries.
- Create `apps/research-radar-poc/src/api/agent.ts`: create run and list events.
- Create `apps/research-radar-poc/src/api/research.ts`: compose and submit research scan command.
- Create `apps/research-radar-poc/src/app-client.tsx`: full interactive research radar UI.
- Create tests next to the app and libs: `app/page.test.tsx`, `src/lib/research-report.test.ts`, `src/lib/agent-events.test.ts`, `src/api/research.test.ts`.
- Modify root docs only if verification reveals a missing run command note; otherwise keep docs unchanged.

---

### Task 1: App Scaffold And Tooling

**Files:**
- Create: `apps/research-radar-poc/package.json`
- Create: `apps/research-radar-poc/tsconfig.json`
- Create: `apps/research-radar-poc/next.config.mjs`
- Create: `apps/research-radar-poc/postcss.config.mjs`
- Create: `apps/research-radar-poc/tailwind.config.ts`
- Create: `apps/research-radar-poc/eslint.config.mjs`
- Create: `apps/research-radar-poc/vitest.config.ts`
- Create: `apps/research-radar-poc/next-env.d.ts`

**Interfaces:**
- Consumes: existing monorepo frontend conventions from `apps/codex-app-poc`.
- Produces: a runnable Next.js package named `@novex/research-radar-poc`.

- [ ] **Step 1: Write the package and config files**

Use the same versions and aliases as `apps/codex-app-poc`. `package.json` scripts:

```json
{
  "name": "@novex/research-radar-poc",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "next dev --webpack -p ${RESEARCH_RADAR_POC_PORT:-62607}",
    "build": "next build",
    "lint": "eslint .",
    "typecheck": "tsc --noEmit",
    "test": "vitest run"
  }
}
```

- [ ] **Step 2: Run typecheck to verify scaffold is discoverable**

Run: `pnpm --dir apps/research-radar-poc typecheck`

Expected: fails only because no app files exist yet, or passes if the empty scaffold is valid.

- [ ] **Step 3: Commit**

```bash
git add apps/research-radar-poc/package.json apps/research-radar-poc/tsconfig.json apps/research-radar-poc/next.config.mjs apps/research-radar-poc/postcss.config.mjs apps/research-radar-poc/tailwind.config.ts apps/research-radar-poc/eslint.config.mjs apps/research-radar-poc/vitest.config.ts apps/research-radar-poc/next-env.d.ts
git commit -m "feat: scaffold research radar poc app"
```

---

### Task 2: Core Types, API Helpers, And Agent Command

**Files:**
- Create: `apps/research-radar-poc/src/types/agent.ts`
- Create: `apps/research-radar-poc/src/types/research.ts`
- Create: `apps/research-radar-poc/src/lib/auth.ts`
- Create: `apps/research-radar-poc/src/lib/api.ts`
- Create: `apps/research-radar-poc/src/api/agent.ts`
- Create: `apps/research-radar-poc/src/api/research.ts`
- Test: `apps/research-radar-poc/src/api/research.test.ts`

**Interfaces:**
- Produces: `createResearchRadarRun(input: ResearchScanInput): Promise<AgentRunResp>`.
- Produces: `configuredModelRouteOptions(): ModelRouteOption[]`.
- Produces: `buildResearchRadarAgentRunCommand(input: ResearchScanInput): AgentRunCommand`.

- [ ] **Step 1: Write failing API command tests**

Test that `buildResearchRadarAgentRunCommand` returns `runtimeMode: "model_loop"`, `webSearchEnabled: true`, selected `modelRouteId`, `maxToolCalls: 4`, and a prompt containing the topic, filters, ranking, and required headings.

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/research-radar-poc test src/api/research.test.ts`

Expected: FAIL because the API files do not exist.

- [ ] **Step 3: Implement DTOs and API helpers**

Define the exact Agent DTOs used by the existing endpoint and research DTOs:

```ts
export type ResearchRanking = "balanced" | "importance" | "recency" | "beginner";
export type ResearchFilter = "papers" | "projects" | "datasets" | "benchmarks" | "news" | "community";
export type ResearchScanInput = {
  topic: string;
  filters: ResearchFilter[];
  ranking: ResearchRanking;
  routeId?: string;
};
```

Use `NEXT_PUBLIC_API_BASE_URL`, dev token lookup from local storage, JSON API response unwrapping, and `/ai/agents/runs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/research-radar-poc test src/api/research.test.ts`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/research-radar-poc/src/types apps/research-radar-poc/src/lib/auth.ts apps/research-radar-poc/src/lib/api.ts apps/research-radar-poc/src/api apps/research-radar-poc/src/api/research.test.ts
git commit -m "feat: add research radar agent command"
```

---

### Task 3: Report Parser And Event Evidence

**Files:**
- Create: `apps/research-radar-poc/src/lib/research-report.ts`
- Create: `apps/research-radar-poc/src/lib/agent-events.ts`
- Test: `apps/research-radar-poc/src/lib/research-report.test.ts`
- Test: `apps/research-radar-poc/src/lib/agent-events.test.ts`

**Interfaces:**
- Produces: `parseResearchReport(markdown: string): ParsedResearchReport`.
- Produces: `summarizeModelDeltas(events: AgentRunEventResp[]): ModelDeltaSummary | null`.
- Produces: `summarizeResearchEvent(event: AgentRunEventResp): ResearchEventEvidence`.

- [ ] **Step 1: Write failing parser tests**

Cover the exact eight headings, missing headings fallback, whitespace trimming, and section ordering.

- [ ] **Step 2: Write failing event tests**

Cover sorted model deltas and a `tool_observation` payload with `{ dryRun: true, status: "dry_run" }`.

- [ ] **Step 3: Run tests to verify they fail**

Run: `pnpm --dir apps/research-radar-poc test src/lib/research-report.test.ts src/lib/agent-events.test.ts`

Expected: FAIL because implementation files do not exist.

- [ ] **Step 4: Implement parser and event summarizers**

`parseResearchReport` returns `{ structured: true, sections }` only when at least four known headings are present. Otherwise it returns `{ structured: false, sections: [{ id: "raw", title: "Research Report", content: markdown.trim() }] }`.

Event summaries should classify model deltas as `Assistant`, tool observations as a readable tool title, and unknown events by `eventType`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `pnpm --dir apps/research-radar-poc test src/lib/research-report.test.ts src/lib/agent-events.test.ts`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/research-radar-poc/src/lib/research-report.ts apps/research-radar-poc/src/lib/agent-events.ts apps/research-radar-poc/src/lib/research-report.test.ts apps/research-radar-poc/src/lib/agent-events.test.ts
git commit -m "feat: parse research reports and evidence"
```

---

### Task 4: Polished Research Radar UI

**Files:**
- Create: `apps/research-radar-poc/app/layout.tsx`
- Create: `apps/research-radar-poc/app/page.tsx`
- Create: `apps/research-radar-poc/app/globals.css`
- Create: `apps/research-radar-poc/src/app-client.tsx`
- Test: `apps/research-radar-poc/app/page.test.tsx`

**Interfaces:**
- Consumes: `createResearchRadarRun`, `listAgentRunEvents`, `parseResearchReport`, `summarizeModelDeltas`, and `summarizeResearchEvent`.
- Produces: a first-screen workbench with topic input, filter chips, ranking segmented control, model selector, scan history, report cards, and evidence rail.

- [ ] **Step 1: Write failing UI tests**

Tests should assert:

- heading `Research Radar`;
- topic input label `研究主题`;
- filters `Papers`, `Projects`, `Datasets`, `Benchmarks`, `News`, `Community`;
- ranking control default `Balanced`;
- submit button label `启动雷达扫描`;
- a mocked successful run renders `Research Overview`, run id, and evidence.

- [ ] **Step 2: Run UI test to verify it fails**

Run: `pnpm --dir apps/research-radar-poc test app/page.test.tsx`

Expected: FAIL because app files do not exist.

- [ ] **Step 3: Implement the app shell and UI**

Use a three-column desktop layout and stacked mobile layout. Adapt dashboard visual ideas from community patterns:

- shadcn-style sidebar/header rhythm, compact controls, icon buttons, and bordered panels;
- Tremor-style metric/radar cards, evidence lists, and dense dashboard spacing;
- custom topic-focused content, not copied marketing/admin pages.

Important components inside `app-client.tsx`:

```ts
function ResearchRadarApp(): JSX.Element
function TopicComposer(props: TopicComposerProps): JSX.Element
function FilterDock(props: FilterDockProps): JSX.Element
function RankingControl(props: RankingControlProps): JSX.Element
function ReportWorkspace(props: ReportWorkspaceProps): JSX.Element
function EvidenceRail(props: EvidenceRailProps): JSX.Element
function ModelSelector(props: ModelSelectorProps): JSX.Element
```

- [ ] **Step 4: Run UI test to verify it passes**

Run: `pnpm --dir apps/research-radar-poc test app/page.test.tsx`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/research-radar-poc/app apps/research-radar-poc/src/app-client.tsx apps/research-radar-poc/app/page.test.tsx
git commit -m "feat: build research radar workspace ui"
```

---

### Task 5: Full Verification And Local Run

**Files:**
- Modify only files required by failing tests or type errors under `apps/research-radar-poc`.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: verified app and local dev server URL.

- [ ] **Step 1: Run full app test suite**

Run: `pnpm --dir apps/research-radar-poc test`

Expected: PASS.

- [ ] **Step 2: Run typecheck**

Run: `pnpm --dir apps/research-radar-poc typecheck`

Expected: PASS.

- [ ] **Step 3: Run lint**

Run: `pnpm --dir apps/research-radar-poc lint`

Expected: PASS.

- [ ] **Step 4: Run production build**

Run: `pnpm --dir apps/research-radar-poc build`

Expected: PASS.

- [ ] **Step 5: Check whitespace**

Run: `git diff --check`

Expected: no output.

- [ ] **Step 6: Start dev server**

Run: `NEXT_PUBLIC_API_BASE_URL=http://localhost:62601 pnpm --dir apps/research-radar-poc dev`

Expected: server listens on `http://localhost:62607`.

- [ ] **Step 7: Commit verification fixes if needed**

```bash
git add apps/research-radar-poc
git commit -m "fix: verify research radar poc"
```
