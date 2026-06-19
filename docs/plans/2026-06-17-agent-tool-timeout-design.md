# Agent Tool Timeout Design

## Goal

Add the first runtime cancellation slice after parallel tool I/O: every prepared agent tool call has a runtime I/O timeout, timeout produces a cancelled tool execution, and backend records a cancelled observation/audit instead of hanging or collapsing the error into a generic failure.

This is not full cross-request cancellation token propagation yet. It is the timeout boundary that future cancel tokens can plug into.

## Current State

Novex now supports:

- `ToolConcurrencyPolicy.waits_for_runtime_cancellation` metadata.
- parsed tool-call batches.
- true parallel tool I/O with serial audit/step/event persistence.
- external HTTP client timeouts for Feishu, GitHub, media, model, and other provider calls.

The gap is that backend has no uniform tool I/O runtime timeout. A tool future that does not return has no runtime-level cancelled execution, no cancelled observation, and no stable audit status.

## Options

### Option A: Full Cancel Token Propagation

Wire a cancellation token from `POST /cancel` into in-flight model-loop tool tasks. This is the final direction, but current agent runs are synchronous request flows, not background workers with an in-memory run task registry. Doing this honestly needs a runtime task registry.

### Option B: Tool I/O Timeout Contract First

Add a runtime timeout around each prepared tool I/O future. When elapsed, return an `AgentToolExecution` with `status=cancelled`, `ToolObservationStatus::Cancelled`, and a structured payload with `cancelReason=tool_io_timeout`.

This is the selected slice because it creates the same persistence/event semantics that cancel tokens will use later.

### Option C: Rely on HTTP Client Timeouts

Keep provider-specific timeouts only. This leaves dry-run/custom/MCP futures without a uniform runtime guard and gives trace/eval no common cancellation shape.

## Selected Design

Backend adds:

- `AGENT_TOOL_IO_TIMEOUT`: conservative runtime default.
- `AgentToolExecution::cancelled`.
- `PreparedAgentToolCall.timeout`.
- timeout wrapping in `execute_agent_tool_io_batch`.

The helper preserves batch result order. In `Parallel` mode each future is independently timed out. In `Serial` mode each call also has its own timeout.

Model-loop observation mapping will distinguish:

- `succeeded` -> `ToolObservationStatus::Succeeded`
- `cancelled` -> `ToolObservationStatus::Cancelled`
- everything else -> `ToolObservationStatus::Failed`

## Non-Goals

- No in-memory run cancellation registry yet.
- No abort handle storage yet.
- No provider-specific timeout configuration UI yet.
- No automatic run termination on timeout yet; the model receives the cancelled observation and may explain or continue within budget.

## Testing

Backend tests:

- timeout wrapper converts a never-returning future into a cancelled execution.
- cancelled execution status maps to `ToolObservationStatus::Cancelled`.
- model-loop source guard confirms observation mapping handles cancelled status.

Acceptance:

- `cargo test -p backend tool_io_timeout --offline`
- `cargo test -p backend model_loop --offline`
- `cargo test --workspace --offline`
