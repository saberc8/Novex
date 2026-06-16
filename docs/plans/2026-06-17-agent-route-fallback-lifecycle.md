# Agent Route Fallback Lifecycle Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make configured model route fallback a real Code Agent runtime behavior and expose nested provider lifecycle evidence to trace and eval.

**Architecture:** The model runtime owns one-hop fallback planning and execution. `ModelChatResp` carries provider attempt metadata, the agent trace payload serializes it, and eval derives fallback tags from nested attempts.

**Tech Stack:** Rust, `backend-rust`, `novex-eval`, `novex-model`, `serde_json`, `sqlx`.

---

### Task 1: Commit Design And Plan

**Files:**
- Create: `docs/plans/2026-06-17-agent-route-fallback-lifecycle-design.md`
- Create: `docs/plans/2026-06-17-agent-route-fallback-lifecycle.md`

**Step 1: Review docs**

Run:

```bash
git diff -- docs/plans/2026-06-17-agent-route-fallback-lifecycle-design.md docs/plans/2026-06-17-agent-route-fallback-lifecycle.md
```

Expected: docs describe one-hop fallback, provider attempts, trace/eval tags, and verification commands.

**Step 2: Commit**

Run:

```bash
git add docs/plans/2026-06-17-agent-route-fallback-lifecycle-design.md docs/plans/2026-06-17-agent-route-fallback-lifecycle.md
git commit -m "docs: plan agent route fallback lifecycle"
```

### Task 2: Add Model Fallback Plan Contract

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add tests:

```rust
#[test]
fn model_route_fallback_policy_enables_valid_fallback_route() {
    let status = evaluate_model_route_policy(ModelRoutePolicyInput {
        network_zone: "private",
        fallback_network_zone: Some("private"),
        fallback_policy: &json!({ "enabled": true }),
        route_policy: &Value::Null,
    });

    let decision = model_fallback_policy_decision_from_status(&status, Some("runtime.llm.backup"));

    assert!(decision.enabled);
    assert_eq!(decision.fallback_route_id.as_deref(), Some("runtime.llm.backup"));
}

#[test]
fn model_route_fallback_policy_blocks_policy_violations() {
    let status = evaluate_model_route_policy(ModelRoutePolicyInput {
        network_zone: "private",
        fallback_network_zone: Some("public"),
        fallback_policy: &json!({ "enabled": true }),
        route_policy: &Value::Null,
    });

    let decision = model_fallback_policy_decision_from_status(&status, Some("runtime.llm.backup"));

    assert!(!decision.enabled);
    assert_eq!(decision.block_reason.as_deref(), Some("policy_violation"));
}

#[test]
fn model_route_fallback_source_contract_reads_configured_fallback_route() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("pub async fn fallback_plan_for_purpose"));
    assert!(source.contains("fallback_route.code AS fallback_route_code"));
    assert!(source.contains("evaluate_model_route_policy(ModelRoutePolicyInput"));
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend-rust model_route_fallback --offline
```

Expected: FAIL because fallback decision/helper methods do not exist.

**Step 3: Implement minimal model fallback plan**

Add:

- `ModelFallbackPolicyDecision`
- `ModelRouteFallbackPlan`
- `ModelRouteFallbackPolicyRow`
- `ModelRuntimeService::fallback_plan_for_purpose`
- private helper `model_fallback_policy_decision_from_status`

The query should select primary route policy plus `fallback_route.code AS fallback_route_code`.

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend-rust model_route_fallback --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: expose model route fallback plan"
```

### Task 3: Add Provider Attempt Metadata And Runtime Fallback

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: tests in `backend/src/application/ai/model_service.rs`
- Modify literal `ModelChatResp` construction in backend tests as needed

**Step 1: Write failing tests**

Add tests:

```rust
#[test]
fn provider_lifecycle_attempt_records_success_metadata() {
    let route = llm_test_config().route(ModelRuntimeTarget::Llm).unwrap().clone();
    let attempt = model_provider_attempt_succeeded("fallback", &route, 42);

    assert_eq!(attempt.attempt_kind, "fallback");
    assert_eq!(attempt.route_id, "runtime.llm");
    assert_eq!(attempt.status, "succeeded");
    assert_eq!(attempt.latency_ms, 42);
}

#[test]
fn provider_lifecycle_attempt_records_retryable_http_failure() {
    let route = llm_test_config().route(ModelRuntimeTarget::Llm).unwrap().clone();
    let err = AppError::bad_request("LLM 模型调用失败: HTTP 502");
    let attempt = model_provider_attempt_failed("primary", &route, &err, 12);

    assert_eq!(attempt.status, "failed");
    assert_eq!(attempt.error_kind.as_deref(), Some("provider_http"));
    assert_eq!(attempt.http_status, Some(502));
}

#[test]
fn provider_lifecycle_source_contract_fallback_wraps_chat_completion() {
    let source = include_str!("model_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("fallback_plan_for_purpose(purpose"));
    assert!(source.contains("model_provider_error_is_fallback_candidate"));
    assert!(source.contains("attempt_kind = \"fallback\""));
}
```

**Step 2: Run RED**

Run:

```bash
cargo test -p backend-rust provider_lifecycle --offline
```

Expected: FAIL because attempt metadata does not exist.

**Step 3: Implement minimal runtime fallback**

Add `ModelProviderAttempt` and `provider_attempts` to `ModelChatResp`.

Update `execute_normalized_chat_completion_with_route` to attach a successful primary attempt. Add helper functions:

- `model_provider_attempt_succeeded`
- `model_provider_attempt_failed`
- `model_provider_error_is_fallback_candidate`
- `model_provider_error_class`
- `model_provider_error_http_status`
- `execute_normalized_chat_completion_with_fallback`

`chat_completion_for_purpose` should call the fallback wrapper. On primary failure, if the fallback plan is enabled and the error is fallback-eligible, execute the fallback route and prepend the failed primary attempt to the fallback response attempts.

**Step 4: Run GREEN**

Run:

```bash
cargo test -p backend-rust provider_lifecycle --offline
cargo test -p backend-rust model_chat_response --offline
cargo test -p backend-rust model_chat_usage_record --offline
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add backend/src/application/ai/model_service.rs backend/src/application/ai/agent_service.rs backend/src/application/ai/integration_service.rs backend/src/application/ai/knowledge_service.rs
git commit -m "feat: execute model route fallback attempts"
```

### Task 4: Emit Agent Trace Lifecycle And Eval Fallback Tags

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `crates/novex-eval/src/lib.rs`

**Step 1: Write failing agent trace test**

Add a test in `agent_service.rs`:

```rust
#[test]
fn provider_lifecycle_trace_payload_exposes_fallback_attempts() {
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
            test_provider_attempt("primary", "runtime.llm", "failed"),
            test_provider_attempt("fallback", "runtime.llm.backup", "succeeded"),
        ],
    };

    let payload = model_inference_event_payload(&response);

    assert_eq!(payload["item"]["fallbackUsed"], true);
    assert_eq!(payload["item"]["fallbackRouteId"], "runtime.llm.backup");
    assert_eq!(payload["item"]["providerAttempts"].as_array().unwrap().len(), 2);
}
```

**Step 2: Write failing eval test**

Add a test in `crates/novex-eval/src/lib.rs`:

```rust
#[test]
fn trace_eval_candidate_tags_provider_fallback_attempts() {
    let bundle = TraceBundle::new("agent-1").with_event(TraceEvent::inference(
        1,
        json!({
            "item": {
                "type": "model_inference",
                "routeId": "runtime.llm.backup",
                "providerAttempts": [
                    { "attemptKind": "primary", "routeId": "runtime.llm", "status": "failed" },
                    { "attemptKind": "fallback", "routeId": "runtime.llm.backup", "status": "succeeded" }
                ]
            }
        }),
    ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["modelProviderAttemptCount"], 2);
    assert_eq!(candidate.tags["modelFallbackCount"], 1);
    assert_eq!(candidate.tags["modelFallbackRouteId"], "runtime.llm.backup");
}
```

**Step 3: Run RED**

Run:

```bash
cargo test -p backend-rust provider_lifecycle_trace --offline
cargo test -p novex-eval provider_fallback --offline
```

Expected: FAIL because payload/eval extraction is missing.

**Step 4: Implement trace and eval extraction**

Update `model_inference_event_payload` to insert:

- `providerAttempts`
- `fallbackUsed`
- `fallbackRouteId`

Update `TraceInferenceSummary` with:

- `provider_attempt_count`
- `fallback_count`
- `fallback_route_id`

Read nested `providerAttempts` from inference payloads.

**Step 5: Run GREEN**

Run:

```bash
cargo test -p backend-rust provider_lifecycle_trace --offline
cargo test -p novex-eval provider_fallback --offline
```

Expected: PASS.

**Step 6: Commit**

Run:

```bash
git add backend/src/application/ai/agent_service.rs crates/novex-eval/src/lib.rs
git commit -m "feat: trace provider fallback lifecycle"
```

### Task 5: Update Matrix, Verify, Merge

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update matrix**

Set rollout trace to the next slice status and record route fallback/provider lifecycle evidence. Add this plan to follow-up implementation plans.

**Step 2: Commit matrix**

Run:

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: record route fallback lifecycle progress"
```

**Step 3: Final verification on feature branch**

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust model_route_fallback --offline
cargo test -p backend-rust provider_lifecycle --offline
cargo test -p backend-rust provider_lifecycle_trace --offline
cargo test -p novex-eval provider_fallback --offline
cargo test --workspace --offline
```

Expected: PASS. `live_rag_e2e` remains ignored unless external infra is configured.

**Step 4: Merge to local main**

Run:

```bash
cd /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex
git merge --no-ff feat/enterprise-agent-foundation -m "merge: enterprise agent foundation route fallback lifecycle"
cargo fmt -- --check
cargo test --workspace --offline
cd /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex/.worktrees/enterprise-agent-foundation
git merge --ff-only main
git status --short --branch
```

Expected: both main and feature worktree are clean.
