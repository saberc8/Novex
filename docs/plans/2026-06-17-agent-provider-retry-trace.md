# Agent Provider Retry Trace Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Retry retryable Code Agent provider failures from route policy and preserve each failed attempt in rollout/eval traces.

**Architecture:** Add a capped `ModelRetryPolicy` in `ModelRuntimeService`, derive it from existing route/profile policy, and use it in the Agent model loop. Failed attempts keep using `model_inference_error` spans, extended with `attempt`, `maxAttempts`, and `willRetry`. `novex-eval` aggregates `modelRetryCount` from those spans.

**Tech Stack:** Rust, sqlx, serde_json, existing `novex-model` policy helpers, backend Run Graph events, Cargo offline tests.

---

### Task 1: Add Model Runtime Retry Policy

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write the failing tests**

Add:

```rust
#[test]
fn model_route_retry_policy_caps_policy_max_retries() {
    let status = ModelRoutePolicyStatus {
        network_zone: "public".to_owned(),
        fallback_network_zone: None,
        fallback_enabled: false,
        cross_zone_fallback_allowed: false,
        max_retries: 10,
        circuit_breaker_seconds: 0,
        violations: vec![],
    };

    let policy = model_retry_policy_from_route_policy_status(&status);

    assert_eq!(policy.max_retries, 3);
    assert_eq!(policy.max_attempts(), 4);
}
```

Add source-contract coverage:

```rust
#[test]
fn model_runtime_retry_policy_reads_route_policy_source_contract() {
    let source = include_str!("model_service.rs");

    assert!(source.contains("pub async fn retry_policy_for_purpose"));
    assert!(source.contains("profile.fallback_policy"));
    assert!(source.contains("evaluate_model_route_policy"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend model_route_retry_policy --offline
```

Expected: FAIL because the retry policy type/helper/query does not exist.

**Step 3: Implement minimal code**

Add:

```rust
pub struct ModelRetryPolicy {
    pub max_retries: usize,
}
```

with:

```rust
impl ModelRetryPolicy {
    pub const fn disabled() -> Self;
    pub const fn max_attempts(&self) -> usize;
}
```

Add `retry_policy_for_purpose` on `ModelRuntimeService` that reads the highest-priority route policy and evaluates it using `evaluate_model_route_policy`.

**Step 4: Run test**

Run:

```bash
cargo test -p backend model_route_retry_policy --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: expose model route retry policy"
```

### Task 2: Add Eval Retry Count Tags

**Files:**
- Modify: `crates/novex-eval/src/lib.rs`

**Step 1: Write the failing test**

Update `trace_eval_candidate_tags_provider_error_spans` or add:

```rust
#[test]
fn trace_eval_candidate_tags_provider_retry_spans() {
    let bundle = TraceBundle::new("agent-1")
        .with_event(TraceEvent::inference(
            1,
            json!({
                "item": {
                    "type": "model_inference_error",
                    "errorKind": "provider_http",
                    "httpStatus": 502,
                    "retryable": true,
                    "willRetry": true,
                    "latencyMs": 12
                }
            }),
        ))
        .with_event(TraceEvent::inference(
            2,
            json!({
                "item": {
                    "type": "model_inference",
                    "latencyMs": 20
                }
            }),
        ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["inferenceErrorCount"], 1);
    assert_eq!(candidate.tags["modelRetryCount"], 1);
    assert_eq!(candidate.tags["latencyMs"], 32);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p novex-eval provider_retry_spans --offline
```

Expected: FAIL because eval does not aggregate `willRetry`.

**Step 3: Implement minimal code**

Add `retry_count` to `TraceInferenceSummary`, increment when `willRetry == true`, and insert `modelRetryCount` when nonzero.

**Step 4: Run test**

Run:

```bash
cargo test -p novex-eval provider_retry_spans --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-eval/src/lib.rs
git commit -m "feat: tag eval candidates with provider retries"
```

### Task 3: Retry Agent Model Calls And Trace Attempts

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write the failing tests**

Add payload coverage:

```rust
#[test]
fn model_inference_error_event_payload_marks_retry_attempts() {
    let payload = model_inference_error_attempt_event_payload(
        &AppError::bad_request("LLM 模型调用失败: HTTP 429"),
        12,
        2,
        3,
        true,
    );

    assert_eq!(payload["item"]["attempt"], 2);
    assert_eq!(payload["item"]["maxAttempts"], 3);
    assert_eq!(payload["item"]["willRetry"], true);
    assert_eq!(payload["item"]["retryable"], true);
    assert_eq!(payload["item"]["httpStatus"], 429);
}
```

Add source-contract coverage:

```rust
#[test]
fn agent_service_model_loop_retries_retryable_provider_errors() {
    let source = include_str!("agent_service.rs").split("#[cfg(test)]").next().unwrap();

    assert!(source.contains("retry_policy_for_purpose(ModelRoutePurpose::CodeAgent)"));
    assert!(source.contains("for attempt in 1..=model_retry_policy.max_attempts()"));
    assert!(source.contains("will_retry"));
    assert!(source.contains("model_inference_error_attempt_event_payload"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend provider_retry --offline
```

Expected: FAIL because attempt payload and retry loop do not exist.

**Step 3: Implement minimal code**

Before run records are created, fetch:

```rust
let model_retry_policy = self
    .model_runtime
    .retry_policy_for_purpose(ModelRoutePurpose::CodeAgent)
    .await?;
```

Wrap the model call in `for attempt in 1..=model_retry_policy.max_attempts()`. On retryable errors before the last attempt, append `model_inference_error_attempt_event_payload(..., will_retry = true)`, sleep a bounded retry delay, and continue. On terminal errors, append `willRetry: false`, then finish failed as today.

**Step 4: Run tests**

Run:

```bash
cargo test -p backend provider_retry --offline
cargo test -p backend provider_error --offline
cargo test -p backend model_loop --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: retry agent provider errors"
```

### Task 4: Matrix And Full Verification

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update matrix**

Move Rollout trace to the next slice and record provider retry evidence. Leave route fallback and nested provider lifecycle as follow-ups.

**Step 2: Run focused verification**

Run:

```bash
cargo fmt -- --check
cargo test -p backend model_route_retry_policy --offline
cargo test -p backend provider_retry --offline
cargo test -p backend provider_error --offline
cargo test -p backend model_loop --offline
cargo test -p novex-eval provider_retry_spans --offline
```

Expected: PASS.

**Step 3: Run full verification**

Run:

```bash
cargo test --workspace --offline
```

Expected: PASS, with `live_rag_e2e` ignored unless POC infra is present.

**Step 4: Commit**

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: record provider retry trace progress"
```

**Step 5: Merge stage to main**

Run from the main worktree:

```bash
git merge --no-ff feat/enterprise-agent-foundation -m "merge: enterprise agent foundation provider retry trace"
cargo fmt -- --check
cargo test --workspace --offline
```

Then fast-forward the feature worktree back to `main`.
