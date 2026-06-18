# Agent Tool I/O Observability Plan

## Goal

Make each Agent tool I/O task visible as trace and eval evidence. Parallel tool calls already run through an owned task supervisor; this slice adds per-task metrics so chat flow, POC runs, customer-service agents, enterprise knowledge-base flows, and NotebookLM-style workspaces can explain which tool task ran, how long it took, which batch mode supervised it, and whether cancellation or timeout happened at the tool I/O boundary.

## Architecture

Keep trace schema stable by enriching existing `observation` run-event payloads. The backend tool I/O runtime creates a typed metrics object for each `ExecutedAgentToolCall`; the model loop attaches it under `toolIoTask` on the tool observation payload; `novex-trace` preserves the payload as-is; `novex-eval` summarizes observation events into deterministic tags.

This follows the current event-evidence pattern used by runtime supervisor cancellation and inference spans: execution produces structured payload, trace replay keeps it, eval extracts compact tags.

## Scope

- Add a typed `AgentToolIoMetrics` contract carrying execution mode, task runtime, supervisor, batch index, started/finished timestamps, and duration.
- Ensure successful, failed, timeout, and external-cancel tool I/O outcomes include metrics.
- Attach metrics to model-loop tool observation events as `toolIoTask`.
- Add eval summary tags for tool I/O task counts, parallel/serial counts, cancellation count, timeout count, max duration, and supervisors.
- Preserve ordered parallel results, serial execution semantics, audit persistence, and existing timeout/cancel payloads.

## Out of Scope

- Adding a database table for per-task spans.
- Changing `novex-trace` event types or trace bundle serialization.
- Moving tool execution into a worker pool.
- Changing approval, Guardian review, tool routing, or concrete executor behavior.
- Exposing frontend charts for these metrics.

## RED Tests

- Runtime source-contract test requires `AgentToolIoMetrics`, `tool_io_metrics`, duration/timestamp fields, and `executionMode` payload serialization.
- Runtime behavior test verifies a successful parallel tool call receives task metrics with parallel mode, task supervisor, batch index, and non-negative duration.
- Runtime timeout/cancel tests verify cancelled executions still carry task metrics and preserve existing cancellation payload fields.
- Agent service source-contract test verifies model-loop observation payloads insert `toolIoTask` from executed call metrics.
- Trace conversion test verifies observation payloads preserve `toolIoTask` through `TraceBundle`.
- Eval test verifies `EvalCaseCandidate::from_trace_bundle` extracts tool I/O task tags from observation events.

## Implementation Steps

1. Add RED tests in backend runtime, agent service trace conversion, and `novex-eval`.
2. Introduce `AgentToolIoMetrics` and helper constructors/payload serialization.
3. Wrap `execute_agent_tool_io_with_timeout_and_cancel` with timing and attach metrics to every `ExecutedAgentToolCall`.
4. Add `toolIoTask` to model-loop observation payloads.
5. Add eval summary extraction for tool I/O observations.
6. Verify focused tests and full workspace, merge to `main`, run `cargo clean`, and remove this worktree branch.

## Verification

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test -p backend-rust agent_tool_io_runtime --offline`
- `cargo test -p backend-rust tool_io_observability --offline`
- `cargo test -p backend-rust parallel_tool_io_batch --offline`
- `cargo test -p backend-rust runtime_registry --offline`
- `cargo test -p backend-rust model_loop --offline`
- `cargo test -p novex-eval tool_io --offline`
- `cargo test --workspace --offline`
