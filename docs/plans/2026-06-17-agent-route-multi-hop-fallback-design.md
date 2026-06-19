# Agent Route Multi-Hop Fallback Design

## Context

The current model runtime has one-hop fallback:

1. resolve the selected primary route for a purpose,
2. execute the primary route,
3. when the error is fallback-eligible and policy allows fallback, execute the configured fallback route once,
4. expose `providerAttempts` to trace/eval.

This is enough for the first Codex-style provider lifecycle slice, but it is not yet enterprise-grade. Real deployments often need a short chain such as primary regional provider -> secondary regional provider -> global provider. The route policy already supports querying fallback for any route id, so this slice can extend runtime behavior without adding a new config schema.

## Goal

Support bounded multi-hop model route fallback chains for `chat_completion_for_purpose`, while preserving complete provider-attempt evidence for trace, eval, and future rollout replay.

## Approach

Use the existing `ai_model_route.fallback_route_id` chain and `fallback_plan_for_purpose_with_route_id(purpose, route_id)` as the source of truth. Runtime attempts remain sequential:

1. Attempt the current route.
2. If it succeeds, return the response with all prior attempts prepended.
3. If it fails with a fallback-eligible provider error, record a failed attempt.
4. If the route has an enabled fallback plan, move to the next route.
5. Stop when there is no enabled fallback, a route id repeats, or the bounded hop limit is reached.

The first attempted route keeps `attemptKind = "primary"`. Every later attempted route uses `attemptKind = "fallback"`. Circuit-open skips are also attempt records and should not hide the route from trace/eval.

## Bounds And Failure Semantics

Add a small fixed bound, `MAX_MODEL_FALLBACK_HOPS`, to prevent accidental infinite chains. A hop means a fallback route after the primary. With `MAX_MODEL_FALLBACK_HOPS = 3`, a request may attempt at most four routes: primary plus three fallback routes.

Cycle protection is separate from the hop bound. The runtime tracks visited route ids. If the next fallback route is already visited, it stops and returns the most recent provider error instead of looping.

If the last attempted route fails and no valid next fallback exists, the method returns that last route's error. The failure is still visible inside `providerAttempts` only when a later route succeeds. This keeps the current external error contract unchanged and avoids inventing a new error envelope in this slice.

## Circuit Breaker Interaction

Before attempting a route, the runtime checks whether that route's circuit is open. If open and the route has an enabled fallback plan, it records a skipped attempt and moves to the next route. If open but no fallback is available, it returns a bad request indicating the fallback route is unavailable.

After a fallback-eligible failure, the runtime opens that route's circuit according to the route's own `circuitBreakerSeconds` policy. This applies to primary and fallback routes.

## Trace And Eval

No schema change is required. `providerAttempts` already carries ordered attempts with route id, provider, model, status, error kind, and latency. Existing trace/eval fallback counters will count every successful fallback attempt. Existing circuit-open counters will count skipped circuit attempts.

This slice adds tests that prove a multi-hop success response preserves:

- failed primary attempt,
- failed first fallback attempt,
- succeeded second fallback attempt.

## Out Of Scope

- Persisted/cross-process circuit breaker state.
- Parallel hedged provider racing.
- New DB schema for fallback chains.
- New public API fields beyond existing `providerAttempts`.
- Changing non-`chat_completion_for_purpose` direct chat-flow paths.

## Acceptance

- `cargo test -p backend multi_hop_fallback --offline`
- `cargo test -p backend provider_lifecycle --offline`
- `cargo test -p backend route_circuit_breaker --offline`
- `cargo test -p novex-eval provider_fallback --offline`
- `cargo fmt -- --check`
- `cargo test --workspace --offline`
