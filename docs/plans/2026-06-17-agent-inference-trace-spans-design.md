# Agent Inference Trace Spans Design

## Goal

Make rollout trace/replay carry model inference evidence from the real agent loop: route, provider, model, latency, token usage, and an optional cost field. This gives enterprise eval and rollout gates enough evidence to reason about model behavior instead of only tool behavior.

## Current State

The model runtime already returns `ModelChatResp` with:

- `route_id`
- `model`
- `latency_ms`
- `usage`

`novex-model` already contains token usage normalization and cost estimation helpers. The backend agent model loop calls `chat_completion_for_purpose(ModelRoutePurpose::CodeAgent, ...)`, parses the answer, and records thought/tool/final events. The model response metadata is not recorded as an agent event, so `TraceBundle` and `novex-eval` cannot see inference latency, provider/model identity, or token usage.

`TraceEventKind` currently preserves runtime control flow: retrieval, action selection, context compaction, and cancellation. It does not yet have an inference event.

## Options

### Option A: Read model usage from `ai_model_usage`

Join usage records into rollout trace snapshots when building replay/eval data.

This can eventually provide authoritative DB cost, but it adds query coupling and is not available for the current `chat_completion_for_purpose` code path, which does not persist usage.

### Option B: Record inference metadata as a RunEvent payload

After each completed model call in the agent loop, append a `Thought` event with `item.type=model_inference` and a metadata-only payload. Convert that event to a dedicated `TraceEventKind::Inference`.

This is selected. It keeps trace conversion local, avoids schema changes, and makes replay/eval immediately useful. The event must not include the prompt or model answer content.

### Option C: Build full nested provider spans now

Introduce nested spans for request, provider response, retry, cost, and token accounting.

This is the Codex-shaped long-term direction, but it is too wide for this slice. The selected shape keeps payload fields compatible with a future nested span tree.

## Selected Design

Add `TraceEventKind::Inference` and `TraceEvent::inference(sequence_no, payload)`.

The backend maps any `thought` event whose payload has `item.type=model_inference` to `TraceEventKind::Inference`. Ordinary thought events continue to map to assistant messages.

The model loop records one inference event after a completed model call and before parsing the model output:

```json
{
  "runtimeMode": "model_loop",
  "item": {
    "type": "model_inference",
    "routeId": "runtime.llm.code_agent",
    "provider": "deep-seek",
    "model": "deepseek-v4-flash",
    "latencyMs": 42,
    "usage": {
      "promptTokens": 11,
      "completionTokens": 7,
      "totalTokens": 18
    },
    "costCents": null
  }
}
```

`ModelChatResp` should expose the selected provider so the agent loop does not need to re-resolve routes after the call. Cost stays optional in this slice: trace/eval can extract `costCents` when present, but must not invent cost when no cost record exists.

## Eval Impact

`EvalCaseCandidate::from_trace_bundle` should derive:

- `inferenceCount`
- `modelRouteId`
- `modelProvider`
- `modelName`
- `latencyMs`
- `promptTokens`
- `completionTokens`
- `totalTokens`
- `costCents` only when an inference payload provides it

For multiple inference spans, `latencyMs` is summed and token counts are summed. Route/provider/model tags use the first non-empty value so rollout filters stay stable.

## Non-Goals

- No DB migration.
- No model prompt or answer content in inference payloads.
- No full nested span tree yet.
- No retry or provider error span yet.
- No fabricated cost estimate when the selected route does not expose cost metadata.

## Verification

- `novex-trace` unit test proves inference events are preserved.
- backend unit test proves `thought` events with `item.type=model_inference` map to inference trace events.
- backend source-contract test proves the model loop records inference metadata after model calls.
- `novex-eval` unit test proves inference tags aggregate latency and usage.
- workspace offline tests remain green.
