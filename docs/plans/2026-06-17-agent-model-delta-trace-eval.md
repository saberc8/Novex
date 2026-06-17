# Agent Model Delta Trace/Eval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make persisted CodeAgent `model_delta` run events visible in trace replay and eval candidate tags.

**Architecture:** Keep `TraceEventKind::Inference` as the model-provider evidence bucket. Extend the existing run-event-to-trace filter to include `model_delta`, then extend `novex-eval` inference summary aggregation with model-delta counts and text length.

**Tech Stack:** Rust, `backend-rust`, `novex-trace`, `novex-eval`, existing `ai_run_event` payloads.

## Global Constraints

- Do not add database schema in this slice.
- Do not change SSE/WebSocket event transport in this slice.
- Do not add frontend rendering in this slice.
- Do not treat delta chunks as final answers.
- Preserve existing `model_inference` and `model_inference_error` trace/eval tags.

---

### Task 1: Trace Preserves Model Delta Events

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Interfaces:**
- Consumes: `RunEventRecord` with `event_type = "thought"` and payload `item.type = "model_delta"`.
- Produces: `TraceEventKind::Inference` preserving the original payload.

- [ ] **Step 1: Write the failing test**

Add `agent_run_events_convert_model_delta_spans_to_trace_bundle` near the existing inference trace tests. It should create a fake thought event with:

```json
{
  "runtimeMode": "model_loop",
  "item": {
    "type": "model_delta",
    "source": "provider_stream",
    "routeId": "runtime.llm.code_agent",
    "provider": "openai-compatible",
    "model": "gpt-compatible",
    "deltaIndex": 1,
    "content": " world",
    "providerEvent": "chat.completion.chunk"
  }
}
```

Assert the trace event kind is `Inference` and payload fields are preserved.

- [ ] **Step 2: Run red verification**

Run:

```bash
cargo test -p backend-rust model_delta_spans --offline
```

Expected: FAIL because `model_delta` is not yet an inference trace item.

- [ ] **Step 3: Implement minimal trace conversion**

Extend `is_model_inference_trace_item` to include `model_delta`.

- [ ] **Step 4: Run green verification**

Run:

```bash
cargo test -p backend-rust model_delta_spans --offline
```

Expected: PASS.

### Task 2: Eval Tags Streaming Delta Evidence

**Files:**
- Modify: `crates/novex-eval/src/lib.rs`

**Interfaces:**
- Consumes: `TraceEventKind::Inference` payloads with `item.type = "model_delta"`.
- Produces: eval candidate tags `modelDeltaCount`, `modelDeltaTextLength`, and `streamingModelOutput`.

- [ ] **Step 1: Write the failing test**

Add `trace_eval_candidate_tags_model_delta_streaming` near the inference span tests. Build a bundle with two `TraceEvent::inference` delta events and one final `model_inference` event. Assert:

```rust
assert_eq!(candidate.tags["modelDeltaCount"], 2);
assert_eq!(candidate.tags["modelDeltaTextLength"], 11);
assert_eq!(candidate.tags["streamingModelOutput"], true);
assert_eq!(candidate.tags["inferenceCount"], 3);
```

- [ ] **Step 2: Run red verification**

Run:

```bash
cargo test -p novex-eval model_delta --offline
```

Expected: FAIL because the tags do not exist.

- [ ] **Step 3: Implement minimal eval aggregation**

Extend `TraceInferenceSummary` with delta count/text length fields. In `trace_inference_summary`, when `item.type == "model_delta"`, count the event and add `content.chars().count()`.

- [ ] **Step 4: Run green verification**

Run:

```bash
cargo test -p novex-eval model_delta --offline
```

Expected: PASS.

### Task 3: Matrix and Verification

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Updates Runtime loop and Rollout/trace/eval evidence with model-delta trace/eval preservation.

- [ ] **Step 1: Update migration matrix**

Move Runtime loop to the next slice number and add model-delta trace/eval preservation to the Rollout/trace/eval row.

- [ ] **Step 2: Run full verification**

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust model_delta_spans --offline
cargo test -p novex-eval model_delta --offline
cargo test --workspace --offline
cd apps/agent-workspace && pnpm test -- src/api/agent.test.ts && pnpm typecheck
cd ../codex-app-poc && pnpm test -- src/api/agent.test.ts && pnpm typecheck
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 3: Commit and integrate**

Commit the feature branch, merge it to `main` with `--no-ff`, verify on `main`, run `cargo clean` in both worktrees, and fast-forward the feature branch to `main`.
