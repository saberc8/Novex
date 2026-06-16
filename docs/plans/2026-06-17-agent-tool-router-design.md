# Agent Tool Router Design

## Goal

Move the first real slice of Codex-style tool routing into `crates/novex-tools`, so `runtimeMode=model_loop` no longer relies only on backend-local hardcoded tool code lists. This keeps Novex moving toward a shared Agent foundation where chat flow, customer service, knowledge, NotebookLM, MCP, and future sandbox tools all use the same model-visible registry contract.

## Current Gap

The runtime loop can now sample the configured `runtime.llm.code_agent` route, execute multiple tools, and compact context. But tool routing is still split:

- `backend/src/application/ai/agent_service.rs` builds the model prompt from `model_loop_tool_codes()`.
- The parsed tool call is dispatched by direct `find_tool_by_code` lookup.
- `crates/novex-tools` owns `ToolDefinition`, model-visible spec conversion, risk, and approval, but not the registry/router boundary.

Codex separates these concerns through a router/registry shape: the model sees a registry-derived tool set, model tool calls are converted into typed invocations, and dispatch happens under policy/runtime control.

## Options

### Option A: Keep Backend Hardcoding

This is the smallest change, but it keeps Agent runtime behavior trapped inside one service. It does not help NotebookLM, customer-service templates, MCP tools, or future sandbox tools share a contract.

### Option B: Add `novex-tools` Router Contract First

`novex-tools` owns a deterministic `ToolRouter` that is built from `ToolDefinition`s. It exposes model-visible specs, canonical tool codes, duplicate/unknown validation, and typed routed calls. Backend still performs actual execution, credentials, audit, and DB status transitions.

This is the recommended slice because it moves the architecture in Codex's direction without pretending to have the full executor registry yet.

### Option C: Full Executor Registry Now

This would move execution handlers, async dispatch, credential resolution, and audit into `novex-tools`. It is closer to Codex long term, but too wide for one safe slice because current execution depends on `AgentService`, repositories, tenant-bound model runtime, and media persistence.

## Selected Design

Implement Option B.

`crates/novex-tools` will add:

- `ToolRouter`
- `RoutedToolCall`
- `ToolRouteError`
- `agent_model_loop_tool_definitions()`

The router will:

- Reject empty and duplicate tool codes.
- Return sorted model-visible specs and tool codes.
- Reject model-requested tools outside the registered set before backend DB lookup.
- Preserve the selected `ToolDefinition` so policy metadata stays attached to the routed call.

Backend will:

- Build the model-loop prompt from `ToolRouter::tool_codes()`.
- Route every parsed model tool call through `ToolRouter::route_tool_call`.
- Use the routed call's canonical `tool.code` for DB lookup, policy, event payload, and execution.
- Keep actual tool execution in `AgentService` for this slice.

## Error Handling

If the model asks for an unregistered tool, backend should append a failed `ActionSelected` event with `stopReason: "unknown_tool"` and finish the run as failed. This is stricter than returning `NotFound`, because it preserves an auditable model mistake inside Run Graph.

## Testing

Runtime/tool crate:

- Router exposes model-visible specs from definitions.
- Router rejects duplicate tool codes.
- Router rejects unknown model-selected tools.
- Built-in agent model-loop definitions include `rag.search`, GitHub, media image, and Feishu contracts.

Backend:

- Source-level test verifies `AgentService` uses `ToolRouter`.
- Behavior-level helper test verifies model-loop prompt uses router-owned tool codes.
- Existing `model_loop`, `agent_service`, and workspace tests remain green.
