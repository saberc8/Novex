# Agent Provider Error Trace Design

## Context

The agent model loop now records successful model inference spans with route, provider, model, latency, token usage, and DB-derived cost. The rollout matrix still calls out provider retry/error spans as the next trace gap. Codex keeps provider calls as first-class rollout evidence: attempts are started, terminal completion/failure/cancellation is recorded, and the reducer can reason about incomplete or failed inference calls.

Novex should keep that engineering shape, but adapt it to the existing enterprise control plane. The backend already routes Code Agent calls through `ModelRuntimeService::chat_completion_for_purpose(ModelRoutePurpose::CodeAgent, ...)`, persists Run Graph events in PostgreSQL, converts those events to `TraceBundle`, and derives eval tags from inference spans.

## Approaches

1. Add a full Codex-style inference lifecycle crate now.
   - Pros: closest long-term shape to Codex.
   - Cons: requires request/response payload storage, inference ids, nested reducer work, and larger model service signature changes.

2. Change all model APIs to return a structured provider error type.
   - Pros: precise route/provider/http metadata for every caller.
   - Cons: touches chat flow, RAG, integration API, studio, and tests; too wide for this slice.

3. Record provider failure evidence at the Agent loop boundary.
   - Pros: gives rollout/eval immediate failure visibility, avoids broad API churn, and preserves a future upgrade path to full attempt lifecycle.
   - Cons: terminal failures only know the requested Code Agent route unless the model service returns a response.

Recommended: approach 3 for this slice. It turns model-call failures into a structured inference trace event now, while leaving provider-native retry/fallback as a follow-up that can enrich the same event contract.

## Design

Successful calls continue to emit:

```json
{
  "runtimeMode": "model_loop",
  "item": {
    "type": "model_inference",
    "routeId": "runtime.llm.code_agent",
    "provider": "deep-seek",
    "model": "deepseek-v4-flash",
    "latencyMs": 42,
    "usage": {},
    "costCents": 0.65
  }
}
```

Failed model calls emit a `thought` Run Event with an inference payload:

```json
{
  "runtimeMode": "model_loop",
  "item": {
    "type": "model_inference_error",
    "routeId": "runtime.llm.code_agent",
    "routePurpose": "code_agent",
    "attempt": 1,
    "maxAttempts": 1,
    "retryable": false,
    "errorKind": "provider_http",
    "httpStatus": 502,
    "message": "LLM model call failed: HTTP 502",
    "latencyMs": 1200
  }
}
```

The event remains `TraceEventKind::Inference` because it is part of the inference lifecycle, not a generic application error. The Agent loop also appends a normal `RunEventKind::Error` before finishing the run as failed so `TraceBundle::replay_summary()` reports `failed`.

`novex-eval` extends inference aggregation with:

- `inferenceErrorCount`
- `retryableInferenceErrorCount`
- `modelErrorKind`
- `modelHttpStatus`

The existing latency/cost tags keep their current behavior. Error inference spans contribute latency, but not token usage or cost.

## Error Classification

This slice classifies AppError values without exposing internal details:

- `provider_http` when the message contains `HTTP <status>`.
- `provider_timeout` when the message mentions timeout.
- `provider_transport` for internal/transport errors.
- `invalid_model_request` for bad request model output/request failures without an HTTP status.
- `not_found`, `conflict`, `unauthorized`, `forbidden` for matching application errors.

HTTP `429` and `5xx` are marked retryable, matching Codex's retry policy direction. Timeout and transport errors are retryable. Other client errors are not retryable.

## Acceptance

- Backend Agent loop records a structured inference error payload when the model call fails.
- Agent run trace conversion maps `model_inference_error` payloads to `TraceEventKind::Inference`.
- Trace replay summary is failed when the model call fails.
- Eval candidate tags expose provider error counts and first error classification.
- Existing successful inference span behavior remains unchanged.

## Follow-Up

- Move retry policy into `ModelRuntimeService` using route policy `maxRetries`.
- Record every provider attempt, not only terminal Agent-loop failures.
- Add route fallback evidence when `fallbackEnabled` is allowed by tenant policy.
- Split full inference lifecycle into a `novex-trace` reducer closer to Codex rollout trace.
