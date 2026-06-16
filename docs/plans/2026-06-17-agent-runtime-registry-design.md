# Agent Runtime Registry Design

## Goal

Upgrade the previous DB-backed cancellation checkpoints into a real in-process runtime control boundary. Active model-loop runs should register a cancellation token, `POST /cancel` should signal that token, and active model/tool futures should be able to stop before their provider timeout elapses.

## Current State

Novex now has:

- Run Graph cancel states and cancel API.
- DB-backed model-loop cancellation checkpoints.
- timeout-driven cancelled tool observations.
- true parallel tool I/O with deterministic audit persistence.

The remaining gap is an in-memory active-run registry. `AgentService` is still constructed per request from `AppState.db`, so two HTTP requests do not share a runtime control handle unless `AppState` owns it.

## Options

### Option A: Process-global static registry

Add a `OnceLock<AgentRuntimeRegistry>` inside `AgentService`.

This is easy to wire but hides lifecycle and makes tests harder to reason about. It also gives future multi-tenant control-plane code no explicit dependency boundary.

### Option B: AppState-owned registry

Add `agent_runtime: AgentRuntimeRegistry` to `AppState`, pass it to `AgentService::for_tenant_with_runtime`, and keep `AgentService::for_tenant` as a compatibility constructor with an isolated default registry.

This is selected. It keeps runtime state explicit and shared across HTTP handlers without requiring a background worker rewrite.

### Option C: Full supervised background worker

Move agent execution out of the HTTP request into a worker registry with join handles, cancellation tokens, restart policy, and progress streaming.

This is the eventual enterprise shape, but it is too large for this slice. The registry/token type introduced here should be usable by that worker later.

## Selected Design

Add:

- `AgentRuntimeRegistry`: cloneable, process-local active-run map.
- `AgentRunCancellationToken`: cloneable watch receiver with `is_cancelled()` and `cancelled()` await boundary.
- `ActiveAgentRunGuard`: unregisters `(tenant_id, run_id)` on drop.

Runtime flow:

1. `create_model_loop_run` creates DB records.
2. It registers `(tenant_id, run_id)` in `AgentRuntimeRegistry`.
3. Every model/tool await is wrapped by cancellation-aware helpers.
4. `cancel_run` writes the existing persisted cancel events and calls `agent_runtime.cancel_run(tenant_id, run_id)`.
5. The active request observes the token and finishes through the existing external-cancel checkpoint path.

If a run is cancelled before the runtime registers, the DB checkpoint still catches it. If the process restarts, DB remains authoritative. The registry is a fast in-process signal, not the source of truth.

## Cancellation-Aware Await Boundaries

Use helper functions rather than spreading `tokio::select!` everywhere:

- `await_model_loop_future_or_cancelled(token, stage, future)`.
- `execute_agent_tool_io_with_timeout_and_cancel(prepared, token, execute)`.

For model calls, cancellation returns to the normal external-cancel finalization path.

For tool calls, cancellation returns an `AgentToolExecution::cancelled` with:

```json
{
  "status": "cancelled",
  "cancelReason": "external_cancel",
  "toolCode": "...",
  "callId": "...",
  "cancelStage": "tool_io"
}
```

Timeout remains `cancelReason=tool_io_timeout`.

## AppState Wiring

`AppState` gains:

- `agent_runtime: AgentRuntimeRegistry`

Agent HTTP handlers should construct services through:

- `AgentService::for_tenant_with_runtime(state.db, current_user.tenant_id, state.agent_runtime)`

Other AI handlers that only list/read agent runs can keep using default constructors only if they do not need runtime cancellation signaling. Direct agent create/resume/cancel handlers must use the shared registry.

## Non-Goals

- No background worker or join-handle lifecycle yet.
- No cross-process cancellation notification.
- No persistence schema change.
- No streaming/SSE token propagation.
- No UI change.

## Verification

- Registry unit test proves a registered token observes `cancel_run`.
- Source guard proves `AppState` owns and passes the runtime registry.
- Source guard proves model/tool futures are wrapped in token-aware helpers.
- Existing `external_cancel`, `tool_io_timeout`, `parallel_tool`, `model_loop`, and workspace tests remain green.

