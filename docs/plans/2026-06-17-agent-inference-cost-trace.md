# Agent Inference Cost Trace Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Populate agent inference trace spans with DB route-derived model cost when tenant pricing metadata exists.

**Architecture:** Extend `ModelChatResp` with optional `cost_cents`, compute it in `ModelRuntimeService` from existing route `cost_spec`, and serialize it through the existing agent `model_inference` payload so `novex-eval` can use its existing cost aggregation.

**Tech Stack:** Rust, serde/serde_json, sqlx, existing `novex-model` cost helpers, Cargo offline tests.

---

### Task 1: Add Response Cost Field And Pure Cost Helper

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write the failing test**

Add:

```rust
#[test]
fn model_chat_cost_cents_from_spec_uses_response_usage() {
    let response = ModelChatResp {
        conversation_id: None,
        answer: "ok".to_owned(),
        route_id: "runtime.llm".to_owned(),
        provider: "deep-seek".to_owned(),
        model: Some("deepseek-v4-flash".to_owned()),
        latency_ms: 42,
        usage: ModelChatUsage {
            prompt_tokens: Some(1000),
            completion_tokens: Some(2000),
            total_tokens: Some(3000),
        },
        cost_cents: None,
    };
    let cost_spec = json!({
        "unit": "tokens",
        "inputPer1kCents": 0.1,
        "outputPer1kCents": 0.2
    });

    let cost_cents = model_chat_cost_cents_from_spec(&cost_spec, &response).unwrap();

    assert!((cost_cents - 0.5).abs() < 0.000_001);
}
```

Add:

```rust
#[test]
fn model_chat_cost_cents_from_spec_ignores_missing_spec() {
    let response = test_model_chat_response();

    assert_eq!(model_chat_cost_cents_from_spec(&Value::Null, &response), None);
    assert_eq!(model_chat_cost_cents_from_spec(&json!({}), &response), None);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend-rust model_chat_cost_cents --offline
```

Expected: FAIL because the helper and `cost_cents` field do not exist.

**Step 3: Implement minimal code**

Add `pub cost_cents: Option<f64>` to `ModelChatResp`.

Add:

```rust
fn model_chat_cost_cents_from_spec(cost_spec: &Value, response: &ModelChatResp) -> Option<f64>
```

Use existing `estimate_model_cost_cents` and `response.usage.accounting_counts()`.

Update existing `ModelChatResp` test literals with `cost_cents: None`.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p backend-rust model_chat_cost_cents --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: estimate model chat response cost"
```

### Task 2: Populate Cost For Runtime Service Responses

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write the failing source-contract test**

Add or update a test asserting the source contains:

```rust
"estimate_model_chat_response_cost_cents"
"response.cost_cents"
"chat_completion_for_purpose"
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend-rust model_chat_response_cost_runtime --offline
```

Expected: FAIL until runtime service fills response cost.

**Step 3: Implement minimal code**

Add:

```rust
async fn estimate_model_chat_response_cost_cents(
    db: &PgPool,
    tenant_id: i64,
    response: &ModelChatResp,
) -> Result<Option<f64>, AppError>
```

Call it in `chat_completion_with_usage`, `chat_completion_for_source`, and `chat_completion_for_purpose` after provider response.

**Step 4: Run tests**

Run:

```bash
cargo test -p backend-rust model_chat_response_cost_runtime --offline
cargo test -p backend-rust model_chat_response_extracts_answer_usage_and_route_summary --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: attach route cost to model chat responses"
```

### Task 3: Serialize Response Cost In Agent Inference Span

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write the failing test**

Add:

```rust
#[test]
fn model_inference_event_payload_preserves_response_cost() {
    let response = ModelChatResp {
        conversation_id: None,
        answer: "ok".to_owned(),
        route_id: "runtime.llm.code_agent".to_owned(),
        provider: "deep-seek".to_owned(),
        model: Some("deepseek-v4-flash".to_owned()),
        latency_ms: 42,
        usage: ModelChatUsage {
            prompt_tokens: Some(11),
            completion_tokens: Some(7),
            total_tokens: Some(18),
        },
        cost_cents: Some(0.65),
    };

    let payload = model_inference_event_payload(&response);

    assert_eq!(payload["item"]["costCents"], 0.65);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend-rust inference_cost --offline
```

Expected: FAIL because agent payload currently writes null.

**Step 3: Implement minimal code**

Set:

```rust
"costCents": response.cost_cents,
```

using a borrowed/serializable value that does not move from `response`.

**Step 4: Run tests**

Run:

```bash
cargo test -p backend-rust inference_cost --offline
cargo test -p backend-rust inference_spans --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: include model cost in inference spans"
```

### Task 4: Matrix And Verification

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update docs**

Update Rollout trace notes so DB route cost backfill is no longer listed as next. Keep provider retry/error spans and nested provider spans as next.

**Step 2: Verify**

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust model_chat_cost_cents --offline
cargo test -p backend-rust model_chat_response_cost_runtime --offline
cargo test -p backend-rust inference_cost --offline
cargo test -p backend-rust inference_spans --offline
cargo test -p novex-eval inference_spans --offline
cargo test --workspace --offline
```

Expected: all pass; `live_rag_e2e` may remain ignored unless POC infra is configured.

**Step 3: Commit and merge stage**

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: record inference cost trace progress"
git checkout main
git merge --no-ff feat/enterprise-agent-foundation
cargo fmt -- --check
cargo test --workspace --offline
```
