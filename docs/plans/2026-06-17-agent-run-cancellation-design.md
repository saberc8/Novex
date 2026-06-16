# Agent Run Cancellation Design

## Goal

Move Novex one step closer to Codex-style runtime control by making external run cancellation observable inside the model-loop execution path. A cancellation request should not only update the database after the fact; the active loop must have a stable checkpoint contract that can stop future model/tool work and write a cancelled trace shape.

## Current State

Novex already has:

- `POST /ai/agents/runs/:run_id/cancel`.
- Run Graph statuses for `running -> cancelling -> cancelled`.
- `CancelRequested` and `Cancelled` events.
- timeout-driven cancelled tool observations for local tool I/O futures.

The gap is runtime propagation. `cancel_run` currently writes terminal status, but the synchronous `create_model_loop_run` request path does not check persisted cancellation state before starting the next model call, before tool routing, or before tool execution. If cancellation arrives while a model-loop request is still active, the loop can keep doing work until the request naturally returns.

## Options

### Option A: Full in-memory task registry and cancellation token

Register every active run in an in-memory registry, attach a token to model/tool futures, and have `cancel_run` signal the token.

This is the final shape for background workers, streaming responses, and long connector calls. It is too wide for the next safe slice because current agent runs are synchronous request flows, and adding a registry without moving execution to supervised background tasks creates lifecycle ambiguity.

### Option B: DB-backed cancellation checkpoints

Add a small checkpoint helper that reads the current run status from persistence and returns a cancellation outcome when the status is `cancelling` or `cancelled`. Call it before each model call, before executing a tool batch, and after tool execution returns. When tripped, the loop writes or preserves a cancelled terminal state and refreshes rollout/trace.

This is the selected slice. It makes external cancellation semantically real for the active loop while keeping the current synchronous architecture. A future task registry can replace or augment the checkpoint source without changing event semantics.

### Option C: Keep cancellation API persistence-only

Leave `cancel_run` as a pure state update and rely on timeout to stop hung tools. This avoids code churn but keeps the runtime blind to user cancellation and makes eval/trace evidence misleading.

## Selected Design

Add a private runtime checkpoint boundary in `AgentService`:

- `agent_run_cancel_checkpoint(user_id, run_id, stage)` reads the persisted run.
- If status is `cancelling` or `cancelled`, it returns a cancellation decision.
- If status is active, it returns continue.
- If status is terminal failed/succeeded, the loop stops with conflict-safe cancellation semantics only when the terminal state is already `cancelled`.

When the model loop observes cancellation:

- Final run status is `cancelled`.
- Output payload includes `cancelled=true`, `cancelReason=external_cancel`, `runtimeMode=model_loop`, and the checkpoint stage.
- Event stream includes `Cancelled` if the cancel API has not already written it.
- Trace snapshot is refreshed with the same cancellation metadata.

The first slice does not interrupt an already awaited provider future. It prevents subsequent model/tool work and gives the current request path a stable cancellation contract. Provider-level aborts and background task tokens remain next.

## Checkpoint Stages

Use stable, event-friendly stage names:

- `before_model_call`
- `after_model_call`
- `before_tool_batch`
- `after_tool_batch`
- `before_next_turn`

The model loop only needs a few checkpoints. They should sit at boundaries where the runtime can safely return a terminal response without leaving partial audit records.

## Trace, Eval, And Rollout Impact

Enterprise evals need to distinguish:

- failed model/tool behavior,
- user-requested cancellation,
- runtime timeout cancellation,
- approval pauses.

This slice gives trace replay and eval a stable external cancellation shape. Customer service and NotebookLM-style runs can be cancelled by operators without polluting failure-rate metrics.

## Non-Goals

- No supervised background task registry yet.
- No cross-request `CancellationToken` yet.
- No provider HTTP request abort yet.
- No websocket/SSE cancellation streaming change yet.
- No UI changes in this slice.

## Verification

- A RED/GREEN unit test proves the model-loop source checks cancellation before model calls.
- A unit test proves cancellation payload includes `cancelReason=external_cancel` and checkpoint stage.
- Existing `agent_service`, `model_loop`, and workspace tests remain green.

