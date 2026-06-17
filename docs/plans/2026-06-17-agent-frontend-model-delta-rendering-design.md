# Agent Frontend Model Delta Rendering Design

## Goal

Render persisted CodeAgent `model_delta` run events as user-visible streaming model output in the Agent workspace and Codex-style POC.

## Current State

The backend can parse provider token deltas, persist them as durable run events with `payload.item.type = "model_delta"`, and preserve them in trace/eval. Frontend clients can list or stream run events, but the workspace only renders raw JSON event payloads and the Codex POC only shows the final run response. A real agent-loop POC should make partial model output visible without asking users to inspect JSON.

## Selected Approach

Add a small presentation helper per frontend app that:

- Accepts ordered or unordered run events.
- Extracts events whose payload has either `item.type = "model_delta"` or direct `type = "model_delta"`.
- Sorts chunks by numeric `deltaIndex` when available, then by `sequenceNo`.
- Concatenates raw `content` without trimming so token whitespace is preserved.
- Returns chunk count and optional route/provider/model metadata.

The Agent workspace will show a compact "Live model output" panel above the event list when delta chunks exist. The Codex POC will fetch the first page of run events after run creation and render the same live-output panel below the run status.

## Alternatives Considered

1. Render each `model_delta` event as a separate workflow card only.
   This preserves event granularity, but it does not recreate the model output users expect to read.

2. Stream directly over WebSocket in this slice.
   This is closer to the final experience, but the durable event listing path already exists and is easier to verify. WebSocket live subscription can build on the same extraction helper later.

3. Fold deltas into `finalOutput`.
   This would blur partial provider output with the authoritative terminal answer. The UI should present deltas as live model output evidence, not final answer truth.

## Scope

This slice covers frontend extraction and display only. It does not change backend event schemas, SSE/WebSocket transport, trace/eval aggregation, or provider streaming parsers.

## Testing

Frontend tests must prove:

- The helper preserves whitespace across chunks and orders by `deltaIndex`.
- Non-delta events are ignored.
- Agent workspace renders aggregated live model output from listed run events.
- Codex POC submits the configured model-loop run, fetches event evidence, and renders aggregated live model output.
- Existing API, page, and typecheck gates continue to pass.
