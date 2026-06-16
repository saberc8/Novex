# Agent Inference Cost Trace Design

## Goal

Make agent inference spans carry route-derived model cost when the selected DB model route has a pricing `cost_spec`. This lets rollout/eval gates check cost from the same trace bundle they already use for latency, token, provider, and route evidence.

## Current State

`ModelChatResp` contains route, provider, model, latency, and token usage. The agent model loop records that metadata in `model_inference` run events, which become `TraceEventKind::Inference` and eval tags.

`costCents` is currently always `null` in agent inference spans. The backend already computes model usage cost when persisting `ai_model_usage`, but `chat_completion_for_purpose` does not persist usage and therefore does not expose cost to agent trace/replay.

## Options

### Option A: Join `ai_model_usage` during trace replay

This can eventually provide authoritative persisted cost, but it does not help model-loop agent runs that currently do not write usage rows.

### Option B: Carry `cost_spec` inside `ModelRuntimeRoute`

This would make cost estimation available immediately after provider response, but it expands the shared `novex-model` route contract and also needs a representation for env fallback routes that do not have DB pricing.

### Option C: Estimate response cost in `ModelRuntimeService`

After `chat_completion_for_purpose` resolves and calls a DB route, look up the selected route's profile `cost_spec` by route code and compute `cost_cents` from the normalized token usage. Env fallback or missing/empty cost spec keeps `cost_cents=None`.

This is selected. It is small, avoids fabricating cost, and keeps cost estimation tied to the tenant DB route.

## Selected Design

Add `cost_cents: Option<f64>` to `ModelChatResp`.

Add a helper in `model_service.rs`:

```rust
fn model_chat_cost_cents_from_spec(cost_spec: &Value, response: &ModelChatResp) -> Option<f64>
```

It returns `None` for `null` or empty object specs. Otherwise it uses existing `estimate_model_cost_cents`.

Add an async helper:

```rust
async fn estimate_model_chat_response_cost_cents(db, tenant_id, response) -> Result<Option<f64>, AppError>
```

It queries `ai_model_route` + `ai_model_profile` by tenant and route code. It returns `None` when the selected route is env-only or has no cost spec.

`chat_completion_with_usage`, `chat_completion_for_source`, and `chat_completion_for_purpose` populate `response.cost_cents` after provider response. Existing `record_model_chat_usage` continues computing persisted usage cost independently.

`model_inference_event_payload` writes:

```json
"costCents": response.costCents
```

not a hard-coded null.

## Eval Impact

`novex-eval` already sums `costCents` from inference spans when present. No schema change is needed there. This slice makes that existing behavior receive real cost for DB-backed model routes.

## Non-Goals

- No DB migration.
- No provider-native billing import.
- No persisted usage write for `chat_completion_for_purpose`.
- No cost fabrication for env fallback routes.

## Verification

- Unit test proves `model_chat_cost_cents_from_spec` computes cost from route pricing and token usage.
- Unit test proves missing/empty cost spec leaves response cost unset.
- Unit test proves agent inference payload writes response cost into `costCents`.
- Existing inference trace/eval tests remain green.
- Full workspace offline tests pass before merging back to `main`.
