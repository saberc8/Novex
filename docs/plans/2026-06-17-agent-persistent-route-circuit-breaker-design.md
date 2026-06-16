# Agent Persistent Route Circuit Breaker Design

## Context

Route fallback, circuit-open trace evidence, and bounded multi-hop fallback are now in place. The remaining gap is deployment realism: the circuit breaker is process-local, so each backend worker learns failures independently. That is not enough for enterprise deployments where the same tenant can be served by multiple backend instances.

## Goal

Persist model route circuit breaker state so every backend instance can skip a route that another instance has already opened.

## Approach

Add a runtime table, `ai_model_route_circuit_breaker`, keyed by `(tenant_id, route_id)`. `route_id` stores the model route code, such as `runtime.llm.code_agent`, because runtime traces and `ModelRuntimeRoute` already use that external route id. The table stores:

- `opened_until`: when the route can be tried again,
- `open_reason`: short machine-readable reason,
- `last_error_kind` and `last_http_status`: evidence for operators,
- standard create/update audit fields.

The existing process-local registry stays as a fast path and as a fallback for the same-process request stream. The database is the cross-process source of truth.

## Runtime Flow

Before attempting a route:

1. Check the process-local breaker.
2. If not open locally, query `ai_model_route_circuit_breaker` for an unexpired row.
3. If an unexpired row exists, return a skipped `providerAttempt` with `errorKind = circuit_open`.
4. If an expired row exists, ignore it; cleanup can be opportunistic or left for a later maintenance task.

After a fallback-eligible failure:

1. Record the failed provider attempt.
2. Open the process-local breaker.
3. Upsert the persistent breaker row with the route id, tenant id, opened-until timestamp, and failure classification.
4. Continue to the next fallback route when policy allows it.

The runtime should not change the public error envelope in this slice. Trace/eval evidence still comes from ordered `providerAttempts`.

## SQL Shape

Use a separate additive migration:

```sql
CREATE TABLE IF NOT EXISTS ai_model_route_circuit_breaker (...);
CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_model_route_circuit_breaker_tenant_route
    ON ai_model_route_circuit_breaker (tenant_id, route_id);
CREATE INDEX IF NOT EXISTS idx_ai_model_route_circuit_breaker_opened_until
    ON ai_model_route_circuit_breaker (opened_until);
```

This avoids mutating the older registry migration and keeps rollout safe.

## Alternatives Considered

1. **Keep process-local only.** Simple, but weak for multi-instance deployments.
2. **Store breaker state on `ai_model_route.policy`.** Avoids a new table, but mixes static policy and runtime state.
3. **New runtime table.** Slightly more schema, but clean operational separation and easy inspection. This is the chosen path.

## Out Of Scope

- Automatic background cleanup of expired breaker rows.
- Manual admin UI to clear breaker rows.
- Per-provider rolling failure counters.
- Distributed locks or hedged provider racing.

## Acceptance

- Migration defines `ai_model_route_circuit_breaker` with tenant/route uniqueness and open evidence columns.
- Runtime opens persistent breaker rows after fallback-eligible failures.
- Runtime reads persistent breaker rows before route execution.
- Existing process-local breaker, fallback chain, trace, and eval tests remain green.
- `cargo fmt -- --check`
- `cargo test --workspace --offline`
