# Agent Route Fallback Lifecycle Design

## Context

The Codex migration matrix now has retryable provider errors and retry trace evidence, but route fallback is still only a registry/policy concept. `ai_model_route.fallback_route_id` and profile `fallback_policy` are evaluated for registry summaries and retry caps, while runtime model calls still resolve and execute only one route. The next enterprise foundation slice must make fallback real for the Code Agent path and expose enough evidence for rollout replay, eval gates, and production diagnosis.

## Design Options

### Option A: Model runtime owns fallback, agent trace consumes attempts

`ModelRuntimeService` resolves a primary route plus one eligible fallback route, executes the primary, falls back only for retryable provider/transport failures, and returns a `ModelChatResp` containing provider attempt metadata. The agent service keeps its existing model loop shape and emits the attempt metadata inside the inference payload.

Trade-off: this touches the shared model response contract, but it keeps provider policy near model routing and avoids duplicating fallback behavior in each caller.

### Option B: Agent loop orchestrates fallback directly

The agent service asks the model service for primary/fallback route codes, calls each route explicitly, and writes trace events for each attempt.

Trade-off: this gives the agent service full trace control, but pushes model routing policy into agent runtime code and leaves chat flow/notebook callers without shared fallback behavior.

### Option C: Trace-only fallback tags

Keep runtime behavior unchanged and only add trace/eval placeholders based on route policy.

Trade-off: this is cheap but misaligned with the enterprise goal. It would make the system look observable without actually improving resilience.

## Chosen Approach

Use Option A. The model runtime becomes the source of truth for one-hop fallback, and `ModelChatResp` carries a small `provider_attempts` list. This list is not a new business event store; it is response metadata that lets downstream trace/eval explain what happened during a model sample.

This slice intentionally supports one fallback hop only:

- Primary route is resolved the same way as today.
- Fallback is eligible when route/profile policy enables fallback, a fallback route exists, and policy violations are empty.
- Fallback is attempted only for provider HTTP 429/5xx, timeout, or transport errors.
- Explicit route selection still uses that selected route as the primary; if the selected route has an eligible fallback, it can fall back.
- Circuit breaker cooldown is recorded in policy status but remains a later slice.

## Data Contract

Add `ModelProviderAttempt` to `backend/src/application/ai/model_service.rs`:

- `attemptKind`: `primary` or `fallback`
- `routeId`
- `provider`
- `model`
- `status`: `succeeded` or `failed`
- `latencyMs`
- `errorKind`
- `httpStatus`
- `message`

Extend `ModelChatResp` with `provider_attempts: Vec<ModelProviderAttempt>`.

Agent inference payloads will include:

- top-level model inference fields as today
- `providerAttempts` when present
- `fallbackUsed: true` when any fallback attempt succeeds
- `fallbackRouteId` for the successful fallback route

Eval candidate tags will include:

- `modelFallbackCount`
- `modelFallbackRouteId`
- `modelProviderAttemptCount`

## Components

### Model runtime

Add a route fallback plan query beside the retry policy query. It should read the same route/profile/deployment policy inputs plus fallback route code and fallback network zone. A small helper converts `ModelRoutePolicyStatus` into an executable fallback decision.

Add model-provider error classification in the model layer for fallback eligibility. This should match the retryable category already used by the agent trace: HTTP 429, HTTP 5xx, timeout, and transport errors.

### Agent trace

`model_inference_event_payload` should pass through `providerAttempts` from the model response. It should derive fallback fields from those attempts instead of re-running policy logic.

### Eval

`trace_inference_summary` should inspect both simple inference events and nested `providerAttempts`. It should count fallback successes and preserve the successful fallback route ID.

## Testing Strategy

Use TDD for each behavior:

- Model policy helper: fallback disabled on violations, enabled with valid fallback route.
- Model source-contract test: `chat_completion_for_purpose` uses route fallback planning and retryable provider classification.
- Agent trace payload test: response with primary failed + fallback succeeded attempts emits `providerAttempts`, `fallbackUsed`, and `fallbackRouteId`.
- Eval test: nested provider attempts produce `modelFallbackCount`, `modelFallbackRouteId`, and `modelProviderAttemptCount`.

Final verification for the slice:

```bash
cargo fmt -- --check
cargo test -p backend-rust model_route_fallback --offline
cargo test -p backend-rust provider_lifecycle --offline
cargo test -p novex-eval provider_fallback --offline
cargo test --workspace --offline
```
