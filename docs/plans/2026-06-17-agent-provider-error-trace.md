# Agent Provider Error Trace Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Preserve model provider failure evidence in Agent traces and eval tags.

**Architecture:** Keep successful `model_inference` spans unchanged. Add a structured `model_inference_error` payload emitted from the backend Agent model loop when the configured Code Agent model call returns an error. Map it as an inference trace event, append a normal error event for failed replay status, and extend `novex-eval` inference aggregation with error counts and classifications.

**Tech Stack:** Rust, serde_json, existing backend Run Graph events, `novex-trace`, `novex-eval`, Cargo offline tests.

---

### Task 1: Preserve Provider Error Inference Events In Trace Conversion

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write the failing test**

Add a test:

```rust
#[test]
fn agent_run_events_convert_provider_error_spans_to_trace_bundle() {
    let events = vec![fake_agent_event(
        "thought",
        1,
        json!({
            "runtimeMode": "model_loop",
            "item": {
                "type": "model_inference_error",
                "routeId": "runtime.llm.code_agent",
                "routePurpose": "code_agent",
                "attempt": 1,
                "maxAttempts": 1,
                "retryable": true,
                "errorKind": "provider_http",
                "httpStatus": 502,
                "message": "LLM model call failed: HTTP 502",
                "latencyMs": 12
            }
        }),
    )];

    let bundle = agent_events_to_trace_bundle("agent-1", events);

    assert_eq!(bundle.events[0].kind, TraceEventKind::Inference);
    assert_eq!(bundle.events[0].payload["item"]["type"], "model_inference_error");
    assert_eq!(bundle.events[0].payload["item"]["httpStatus"], 502);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend provider_error_spans --offline
```

Expected: FAIL because trace conversion only treats `model_inference` as an inference event.

**Step 3: Implement minimal code**

Update the thought guard so both `model_inference` and `model_inference_error` map to `TraceEvent::inference`.

**Step 4: Run test**

Run:

```bash
cargo test -p backend provider_error_spans --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: map provider error spans to agent traces"
```

### Task 2: Add Eval Tags For Provider Error Spans

**Files:**
- Modify: `crates/novex-eval/src/lib.rs`

**Step 1: Write the failing test**

Add a test:

```rust
#[test]
fn trace_eval_candidate_tags_provider_error_spans() {
    let bundle = TraceBundle::new("agent-1").with_event(TraceEvent::inference(
        1,
        json!({
            "item": {
                "type": "model_inference_error",
                "routeId": "runtime.llm.code_agent",
                "provider": "deep-seek",
                "errorKind": "provider_http",
                "httpStatus": 502,
                "retryable": true,
                "latencyMs": 12
            }
        }),
    ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["inferenceCount"], 1);
    assert_eq!(candidate.tags["inferenceErrorCount"], 1);
    assert_eq!(candidate.tags["retryableInferenceErrorCount"], 1);
    assert_eq!(candidate.tags["modelErrorKind"], "provider_http");
    assert_eq!(candidate.tags["modelHttpStatus"], 502);
    assert_eq!(candidate.tags["latencyMs"], 12);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p novex-eval provider_error_spans --offline
```

Expected: FAIL because eval aggregation ignores provider error metadata.

**Step 3: Implement minimal code**

Extend `TraceInferenceSummary` with error counters and first error metadata. In `trace_inference_summary`, inspect the normalized inference payload:

- increment `error_count` when `type == "model_inference_error"`
- increment `retryable_error_count` when `retryable == true`
- capture first `errorKind` and `httpStatus`
- keep summing `latencyMs`

Insert tags only when error metadata exists.

**Step 4: Run test**

Run:

```bash
cargo test -p novex-eval provider_error_spans --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-eval/src/lib.rs
git commit -m "feat: tag eval candidates with provider errors"
```

### Task 3: Record Model Call Failures From The Agent Loop

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write the failing tests**

Add pure tests for payload and classification:

```rust
#[test]
fn model_inference_error_event_payload_classifies_retryable_http_errors() {
    let payload = model_inference_error_event_payload(
        &AppError::bad_request("LLM 模型调用失败: HTTP 502"),
        12,
    );

    assert_eq!(payload["item"]["type"], "model_inference_error");
    assert_eq!(payload["item"]["routeId"], "runtime.llm.code_agent");
    assert_eq!(payload["item"]["errorKind"], "provider_http");
    assert_eq!(payload["item"]["httpStatus"], 502);
    assert_eq!(payload["item"]["retryable"], true);
    assert_eq!(payload["item"]["latencyMs"], 12);
}
```

Add a source-contract test asserting the model-loop error path records the payload and finishes the run as failed:

```rust
#[test]
fn agent_service_model_loop_records_provider_error_spans() {
    let source = include_str!("agent_service.rs");

    assert!(source.contains("model_inference_error_event_payload(&err"));
    assert!(source.contains("RunEventKind::Error"));
    assert!(source.contains("\"model_inference_error\""));
    assert!(source.contains("stopReason\": \"model_call_failed"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend provider_error --offline
```

Expected: FAIL because payload helper and error path do not exist.

**Step 3: Implement minimal code**

Add:

```rust
fn model_inference_error_event_payload(error: &AppError, latency_ms: u128) -> Value
```

and small helpers to classify HTTP status/retryability. In `create_model_loop_run`, record `Instant::now()` before awaiting the model call. If the await returns `Err(err)`, append:

1. `RunEventKind::Thought` with `model_inference_error_event_payload(&err, elapsed)`.
2. `RunEventKind::Error` with `stopReason: "model_call_failed"`.
3. Finish the run as `RunStatus::Failed`.
4. Refresh the trace snapshot and return `get_run`.

**Step 4: Run tests**

Run:

```bash
cargo test -p backend provider_error --offline
cargo test -p backend inference_spans --offline
cargo test -p backend model_loop --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: record agent provider error spans"
```

### Task 4: Update Matrix And Verify

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update matrix**

Move Rollout trace to the next slice and note provider error evidence is in place; provider-native retry/fallback and nested lifecycle remain follow-ups.

**Step 2: Run focused verification**

Run:

```bash
cargo fmt -- --check
cargo test -p backend provider_error --offline
cargo test -p backend inference_spans --offline
cargo test -p backend model_loop --offline
cargo test -p novex-eval provider_error_spans --offline
```

Expected: PASS.

**Step 3: Run full verification**

Run:

```bash
cargo test --workspace --offline
```

Expected: PASS, with `live_rag_e2e` ignored unless full POC infra is present.

**Step 4: Commit**

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: record provider error trace progress"
```

**Step 5: Merge stage to main**

Run from the main worktree:

```bash
git merge --no-ff feat/enterprise-agent-foundation -m "merge: enterprise agent foundation provider error trace"
cargo fmt -- --check
cargo test --workspace --offline
```

Then fast-forward the feature worktree back to `main`.
