# Agent Route Circuit Breaker Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make `circuitBreakerSeconds` route policy affect real Code Agent model execution and expose circuit-open evidence in trace/eval.

**Architecture:** Add a process-local route circuit breaker registry to the model runtime. `chat_completion_for_purpose` checks the breaker before sampling the primary route, opens it after fallback-eligible failures, records skipped primary provider attempts, and keeps trace/eval extraction based on nested `providerAttempts`.

**Tech Stack:** Rust, `backend-rust`, `novex-eval`, `serde_json`, `std::sync::OnceLock`, `Mutex<HashMap<_, _>>`.

---

### Task 1: Commit Design And Plan

**Files:**
- Create: `docs/plans/2026-06-17-agent-route-circuit-breaker-design.md`
- Create: `docs/plans/2026-06-17-agent-route-circuit-breaker.md`

**Step 1: Review docs**

Run:

```bash
git diff -- docs/plans/2026-06-17-agent-route-circuit-breaker-design.md docs/plans/2026-06-17-agent-route-circuit-breaker.md
```

Expected: docs describe process-local cooldown, providerAttempts evidence, eval tags, and verification commands.

**Step 2: Commit**

Run:

```bash
git add docs/plans/2026-06-17-agent-route-circuit-breaker-design.md docs/plans/2026-06-17-agent-route-circuit-breaker.md
git commit -m "docs: plan agent route circuit breaker"
```

### Task 2: Add Model Circuit Breaker Helpers

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add tests:

```rust
#[test]
fn route_circuit_breaker_attempt_marks_open_route_as_skipped() {
    let route = llm_test_config().route(ModelRuntimeTarget::Llm).unwrap().clone();
    model_circuit_breaker_clear(route.route_id());
    model_circuit_breaker_open(route.route_id(), 30);

    let attempt = model_circuit_breaker_open_attempt(&route).unwrap();

    assert_eq!(attempt.attempt_kind, "primary");
    assert_eq!(attempt.status, "skipped");
    assert_eq!(attempt.error_kind.as_deref(), Some("circuit_open"));
    assert_eq!(attempt.latency_ms, 0);
    model_circuit_breaker_clear(route.route_id());
}

#[test]
fn route_circuit_breaker_cooldown_requires_enabled_fallback_and_positive_policy() {
    let disabled = ModelRouteFallbackPlan {
        primary_route_id: "runtime.llm".to_owned(),
        decision: ModelFallbackPolicyDecision {
            enabled: false,
            fallback_route_id: Some("runtime.llm.backup".to_owned()),
            block_reason: Some("fallback_disabled".to_owned()),
        },
        policy_status: ModelRoutePolicyStatus {
            network_zone: "public".to_owned(),
            fallback_network_zone: Some("public".to_owned()),
            fallback_enabled: false,
            cross_zone_fallback_allowed: false,
            max_retries: 0,
            circuit_breaker_seconds: 30,
            violations: vec![],
        },
    };

    assert_eq!(model_circuit_breaker_cooldown_seconds(Some(&disabled)), None);
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend-rust route_circuit_breaker --offline
```

Expected: FAIL because circuit breaker helpers do not exist.

**Step 3: Implement helpers**

Add:

- `MODEL_ROUTE_CIRCUIT_BREAKERS: OnceLock<Mutex<HashMap<String, Instant>>>`
- `model_circuit_breaker_registry`
- `model_circuit_breaker_open`
- `model_circuit_breaker_clear`
- `model_circuit_breaker_open_attempt`
- `model_provider_attempt_circuit_open`
- `model_circuit_breaker_cooldown_seconds`

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend-rust route_circuit_breaker --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: add model route circuit breaker state"
```

### Task 3: Wire Circuit Breaker Into Runtime Fallback

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing source-contract test**

Add:

```rust
#[test]
fn route_circuit_breaker_source_contract_bypasses_primary_and_opens_after_failure() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("model_circuit_breaker_open_attempt(primary_route)"));
    assert!(source.contains("model_circuit_breaker_open(primary_route.route_id()"));
    assert!(source.contains("model_circuit_breaker_cooldown_seconds(fallback_plan.as_ref())"));
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend-rust route_circuit_breaker_source --offline
```

Expected: FAIL because runtime wrapper has not been wired.

**Step 3: Implement runtime wiring**

In `execute_normalized_chat_completion_with_fallback`:

1. Load fallback plan before primary call.
2. If plan is enabled and `model_circuit_breaker_open_attempt(primary_route)` returns an attempt, execute fallback immediately and prepend that attempt.
3. After a fallback-eligible primary failure, call `model_circuit_breaker_open` when `model_circuit_breaker_cooldown_seconds(fallback_plan.as_ref())` returns `Some(seconds)`.

Extract a small fallback execution helper only if needed to keep duplication bounded.

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend-rust route_circuit_breaker_source --offline
cargo test -p backend-rust provider_lifecycle --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: bypass open model route circuits"
```

### Task 4: Trace And Eval Circuit Evidence

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `crates/novex-eval/src/lib.rs`

**Step 1: Write failing trace test**

Add to `agent_service.rs`:

```rust
#[test]
fn route_circuit_breaker_trace_payload_preserves_skipped_attempt() {
    let response = ModelChatResp {
        conversation_id: None,
        answer: "ok".to_owned(),
        route_id: "runtime.llm.backup".to_owned(),
        provider: "deep-seek".to_owned(),
        model: Some("deepseek-v4-flash".to_owned()),
        latency_ms: 20,
        usage: ModelChatUsage::default(),
        cost_cents: None,
        provider_attempts: vec![
            test_provider_attempt_with_error("primary", "runtime.llm", "skipped", "circuit_open"),
            test_provider_attempt("fallback", "runtime.llm.backup", "succeeded"),
        ],
    };

    let payload = model_inference_event_payload(&response);

    assert_eq!(payload["item"]["providerAttempts"][0]["errorKind"], "circuit_open");
}
```

**Step 2: Write failing eval test**

Add to `crates/novex-eval/src/lib.rs`:

```rust
#[test]
fn trace_eval_candidate_tags_circuit_breaker_attempts() {
    let bundle = TraceBundle::new("agent-1").with_event(TraceEvent::inference(
        1,
        json!({
            "item": {
                "type": "model_inference",
                "providerAttempts": [
                    { "attemptKind": "primary", "routeId": "runtime.llm", "status": "skipped", "errorKind": "circuit_open" },
                    { "attemptKind": "fallback", "routeId": "runtime.llm.backup", "status": "succeeded" }
                ]
            }
        }),
    ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["modelCircuitOpenCount"], 1);
    assert_eq!(candidate.tags["modelFallbackCount"], 1);
}
```

**Step 3: Run RED**

Run:

```bash
cargo test -p backend-rust route_circuit_breaker_trace --offline
cargo test -p novex-eval circuit_breaker --offline
```

Expected: eval FAILS until `modelCircuitOpenCount` is extracted. Agent test may already pass through generic attempts; keep it as a regression guard.

**Step 4: Implement eval extraction**

Update `TraceInferenceSummary` with `circuit_open_count`. During `providerAttempts` scan, increment when `errorKind == "circuit_open"`. Emit `modelCircuitOpenCount` when positive.

**Step 5: Run GREEN**

Run:

```bash
cargo test -p backend-rust route_circuit_breaker_trace --offline
cargo test -p novex-eval circuit_breaker --offline
```

Expected: PASS.

**Step 6: Commit**

Run:

```bash
git add backend/src/application/ai/agent_service.rs crates/novex-eval/src/lib.rs
git commit -m "feat: tag circuit breaker provider attempts"
```

### Task 5: Update Matrix, Verify, Merge

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update matrix**

Move rollout trace notes from circuit breaker as next to implemented evidence. Leave multi-hop provider lifecycle and persisted/cross-process breaker state as follow-ups.

**Step 2: Commit matrix**

Run:

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: record route circuit breaker progress"
```

**Step 3: Final verification on feature branch**

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust route_circuit_breaker --offline
cargo test -p backend-rust provider_lifecycle --offline
cargo test -p backend-rust provider_lifecycle_trace --offline
cargo test -p novex-eval circuit_breaker --offline
cargo test --workspace --offline
```

Expected: PASS. `live_rag_e2e` remains ignored unless external infra is configured.

**Step 4: Merge to local main**

Run:

```bash
cd /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex
git merge --no-ff feat/enterprise-agent-foundation -m "merge: enterprise agent foundation route circuit breaker"
cargo fmt -- --check
cargo test --workspace --offline
cd /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex/.worktrees/enterprise-agent-foundation
git merge --ff-only main
git status --short --branch
```

Expected: both main and feature worktree are clean.
