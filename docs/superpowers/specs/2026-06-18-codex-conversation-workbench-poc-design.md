# Codex Conversation Workbench POC Design

## Brief

Upgrade `apps/codex-app-poc` from a Codex-like static shell into a real conversational workbench for Novex agent infrastructure. The first screen remains the Codex-style chat page, but the composer and context surfaces become functional: direct questions, file upload and parsing, file-grounded answers, skills, MCP tools, web search, live run events, and trace evidence all flow through one POC.

This POC is a product-facing validation layer for the broader agent foundation. It should prove that the existing configured model route, agent loop, tool router, RAG, capability registry, MCP gateway, and trace/eval events can support future chat flow, intelligent customer service, enterprise knowledge base, and NotebookLM-like workflows.

## Current State

- `apps/codex-app-poc` already matches the Codex desktop conversation visual direction and can create configured model agent runs through `/ai/agents/runs`.
- `apps/codex-app-poc` already reads agent run events and summarizes model delta events.
- `apps/chat-web` already has API clients for knowledge datasets, document upload, RAG ask, model chat, skills, and capability data.
- Backend already exposes agent run CRUD/events/WebSocket ticket routes, knowledge dataset/file upload/parse job/RAG ask routes, skill listing/import routes, MCP server/tool routes, and model route configuration.
- `novex-tools` already defines `rag.search` with `datasetId`, and the model-loop prompt can request tool calls by tool code.
- MCP currently supports registration, discovery, Streamable HTTP live execution, OAuth callback/refresh, and refresh scheduling. Stdio supervision remains in a separate unfinished worktree and is not required for this POC slice.

## Design Choice

Use the existing `apps/codex-app-poc` app rather than creating a new app.

This keeps the visual target stable, avoids duplicating the already approved Codex-like workbench shell, and turns the existing POC into the canonical end-to-end agent foundation entry point. New code should stay focused inside `apps/codex-app-poc` with only thin backend additions when the current API cannot represent workbench context safely.

## Product Shape

The workbench has three visible zones:

1. Left sidebar: existing Codex-like navigation, projects, and sessions remain. It can show lightweight POC status for the active workspace and recent runs.
2. Main conversation: composer, message/result stream, run status, model deltas, tool events, final answer, citations, and errors.
3. Context drawer: selected files, dataset, skills, MCP tools, web search toggle, route/runtime status, and trace link.

The first viewport should still feel like a conversation app, not an admin dashboard. Advanced controls are available as compact toggles, chips, popovers, and a right-side context drawer.

## Workbench Context Contract

The UI builds a typed workbench context before starting a run:

```ts
type WorkbenchContext = {
  mode: "agent";
  datasetId?: number;
  documentIds: number[];
  fileIds: number[];
  skillCodes: string[];
  mcpToolCodes: string[];
  webSearchEnabled: boolean;
  routeId?: string;
};
```

For the first POC, the frontend can submit this context through a new optional `workbenchContext` field on `AgentRunCommand`. Backend normalization should keep the field bounded and persist it into run metadata/output payloads. The model-loop prompt should use it to:

- expose selected MCP tool codes when available;
- bias `rag.search` calls with the selected `datasetId`;
- include selected skill names/codes as operating context;
- include `web.search` only when the web-search capability is implemented or registered.

If a backend slice cannot be completed in the first implementation pass, the UI must show a clear POC fallback badge and keep the unavailable capability disabled rather than silently pretending it ran.

## File Flow

The attachment button opens a file picker. Upload uses the existing knowledge API:

1. Ensure or select a dataset for the workbench. The default POC dataset name is `Codex Workbench Inbox`.
2. Upload files with `POST /ai/knowledge/datasets/:datasetId/documents/files`.
3. Show each file as a chip with upload, parse, indexed, failed, or unavailable state.
4. Poll `GET /ai/knowledge/datasets/:datasetId/parse-jobs/:jobId` until terminal state or timeout.
5. When the user asks a question, include `datasetId` in `workbenchContext` so the agent can call `rag.search`.

The POC must also support asking before parsing completes. In that case the run can start, but the context drawer must show that file-grounded retrieval may be incomplete.

## Direct Conversation Flow

All sends use the configured model agent path by default:

- `runtimeMode: "model_loop"`
- `modelRouteId` from `NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID` when configured
- bounded budget suitable for UI testing
- `workbenchContext` attached

If no file, skill, MCP tool, or web search is selected, the agent may answer directly. This keeps the workbench useful for ordinary questions while still exercising the model-loop event stream.

## Skills Flow

The context drawer lists skills from `/ai/capabilities/skills`. Users can select one or more skills as chips.

First POC behavior:

- selected skills are passed as `skillCodes` in `workbenchContext`;
- selected skill names/codes are included in the run prompt/context;
- the event stream surfaces skill context in run metadata;
- importing or editing skills remains outside this POC.

This validates the skill registry as agent context without forcing full Codex skill execution semantics into the first UI slice.

## MCP Flow

The context drawer lists MCP servers/tools from the capability API. Users can select discovered tools that are active.

First POC behavior:

- selected tool codes are passed as `mcpToolCodes`;
- selected MCP tool codes are included in the model-visible available tool set;
- live MCP execution follows existing backend gating and metadata;
- OAuth refresh state is visible only as capability status, not as a browser OAuth callback UX in this slice.

Stdio MCP process supervision is a separate follow-up integration after the existing `mcp-stdio-process-supervisor` worktree is completed and merged.

## Web Search Flow

The UI includes a web search toggle backed by a POC `web.search` tool.

First backend slice:

- add a low-risk `web.search` tool definition in `novex-tools`;
- add a backend executor that supports a configured provider through environment variables;
- when no provider is configured, return a safe dry-run response that explains web search is unavailable.

The UI must distinguish `enabled`, `dryRun`, and `unavailable` states in the event stream.

## Event Rendering

The workbench should render run events as user-facing conversation evidence:

- `model_delta`: live assistant text.
- `model_inference`: route/provider/model/latency/usage metadata.
- `tool_called` and tool batch events: tool name, arguments summary, status.
- `retrieval`: dataset, hit count, citations when available.
- MCP execution payloads: server/tool/live/mock/dry-run status without secrets.
- file parse state: upload and parse job progress.
- terminal events: succeeded, failed, cancelled, waiting approval.

Raw JSON remains available in a collapsible developer drawer, but the default view should be readable by a product user.

## API Surface

Frontend additions should live under `apps/codex-app-poc/src/api`:

- `knowledge.ts`: dataset list/create, file upload, parse job polling, optional RAG ask helper.
- `capability.ts`: skills, MCP servers, MCP tools.
- `workbench.ts`: compose run command, normalize event summaries, capability status.

Backend additions should be minimal:

- extend `AgentRunCommand` with optional `workbenchContext`;
- normalize and persist safe context metadata;
- thread selected dataset/tool/skill/web-search context into model-loop prompt and tool routing;
- add a low-risk `web.search` POC tool with safe dry-run behavior when no provider is configured.

No new app is created.

## UX States

The first implementation must cover:

- empty state with suggested tasks;
- typing and sending;
- running state with streaming/delta output;
- file upload progress;
- parse pending/indexed/failed states;
- selected skill/MCP/search chips;
- final answer with citation/tool evidence;
- backend unavailable fallback;
- missing permission/auth error;
- capability unavailable state.

## Non-Goals

- Full NotebookLM product workflow.
- Full customer-service production console.
- Skill authoring/import UI.
- MCP OAuth browser callback UX.
- Stdio MCP process execution in this POC branch.
- Production web search vendor abstraction.
- Multi-user session collaboration.
- Full eval dataset authoring UI.

## Acceptance Criteria

1. `apps/codex-app-poc` remains visually close to the existing Codex-like page and opens directly into the conversation workbench.
2. A user can submit a direct question and see a live or replayed agent run result from the configured model path.
3. A user can upload a file, see parse status, and submit a question with a selected dataset context.
4. The run command includes typed workbench context instead of only prompt text.
5. Skills can be listed and selected.
6. MCP servers/tools can be listed and selected.
7. Web search is represented by a toggle with honest enabled/dry-run/unavailable status.
8. Run events are rendered as readable conversation evidence, not only raw JSON.
9. Tests cover API clients, context normalization, event summary rendering, and key UI states.
10. The migration matrix records this POC as the product-facing validation layer for the agent foundation.

## Verification Plan

- `pnpm --dir apps/codex-app-poc test`
- `pnpm --dir apps/codex-app-poc typecheck`
- `pnpm --dir apps/codex-app-poc lint`
- backend focused tests for `workbenchContext` normalization and model-loop prompt/tool context
- backend focused tests for `web.search` if implemented in this slice
- `cargo fmt --all -- --check`
- `git diff --check`

## Follow-Up Foundation Work

- Complete and merge MCP stdio process supervision.
- Promote workbench context into a shared agent protocol crate when stable.
- Add eval cases for file-grounded answer quality, MCP tool success/failure, web search dry-run, and direct answer behavior.
- Add rollout replay views that compare direct answer, RAG answer, MCP-assisted answer, and web-search-assisted answer.
- Add NotebookLM workspace templates on top of the same workbench context contract.
- Add customer-service template that preselects FAQ dataset, customer tools, and handoff tools.
