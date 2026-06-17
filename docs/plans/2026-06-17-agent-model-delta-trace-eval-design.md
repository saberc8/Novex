# Agent Model Delta Trace/Eval Design

## Goal

Preserve CodeAgent provider token deltas beyond the live run-event stream so rollout trace replay and eval candidate capture can audit streaming behavior.

## Current State

CodeAgent model-loop calls can now parse chat-completions SSE token deltas and persist each chunk as a durable `ai_run_event` payload with `item.type = "model_delta"`. The existing trace conversion still only treats `model_inference` and `model_inference_error` thought items as inference trace events. That means live clients can receive deltas, but trace replay and eval tags do not yet prove that streaming happened.

## Selected Approach

Treat `model_delta` as an inference trace item. This keeps all model-provider evidence under the existing `TraceEventKind::Inference` category and avoids introducing a new trace enum variant or schema changes. Eval candidate extraction will count delta trace events and tag:

- `modelDeltaCount`
- `modelDeltaTextLength`
- `streamingModelOutput`

The final `model_inference` event remains the authoritative latency/usage/cost summary. Delta events provide streaming observability and text-length evidence without becoming answer assertions.

## Alternatives Considered

1. Add a new `TraceEventKind::ModelDelta`.
   This is more explicit, but it would force broader trace/eval/UI contract churn for a payload type that is still model-provider evidence.

2. Ignore deltas in trace and rely on run-event replay only.
   This keeps the code unchanged, but eval and rollout would miss streaming proof.

3. Include deltas as assistant messages.
   This would pollute transcript semantics because partial provider output is not necessarily a stable assistant message or final answer.

## Scope

This slice covers backend trace conversion and eval tags only. It does not change WebSocket/SSE transport, UI rendering, partial tool-call parsing, or stream-native model runtime APIs.

## Testing

Backend tests must prove:

- `model_delta` thought events convert to `TraceEventKind::Inference`.
- Trace payload preserves `content`, `deltaIndex`, provider metadata, and source.
- Eval candidate tags count streaming delta events and aggregate delta text length.
- Existing `model_inference` and `model_inference_error` trace behavior remains intact.
