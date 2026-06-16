# Agent Provider Retry Trace Design

## Context

The Agent model loop records successful inference spans and terminal provider error spans. The rollout matrix still lists provider-native retry/fallback and nested provider lifecycle spans as the next gaps. Codex's client retry layer classifies HTTP 429, HTTP 5xx, timeout, and network errors as retryable, backs off between attempts, and keeps inference attempts as rollout evidence.

Novex already has tenant model route policy fields: `ai_model_route.policy`, `ai_model_profile.fallback_policy`, deployment `network_zone`, and `novex-model::evaluate_model_route_policy`. The missing link is runtime use of `maxRetries` and trace evidence that a provider failure was retried.

## Approaches

1. Put retry inside the low-level HTTP model client.
   - Pros: all chat callers get retry automatically.
   - Cons: the Agent loop cannot record one span per failed attempt without changing the model API to stream attempt events.

2. Put retry inside the Agent model loop for Code Agent calls first.
   - Pros: immediate trace/eval evidence, small blast radius, and easy to align with Run Graph events.
   - Cons: chat flow and RAG do not get retry until the lower model runtime API grows attempt callbacks.

3. Implement retry plus route fallback now.
   - Pros: closes both matrix gaps at once.
   - Cons: fallback needs route-chain selection, credential checks, policy validation, and provider snapshots; too wide for a single safe slice.

Recommended: approach 2. This preserves the Codex behavior that retries are observable, while keeping the enterprise model route policy as the source of retry limits. Fallback remains the next slice.

## Design

Add a small runtime retry policy in `ModelRuntimeService`:

```rust
pub struct ModelRetryPolicy {
    pub max_retries: usize,
}
```

`retry_policy_for_purpose(ModelRoutePurpose::CodeAgent)` reads the highest-priority active route and computes `max_retries` from `evaluate_model_route_policy`. The result is capped to `MAX_MODEL_RUNTIME_RETRIES` so tenant policy cannot create runaway loops. Env fallback routes have retry disabled until route policy exists in DB.

The Agent loop wraps the model call:

1. Attempt model call.
2. If it succeeds, record the existing `model_inference` span and continue.
3. If it fails, classify the error using the existing provider error classifier.
4. Record `model_inference_error` with:
   - `attempt`
   - `maxAttempts`
   - `retryable`
   - `willRetry`
   - `errorKind`
   - `httpStatus`
   - `latencyMs`
5. If `willRetry`, wait a short bounded backoff and try again.
6. If not, append `RunEventKind::Error`, finish the run as failed, refresh trace, and return.

Eval aggregation adds `modelRetryCount`, derived from inference error spans whose `willRetry` is true. Existing `inferenceErrorCount` still counts all provider failures.

## Scope

This slice does not add route fallback, circuit breaker persistence, or nested request/response payload storage. It prepares the event contract for those later additions.

## Acceptance

- Model runtime exposes a capped retry policy based on DB route/profile policy.
- Agent model loop performs retryable model calls up to configured attempts.
- Every failed attempt records `model_inference_error` with attempt metadata.
- Eval candidate tags include `modelRetryCount`.
- Successful inference, cancellation, and terminal error behavior remain compatible.

## Codex Parity Notes

Codex retries HTTP 429, HTTP 5xx, timeout, and network errors. Novex uses the same retry classification already introduced by provider error spans. Codex's retry helper uses jittered exponential backoff; Novex starts with a short bounded deterministic backoff inside the Agent loop to avoid surprising enterprise latency while preserving observability.
