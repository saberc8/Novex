# Agent Inference Trace Spans Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Preserve model inference evidence in agent traces so rollout replay and eval can gate model latency, usage, provider, and route behavior.

**Architecture:** Add an inference event kind to `novex-trace`, expose provider in `ModelChatResp`, record metadata-only inference events in the backend model loop, map those events into trace bundles, and derive eval candidate tags from inference spans.

**Tech Stack:** Rust, serde/serde_json, existing backend RunEvent conversion, `novex-model` route summaries, Cargo offline tests.

---

### Task 1: Add Inference Trace Event

**Files:**
- Modify: `crates/novex-trace/src/lib.rs`

**Step 1: Write the failing test**

Add:

```rust
#[test]
fn trace_bundle_preserves_inference_span_events() {
    let bundle = TraceBundle::new("agent-1").with_event(TraceEvent::inference(
        1,
        json!({
            "routeId": "runtime.llm.code_agent",
            "provider": "deep-seek",
            "latencyMs": 42
        }),
    ));

    assert_eq!(bundle.events[0].kind, TraceEventKind::Inference);
    assert_eq!(bundle.events[0].payload["latencyMs"], 42);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p novex-trace inference_span --offline
```

Expected: FAIL because `TraceEvent::inference` and `TraceEventKind::Inference` do not exist.

**Step 3: Write minimal implementation**

Add:

```rust
Inference,
```

to `TraceEventKind` and:

```rust
pub fn inference(sequence_no: i32, payload: Value) -> Self {
    Self {
        sequence_no,
        kind: TraceEventKind::Inference,
        payload,
    }
}
```

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p novex-trace inference_span --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-trace/src/lib.rs
git commit -m "feat: add agent inference trace span"
```

### Task 2: Expose Model Provider In Chat Response

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write the failing test**

Update or add a model response test asserting:

```rust
assert_eq!(response.provider, "deep-seek");
```

for a route using `ModelProviderType::DeepSeek`.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend model_chat_response_extracts_answer_usage_and_route_summary --offline
```

Expected: FAIL because `ModelChatResp` does not expose provider.

**Step 3: Write minimal implementation**

Add `pub provider: String` to `ModelChatResp` and set it in `model_chat_response_from_provider`:

```rust
provider: route.provider().as_str().to_owned(),
```

Update existing test literals with provider values.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p backend model_chat_response_extracts_answer_usage_and_route_summary model_chat_usage_record --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/model_service.rs
git commit -m "feat: expose model provider in chat responses"
```

### Task 3: Record And Map Backend Inference Events

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing trace conversion test**

Add:

```rust
#[test]
fn agent_run_events_convert_inference_spans_to_trace_bundle() {
    let events = vec![fake_agent_event(
        "thought",
        1,
        json!({
            "runtimeMode": "model_loop",
            "item": {
                "type": "model_inference",
                "routeId": "runtime.llm.code_agent",
                "provider": "deep-seek",
                "model": "deepseek-v4-flash",
                "latencyMs": 42,
                "usage": {"promptTokens": 11, "completionTokens": 7, "totalTokens": 18},
                "costCents": null
            }
        }),
    )];

    let bundle = agent_events_to_trace_bundle("agent-1", events);

    assert_eq!(bundle.events[0].kind, TraceEventKind::Inference);
    assert_eq!(bundle.events[0].payload["item"]["routeId"], "runtime.llm.code_agent");
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p backend inference_spans --offline
```

Expected: FAIL until the mapping exists.

**Step 3: Implement mapping and payload builder**

Add a helper:

```rust
fn model_inference_event_payload(response: &ModelChatResp) -> Value
```

that serializes metadata only. Add a `thought` match guard before the ordinary thought mapping:

```rust
"thought" if trace_payload_item_type(&event.payload).as_deref() == Some("model_inference") => {
    Some(TraceEvent::inference(sequence_no, event.payload.clone()))
}
```

After each completed model response in `create_model_loop_run`, append a `RunEventKind::Thought` with `model_inference_event_payload(&model_response)`.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p backend inference_spans --offline
cargo test -p backend model_loop --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: record agent model inference spans"
```

### Task 4: Add Eval Tags From Inference Spans

**Files:**
- Modify: `crates/novex-eval/src/lib.rs`

**Step 1: Write failing test**

Add:

```rust
#[test]
fn trace_eval_candidate_tags_inference_spans() {
    let bundle = TraceBundle::new("agent-1")
        .with_event(TraceEvent::inference(
            1,
            json!({
                "item": {
                    "type": "model_inference",
                    "routeId": "runtime.llm.code_agent",
                    "provider": "deep-seek",
                    "model": "deepseek-v4-flash",
                    "latencyMs": 42,
                    "usage": {"promptTokens": 11, "completionTokens": 7, "totalTokens": 18},
                    "costCents": 0.65
                }
            }),
        ))
        .with_event(TraceEvent::inference(
            2,
            json!({
                "item": {
                    "type": "model_inference",
                    "latencyMs": 8,
                    "usage": {"promptTokens": 3, "completionTokens": 2, "totalTokens": 5}
                }
            }),
        ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["inferenceCount"], 2);
    assert_eq!(candidate.tags["modelProvider"], "deep-seek");
    assert_eq!(candidate.tags["modelRouteId"], "runtime.llm.code_agent");
    assert_eq!(candidate.tags["modelName"], "deepseek-v4-flash");
    assert_eq!(candidate.tags["latencyMs"], 50);
    assert_eq!(candidate.tags["promptTokens"], 14);
    assert_eq!(candidate.tags["completionTokens"], 9);
    assert_eq!(candidate.tags["totalTokens"], 23);
    assert_eq!(candidate.tags["costCents"], 0.65);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p novex-eval inference_spans --offline
```

Expected: FAIL until eval tags are implemented.

**Step 3: Implement aggregation helpers**

Add helpers that read inference payload fields from either the payload root or `payload.item`, sum latency/tokens/cost, and preserve first route/provider/model values.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p novex-eval inference_spans --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-eval/src/lib.rs
git commit -m "feat: tag eval candidates with inference spans"
```

### Task 5: Matrix And Verification

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update docs**

Update Rollout trace notes to mention inference span preservation for route/provider/model/latency/usage and optional cost extraction. Add this plan to follow-up implementation plans.

**Step 2: Verify**

Run:

```bash
cargo fmt -- --check
cargo test -p novex-trace inference_span --offline
cargo test -p backend inference_spans --offline
cargo test -p novex-eval inference_spans --offline
cargo test -p backend model_chat_response_extracts_answer_usage_and_route_summary model_chat_usage_record --offline
cargo test --workspace --offline
```

Expected: all pass; `live_rag_e2e` may remain ignored unless POC infra is configured.

**Step 3: Commit**

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: record inference trace span progress"
```
