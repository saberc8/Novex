# Agent Runtime Trace Spans Design

## Goal

Make rollout trace/replay useful for enterprise eval by preserving runtime control-flow evidence that currently exists in `ai_run_event` but is dropped when converted to `TraceBundle`.

## Current State

`novex-trace` currently models:

- user message,
- assistant message,
- tool call,
- observation,
- approval requested,
- final answer,
- error.

The backend already records richer agent events:

- retrieval context,
- action selected with tool batch, concurrency policy, and serial reason,
- context compaction observations,
- cancel requested and cancelled,
- runtime token cancellation payloads,
- timeout-driven cancelled tool observations.

The conversion `agent_events_to_trace_bundle` does not preserve several of those events. That weakens trace replay and makes eval unable to distinguish cancelled runs, compaction behavior, and batch planning evidence from ordinary tool observations.

## Options

### Option A: Store raw RunEvent records only

Keep `TraceBundle` small and rely on `event_snapshot` for detailed UI/debug data.

This keeps compatibility but makes `novex-eval` depend on backend-specific event rows. That conflicts with the intended crate boundary: eval should consume `novex-trace`.

### Option B: Add generic runtime span events

Add trace event kinds for retrieval, action selection, context compaction, and cancellation. Preserve payloads from the backend event, normalized enough to replay and score.

This is selected. It is a small adapter-port of Codex rollout trace ideas into Novex's current event model.

### Option C: Build full Codex rollout-trace schema now

Introduce nested spans, inference metadata, token/cost accounting, MCP spans, and tool dispatch timing in one pass.

This is the long-term direction, but it is too wide for one safe slice. Runtime trace spans can be extended later with timing/cost fields.

## Selected Design

Extend `TraceEventKind` with:

- `Retrieval`
- `ActionSelected`
- `ContextCompaction`
- `Cancellation`

Add constructors:

- `TraceEvent::retrieval(sequence_no, payload)`
- `TraceEvent::action_selected(sequence_no, payload)`
- `TraceEvent::context_compaction(sequence_no, payload)`
- `TraceEvent::cancellation(sequence_no, payload)`

Backend conversion rules:

- `retrieval` -> `TraceEventKind::Retrieval`
- `action_selected` -> `TraceEventKind::ActionSelected`
- `observation` with `item.type=context_compaction` -> `TraceEventKind::ContextCompaction`
- `cancel_requested` and `cancelled` -> `TraceEventKind::Cancellation`
- existing mappings remain unchanged.

Payload should stay close to the backend event payload so replay/eval can inspect:

- `toolCallBatch`
- `batchExecutionMode`
- `concurrencyPolicy`
- `cancelReason`
- `cancelStage`
- compaction counts and summaries
- retrieval hit count and source

## Eval Impact

`EvalCaseCandidate::from_trace_bundle` should add optional tags:

- `cancelled`
- `cancelReason`
- `compactionCount`
- `retrievalCount`

These tags let customer service and NotebookLM eval suites separate user/operator cancellation from model failure, and assert whether long-document runs used compaction/retrieval as expected.

## Non-Goals

- No DB schema migration.
- No latency/cost computation yet.
- No provider token accounting in trace spans yet.
- No full nested span tree yet.
- No UI changes.

## Verification

- `novex-trace` unit test covers new event kinds and summary cancellation status.
- backend unit test proves run events convert retrieval/action/compaction/cancellation to trace events.
- `novex-eval` unit test proves trace candidates expose cancellation/compaction/retrieval tags.
- existing rollout/eval/model-loop tests remain green.

