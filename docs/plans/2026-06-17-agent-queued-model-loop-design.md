# Agent Queued Model Loop Design

## Goal

Enable `executionMode=queued` for `runtimeMode=model_loop` without creating a second run. A queued model-loop run should be created quickly, followed through the existing SSE event stream, claimed by the background worker, marked running, and executed by the same model-loop runtime used by inline requests.

## Current State

Novex already has:

- `executionMode=queued` command normalization.
- `ai_agent_run_queue` with claim/lease/retry/failure state.
- An embedded Agent queue worker.
- Shared HTTP/worker `AgentRuntimeRegistry`.
- Deterministic existing-run execution via `execute_queued_run`.
- Inline `create_model_loop_run` with the full Codex-style model loop.

The gap is that queued model-loop creation and execution are explicitly rejected. The model-loop body is still coupled to run creation.

## Approaches Considered

### Approach A: Duplicate model-loop logic in the worker

This is rejected. It would drift from inline behavior, make Guardian/cancellation/compaction fixes land twice, and violate the Codex-style single runtime contract.

### Approach B: Add a worker-only model-loop path with reduced features

This is also rejected. It would make queued runs a degraded mode and would hide gaps until production workflows depend on them.

### Approach C: Extract existing-run model-loop execution

Selected. `create_model_loop_run` remains the inline creator, but after it creates Run Graph records it calls `execute_model_loop_existing_run`. The queue worker calls the same function after changing a queued run to running. The helper owns runtime registry registration, model sampling, tool calls, Guardian review, compaction, cancellation checkpoints, trace refresh, and final status.

## Runtime Flow

Inline model-loop:

1. Normalize command.
2. Build model-loop plan and create Run Graph records as `running`.
3. Call `execute_model_loop_existing_run(..., record_input_event=true)`.
4. Emit the running `input_received` event and run the loop.

Queued model-loop:

1. Normalize command.
2. Build model-loop plan and create Run Graph records as `queued`.
3. Emit queued `input_received` and `status_changed` events.
4. Enqueue command payload.
5. Worker claims queue row, marks run `running`, emits running `status_changed`.
6. Worker calls `execute_model_loop_existing_run(..., record_input_event=false)`.
7. Queue row is marked `succeeded`, `retrying`, `failed`, or `cancelled`.

The queued path does not duplicate `input_received`; the durable queued event remains the creation evidence.

## Error Handling

- Terminal or approval-waiting runs remain idempotent: `execute_queued_run` returns the existing run and the queue can be completed.
- Queue retry behavior stays in `agent_queue_runtime.rs`.
- Model provider failures continue to use the existing model-loop retry policy and terminal failure events.
- Cancellation uses the shared `AgentRuntimeRegistry`, so in-process HTTP cancel can interrupt a worker-owned model loop.

## Acceptance

- `runtimeMode=model_loop + executionMode=queued` no longer returns the current unsupported error.
- `create_model_loop_run` and worker execution share `execute_model_loop_existing_run`.
- Inline model-loop still records a running input event.
- Queued model-loop records only queued input at creation, then running status when claimed.
- Worker calls `execute_queued_run`, which dispatches model-loop commands to the shared model-loop executor.
- Existing model-loop tests continue to pass.
- Agent queue tests prove model-loop support is wired and no second run is created.
