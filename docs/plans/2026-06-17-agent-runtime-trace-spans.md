# Agent Runtime Trace Spans Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Preserve retrieval, action-selection, context-compaction, and cancellation runtime evidence in `TraceBundle` so rollout replay and eval can reason about real agent control flow.

**Architecture:** Extend `crates/novex-trace` with additional event kinds and constructors, then adapt backend `RunEventRecord -> TraceEvent` conversion. Add light eval tags derived from those trace events without changing persistence schema.

**Tech Stack:** Rust, serde/serde_json, existing backend trace conversion, Cargo offline tests.

---

### Task 1: Add Runtime Trace Event Kinds

**Files:**
- Modify: `crates/novex-trace/src/lib.rs`

**Step 1: Write failing test**

Add:

```rust
#[test]
fn trace_bundle_preserves_runtime_span_events() {
    let bundle = TraceBundle::new("agent-1")
        .with_event(TraceEvent::retrieval(1, json!({"hitCount":2})))
        .with_event(TraceEvent::action_selected(2, json!({"toolCallBatch":[{"toolCode":"rag.search"}]})))
        .with_event(TraceEvent::context_compaction(3, json!({"compactedItemCount":4})))
        .with_event(TraceEvent::cancellation(4, json!({"cancelReason":"external_cancel"})));

    assert_eq!(bundle.events[0].kind, TraceEventKind::Retrieval);
    assert_eq!(bundle.events[1].kind, TraceEventKind::ActionSelected);
    assert_eq!(bundle.events[2].kind, TraceEventKind::ContextCompaction);
    assert_eq!(bundle.events[3].kind, TraceEventKind::Cancellation);
    assert_eq!(bundle.replay_summary().final_status, "cancelled");
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test -p novex-trace runtime_span --offline
```

Expected: FAIL because the new event kinds/constructors do not exist.

**Step 3: Implement minimal trace events**

Add enum variants and constructors:

- `Retrieval`
- `ActionSelected`
- `ContextCompaction`
- `Cancellation`

Update `TraceBundle::replay_summary` so a cancellation event without final answer reports `final_status="cancelled"`.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p novex-trace runtime_span --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-trace/src/lib.rs
git commit -m "feat: add agent runtime trace spans"
```

### Task 2: Map Backend Run Events To Runtime Spans

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing test**

Add:

```rust
#[test]
fn agent_run_events_convert_runtime_spans_to_trace_bundle() {
    let events = vec![
        fake_agent_event("retrieval", 1, json!({"hitCount":2,"source":"ai_memory"})),
        fake_agent_event("action_selected", 2, json!({"toolCallBatch":[{"toolCode":"rag.search"}]})),
        fake_agent_event(
            "observation",
            3,
            json!({"item":{"type":"context_compaction","summary":"older tool results compacted"},"compactedItemCount":4}),
        ),
        fake_agent_event("cancelled", 4, json!({"cancelReason":"external_cancel"})),
    ];

    let bundle = agent_events_to_trace_bundle("agent-1", events);

    assert!(bundle.events.iter().any(|event| event.kind == TraceEventKind::Retrieval));
    assert!(bundle.events.iter().any(|event| event.kind == TraceEventKind::ActionSelected));
    assert!(bundle.events.iter().any(|event| event.kind == TraceEventKind::ContextCompaction));
    assert!(bundle.events.iter().any(|event| event.kind == TraceEventKind::Cancellation));
    assert_eq!(bundle.replay_summary().final_status, "cancelled");
}
```

Import `TraceEventKind` if needed.

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend runtime_spans --offline
```

Expected: FAIL until mappings exist.

**Step 3: Implement mapping**

In `trace_event_from_run_event`:

- `retrieval` -> `TraceEvent::retrieval(sequence_no, event.payload.clone())`
- `action_selected` -> `TraceEvent::action_selected(sequence_no, event.payload.clone())`
- `cancel_requested` / `cancelled` -> `TraceEvent::cancellation(sequence_no, event.payload.clone())`
- `observation` checks `trace_payload_item_type(...) == Some("context_compaction")` and maps to `TraceEvent::context_compaction`.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p backend runtime_spans --offline
cargo test -p backend agent_run_events_convert_to_trace_bundle --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: map agent runtime events to trace spans"
```

### Task 3: Expose Runtime Span Tags To Eval Candidates

**Files:**
- Modify: `crates/novex-eval/src/lib.rs`

**Step 1: Write failing test**

Add:

```rust
#[test]
fn trace_eval_candidate_tags_runtime_spans() {
    let bundle = TraceBundle::new("agent-1")
        .with_event(TraceEvent::retrieval(1, json!({"hitCount":2})))
        .with_event(TraceEvent::context_compaction(2, json!({"compactedItemCount":4})))
        .with_event(TraceEvent::cancellation(3, json!({"cancelReason":"external_cancel"})));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["retrievalCount"], 1);
    assert_eq!(candidate.tags["compactionCount"], 1);
    assert_eq!(candidate.tags["cancelled"], true);
    assert_eq!(candidate.tags["cancelReason"], "external_cancel");
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test -p novex-eval runtime_spans --offline
```

Expected: FAIL until tags are implemented.

**Step 3: Implement tags**

Add helpers that count trace events by kind and read the first cancellation payload `cancelReason`. Insert tags into `EvalCaseCandidate::from_trace_bundle_with_policy`.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p novex-eval runtime_spans --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-eval/src/lib.rs
git commit -m "feat: tag eval candidates with runtime spans"
```

### Task 4: Matrix And Final Verification

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update docs**

Update Rollout trace notes from `slice-1 implemented` to include runtime span preservation for retrieval, action selection, compaction, and cancellation. Leave inference latency/cost and nested provider spans as next.

**Step 2: Verify**

Run:

```bash
cargo fmt -- --check
cargo test -p novex-trace runtime_span --offline
cargo test -p backend runtime_spans --offline
cargo test -p novex-eval runtime_spans --offline
cargo test -p backend agent_run_events_convert_to_trace_bundle --offline
cargo test -p backend eval_runtime --offline
cargo test --workspace --offline
```

Expected: all pass; `live_rag_e2e` may remain ignored unless POC infra is configured.

**Step 3: Commit**

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: record runtime trace span progress"
```

