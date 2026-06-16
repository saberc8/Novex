# Agent Parallel Executor Design

## Goal

Turn `ToolBatchPlan::Parallel` from event metadata into real concurrent tool I/O while preserving enterprise audit ordering. This closes the next Codex parallel-runtime gap without weakening approval, trace, or Run Graph consistency.

## Current State

Novex now supports:

- `ToolConcurrencyPolicy` with shared/exclusive lock semantics.
- `ToolBatchPlan` for `parallel` or `serial`.
- model output batches through `{"type":"tool_calls","calls":[...]}`.
- backend `ActionSelected` events with `batchExecutionMode` and `toolCallBatch`.

The gap is execution: even `parallel` batches still run one tool at a time.

## Constraint

`execute_and_record_tool_call` currently performs both external tool I/O and persistence:

- credential/MCP lookup.
- connector/model/tool execution.
- `ai_tool_call_audit` insert.
- media result persistence.
- run step insert with `next_event_sequence`.

Running that whole function concurrently would make step sequence numbers race because `next_event_sequence` is `MAX(sequence_no) + 1`. It could also blur audit ordering.

## Selected Design

Split execution into two phases:

1. **I/O phase:** execute safe prepared tool calls concurrently when `ToolBatchExecutionMode::Parallel`.
2. **Record phase:** write audit, media records, run steps, `ToolCalled`, and `Observation` events serially in original batch order.

This makes parallel batches faster for read-only/shared tools while preserving deterministic persisted traces.

## Components

Backend adds:

- `PreparedAgentToolCall`: batch index, call id, tenant tool record, and arguments.
- `ExecutedAgentToolCall`: prepared call plus in-memory execution result.
- `execute_agent_tool_io_batch`: small async helper that runs prepared calls either via `join_all` or serially.
- `execute_agent_tool_io`: loads credentials/MCP metadata and runs the actual tool without writing audit/step/event records.
- `record_agent_tool_execution`: persists a completed execution in deterministic order.

Existing `execute_and_record_tool_call` remains as a compatibility wrapper for non-batch paths.

## Approval Safety

Before any batch execution, backend must load every tenant tool record and evaluate approval policy. If any call requires approval, the model loop pauses before running the batch. This prevents partial side effects before human review.

`ToolBatchPlan::Parallel` already only happens when every routed call opts into shared/parallel execution. Mutating or exclusive tools stay serial.

## Non-Goals

- No runtime cancellation token propagation in this slice.
- No per-tool timeout override in this slice.
- No parallel persistence writes.
- No parallel execution for exclusive/mutating tools.

## Testing

Backend unit tests:

- parallel I/O helper uses concurrent polling and preserves result order.
- serial I/O helper executes calls in sequence.
- source-level guard confirms model loop prepares approval decisions before executing a batch.
- source-level guard confirms record/persistence is separated from parallel I/O.

Acceptance:

- `cargo test -p backend-rust parallel_tool --offline`
- `cargo test -p backend-rust model_loop --offline`
- `cargo test --workspace --offline`
