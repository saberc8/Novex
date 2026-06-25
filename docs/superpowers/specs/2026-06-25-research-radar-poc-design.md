# Research Radar POC Design

## Brief

Create a standalone `apps/research-radar-poc` web POC for AI research direction tracking. A user enters a research topic, chooses discovery preferences, and starts a real Novex Agent `model_loop` run. The app turns the run output and run events into a research radar: overview, active topics, key authors and institutions, representative work, a beginner reading route, possible research openings, and experiment plans.

The POC validates that the existing Novex agent runtime, configured model route, web search tool, capability context, and event evidence can support a research intelligence workflow without adding a new backend service in the first slice.

## Current State

- The repo already has several Next.js POC apps under `apps/`.
- `apps/codex-app-poc` already calls `/ai/agents/runs` with `runtimeMode: "model_loop"`, an optional configured model route, bounded budgets, and workbench context.
- `apps/codex-app-poc` already summarizes model delta and raw agent events into readable UI evidence.
- The backend already exposes Agent run create/list/event routes under `/ai/agents/runs`.
- Novex already documents a web search provider chain for `web.search`; when external providers are missing, the tool may degrade or dry-run depending on backend configuration.
- The existing visual language for POC apps is a quiet workbench UI with side navigation, compact controls, restrained borders, and dense but readable output panels.

## Design Choice

Create a new standalone app named `apps/research-radar-poc`.

The new app should reuse patterns from `apps/codex-app-poc` rather than sharing code in a broad refactor. For the first implementation, duplication of small API helpers and event summarizers is acceptable because this is a product POC and the interaction model differs from the Codex-like workbench. Shared packages can be extracted later if both apps stabilize around the same contracts.

The first slice uses the existing Agent API directly. Do not add a dedicated `/ai/research-radar/*` backend API unless the existing Agent contract cannot carry a required field safely.

## Product Shape

The first screen is the working research radar, not a landing page.

The app has four visible zones:

1. Header: product name, model route selector, run status, and a compact "new scan" action.
2. Input panel: research topic, optional focus chips, source preferences, sorting priority, and submit button.
3. Radar workspace: structured research output with sections for overview, trends, authors/institutions, representative work, reading route, research openings, and experiment plans.
4. Evidence rail: live model output, tool/event evidence, source labels, run id, trace id, and errors.

The UI should feel like a research operations tool: organized, scannable, and calm. It should not use a marketing hero, large decorative cards, or a single-color palette. Use the existing Novex POC style as the visual source: light background, white work surfaces, subtle borders, compact cards, lucide icons, and clear typographic hierarchy.

## User Workflow

1. User opens `apps/research-radar-poc`.
2. User enters a research topic such as `LLM agent memory`, `multimodal RAG`, or `AI coding agents`.
3. User optionally toggles focus chips:
   - Papers
   - Open source projects
   - Datasets
   - Benchmarks
   - News
   - Community discussion
4. User chooses ranking priority:
   - Balanced
   - Importance
   - Recency
   - Beginner friendly
5. User submits the scan.
6. Frontend creates a real Agent run with `runtimeMode: "model_loop"` and `webSearchEnabled: true`.
7. Frontend fetches run events after creation and renders both final output and evidence.
8. User can adjust the topic or filters and run another scan.

## Agent Run Contract

All scans call the existing Agent endpoint:

```ts
POST /ai/agents/runs
```

The command uses:

```ts
{
  input: string;
  runtimeMode: "model_loop";
  autoApprove: false;
  modelRouteId?: string;
  budget: {
    maxSteps: 8;
    maxToolCalls: 4;
    maxSeconds: 120;
    maxCostCents: 0;
  };
  workbenchContext: {
    mode: "agent";
    documentIds: [];
    fileIds: [];
    skillCodes: [];
    mcpToolCodes: [];
    webSearchEnabled: true;
    routeId?: string;
  };
}
```

Use `NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID` as the default route, and support `NEXT_PUBLIC_AGENT_MODEL_ROUTE_OPTIONS` with the same parsing behavior as `apps/codex-app-poc`.

## Research Prompt Contract

The frontend converts the user's topic and preferences into a research-specific prompt. The prompt instructs the model to search when useful and return a report with these exact markdown headings:

- `## Research Overview`
- `## Active Topics`
- `## Key Authors And Institutions`
- `## Representative Work`
- `## Reading Route`
- `## Research Openings`
- `## Experiment Plans`
- `## Sources And Caveats`

Each section should be concise and should include source hints when available. The prompt should ask the model to call out uncertainty, stale information, and missing search coverage.

The UI parses these headings for section cards. If the final output does not match the expected headings, the UI shows the full raw final output in a general report section and still shows event evidence in the rail.

## Data Model

Frontend state is local to the app:

```ts
type ResearchScan = {
  id: string;
  topic: string;
  filters: ResearchFilter[];
  ranking: ResearchRanking;
  routeId: string;
  runResult: AgentRunResp | null;
  runEvents: AgentRunEventResp[];
  runError: string | null;
  createdAt: number;
};
```

Scans may be persisted in `localStorage` for a lightweight history. The first slice does not need backend persistence beyond normal Agent run persistence.

## Event And Evidence Rendering

Render events in user-facing language:

- `model_delta`: live or replayed assistant text.
- `model_inference`: route, provider, model, latency, and token metadata when available.
- tool calls and observations: tool name, arguments summary, status, and dry-run/unavailable signals.
- terminal run states: succeeded, failed, cancelled, or approval pause.

The evidence rail should never expose secrets or full raw payloads by default. A compact developer details expander can show safe JSON snippets for debugging.

## Error Handling

The app must cover:

- empty topic submission;
- backend unavailable;
- missing permission or auth failure;
- configured route missing;
- Agent run creation failure;
- event listing failure after successful run creation;
- web search dry-run or unavailable evidence;
- model output that does not match the expected markdown contract.

Errors should be visible next to the scan that caused them and should not erase previous successful scans.

## Architecture

Create a standalone Next.js app:

```text
apps/research-radar-poc/
  app/
    globals.css
    layout.tsx
    page.tsx
  src/
    api/
      agent.ts
      research.ts
    lib/
      api.ts
      auth.ts
      agent-events.ts
      research-report.ts
    types/
      agent.ts
      research.ts
```

The app should follow existing frontend patterns:

- Next.js app router.
- TypeScript.
- Tailwind CSS.
- Vitest and Testing Library.
- `lucide-react` for icons.
- Environment variable naming aligned with `apps/codex-app-poc`.

Avoid changing shared backend code unless tests reveal the existing Agent API cannot support the scan contract.

## Testing

Add focused tests for:

- home screen renders the research radar workbench;
- submitting a topic creates a model-loop Agent run with web search enabled;
- route selector sends the selected `modelRouteId`;
- expected markdown headings are parsed into report sections;
- malformed final output falls back to raw report rendering;
- event evidence rendering handles model deltas and tool observations;
- submission and backend errors remain visible without clearing prior scans.

## Verification Plan

Run:

```bash
pnpm --dir apps/research-radar-poc test
pnpm --dir apps/research-radar-poc typecheck
pnpm --dir apps/research-radar-poc lint
pnpm --dir apps/research-radar-poc build
git diff --check
```

For live validation, start the backend and run the app against `NEXT_PUBLIC_API_BASE_URL=http://localhost:62601`, then submit a real topic and confirm the Agent run reaches a terminal state with readable evidence.

## Non-Goals

- Dedicated backend research service.
- Persistent research projects stored in backend tables.
- Citation graph, PDF ingestion, or paper library management.
- Full scheduled monitoring or alerts.
- Multi-user collaboration.
- Claiming source freshness when web search is unavailable or dry-run.
- Replacing NotebookLM or the Codex-like agent workbench.

## Acceptance Criteria

1. A new `apps/research-radar-poc` app exists and can run independently.
2. The first screen is a functional research radar workspace.
3. A user can enter a research topic and create a real Novex Agent `model_loop` run.
4. The run command enables web search in `workbenchContext`.
5. The selected model route is included when configured or selected.
6. The UI renders structured report sections when the model follows the markdown contract.
7. The UI falls back gracefully when output is unstructured.
8. The evidence rail displays run id, status, model deltas, and tool/event evidence.
9. Previous scans remain visible when a later scan fails.
10. Tests cover the Agent command, report parsing, evidence rendering, and core UI states.
