# Agent POC Configured Model Route Design

## Goal

Let the Codex-style POC and backend Agent loop run against a specific configured model route without bypassing Novex model governance. The default remains the tenant/environment `CodeAgent` route, while demos and operators may provide `modelRouteId` such as `runtime.llm` or a tenant registry route code.

## Current State

`runtimeMode=model_loop` already calls `ModelRuntimeService::chat_completion_for_purpose(ModelRoutePurpose::CodeAgent, ...)`, so live model execution uses the configured model runtime. The missing control is explicit route selection from the Agent request. The POC only sends `runtimeMode=model_loop`; it cannot pin a route for real-model demonstrations, and backend tests only assert the purpose, not the request-level route contract.

## Selected Approach

Add an optional `modelRouteId` field to `AgentRunCommand`.

- Normalize it with the same bounded string rules used by chat flow and RAG model route selectors.
- Store it inside queued command payloads through the existing serialized command path.
- Pass it into `ModelChatCommand.route_id` for every CodeAgent model call and model-assisted compaction call that belongs to the same run command.
- Keep `None` as the default so existing inline and queued runs continue to resolve the default configured `CodeAgent` route.
- Teach `apps/codex-app-poc` to read `NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID` and include it in create-run requests when configured.

## Data Flow

POC submit:

1. User submits the composer.
2. `createConfiguredModelAgentRun` builds `{ runtimeMode: "model_loop", modelRouteId?, budget, autoApprove }`.
3. Backend normalizes the command.
4. Agent loop resolves `ModelRoutePurpose::CodeAgent` with the optional route id.
5. Model inference events continue to record the actual selected `routeId`, provider, model, latency, usage, cost, and provider attempts.

Queued submit:

1. The normalized command, including `modelRouteId`, is serialized into `ai_agent_run_queue.payload`.
2. The worker deserializes the same command and calls the shared existing-run model-loop executor.
3. The model loop uses the same route selection path as inline execution.

## Error Handling

If a provided `modelRouteId` does not resolve to an enabled `CodeAgent` route, `ModelRuntimeService` returns the existing `选择的模型路由不可用` bad request. If no route id is provided and no configured `CodeAgent` route exists, the existing `LLM 模型环境变量未配置完整` error remains.

## Acceptance

- `AgentRunCommand` accepts and trims `modelRouteId`.
- Overlong route ids are rejected before model execution.
- Model-loop calls include `ModelChatCommand { route_id: command.model_route_id.clone(), ... }`.
- POC API tests prove configured route ids are included when present and omitted when absent.
- Existing inline and queued model-loop tests remain green.
