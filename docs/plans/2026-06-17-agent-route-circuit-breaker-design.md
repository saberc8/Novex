# Agent Route Circuit Breaker Design

## Context

The model route policy contract already parses `circuitBreakerSeconds`, and the Code Agent runtime now supports retry, one-hop fallback, provider attempt metadata, trace replay, and eval fallback tags. The remaining resilience gap is that a repeatedly failing primary route is still sampled first on every request. Codex-style client resilience needs a cooldown gate: after a retryable primary provider failure, the runtime should temporarily skip the primary route and go straight to the configured fallback route.

## Design Options

### Option A: Process-local route circuit breaker

Keep an in-memory `HashMap<route_id, opened_until>` inside the model runtime process. When a primary route fails with a fallback-eligible provider error and policy has `circuitBreakerSeconds > 0`, open the breaker. While open, `chat_completion_for_purpose` skips the primary route, records a `providerAttempts` item with `status=skipped` and `errorKind=circuit_open`, then executes the fallback route.

Trade-off: this is not cross-process durable, but it is real runtime behavior with low blast radius and fits the current synchronous request model.

### Option B: Persist breaker state in the model registry

Add a route health/circuit state table and make every instance read/write breaker state.

Trade-off: this is closer to full enterprise multi-instance control, but it needs schema design, pruning, health dashboard semantics, and race handling. It is too wide for this slice.

### Option C: Trace-only breaker evidence

Emit breaker-shaped trace tags without changing model execution.

Trade-off: rejected. It would create observability theater rather than resilience.

## Chosen Approach

Use Option A now, with the contract shaped so Option B can replace the backing store later.

This slice adds:

- A process-local route circuit breaker registry.
- A skipped primary provider attempt when a breaker is open.
- Opening the breaker only after fallback-eligible primary failures and only when route policy enables a positive cooldown.
- Agent trace payload pass-through through the existing `providerAttempts` list.
- Eval tags for `modelCircuitOpenCount`.

## Runtime Semantics

For `chat_completion_for_purpose`:

1. Resolve the selected primary route and fallback plan.
2. If fallback is enabled and the primary route circuit is open, skip the primary provider call.
3. Execute the fallback route and prepend a `providerAttempts` item:
   - `attemptKind=primary`
   - `routeId=<primary>`
   - `status=skipped`
   - `errorKind=circuit_open`
   - `message=model route circuit breaker open`
4. If the breaker is not open, call the primary route as today.
5. If the primary fails with HTTP 429/5xx, timeout, or transport error:
   - record a failed primary provider attempt;
   - open the breaker for `circuitBreakerSeconds` when policy enables it;
   - execute the fallback route if fallback is enabled.
6. Non-fallback-eligible errors keep the existing terminal error behavior.

## Scope Limits

This slice does not add:

- cross-process persisted breaker state;
- operator UI for breaker status;
- multi-hop fallback;
- active health probing;
- adaptive thresholds beyond the existing policy cooldown.

## Testing Strategy

Use TDD:

- Model helper test: opening a route circuit produces a skipped `providerAttempts` item.
- Model helper test: cooldown is disabled when fallback is disabled or cooldown is zero.
- Source-contract test: `execute_normalized_chat_completion_with_fallback` checks the breaker before primary calls and opens it after fallback-eligible failures.
- Agent trace test: a skipped primary attempt is serialized.
- Eval test: nested provider attempts with `errorKind=circuit_open` produce `modelCircuitOpenCount`.

Final verification:

```bash
cargo fmt -- --check
cargo test -p backend route_circuit_breaker --offline
cargo test -p backend provider_lifecycle_trace --offline
cargo test -p novex-eval circuit_breaker --offline
cargo test --workspace --offline
```
