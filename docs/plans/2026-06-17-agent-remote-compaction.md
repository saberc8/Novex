# Agent Remote Compaction Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Codex remote-compaction endpoint contract to Novex model-loop compaction, including runtime request/checkpoint metadata, backend event payloads, and eval tags.

**Architecture:** `novex-agent-runtime` owns the deterministic remote compaction request shape. `AgentService` adapts that request into the existing configured `CodeAgent` model compaction call and persists serialized request evidence in the compaction event. `novex-eval` extracts remote implementation tags from trace bundles.

**Tech Stack:** Rust, serde, serde_json, existing model-loop backend service, Cargo offline tests.

---

### Task 1: Runtime Remote Compaction Request Contract

**Files:**
- Modify: `crates/novex-agent-runtime/src/lib.rs`

**Step 1: Write failing tests**

Add:

```rust
#[test]
fn runtime_remote_compaction_request_exposes_endpoint_metadata() {
    let budget = AgentRuntimeBudget {
        max_turns: 8,
        max_tool_calls: 4,
        compact_after_observations: Some(1),
    };
    let mut state = AgentRuntimeState::with_budget("run-1", budget);
    state.push_item(AgentTurnItem::user_message("find refund policy"));
    state.push_item(AgentTurnItem::tool_call(
        "call-1",
        "rag.search",
        json!({"query":"refund"}),
    ));
    state.push_item(AgentTurnItem::tool_observation(
        "call-1",
        ToolObservationStatus::Succeeded,
        json!({"hits":[{"text":"refund within 7 days"}]}),
    ));

    let request = state
        .remote_compaction_request(vec!["rag.search".to_owned(), "github.repo.read".to_owned()])
        .unwrap();

    assert_eq!(request.window_id, 1);
    assert_eq!(
        request.implementation,
        AgentRemoteCompactionImplementation::ResponsesCompactionV2
    );
    assert_eq!(request.trigger, AgentCompactionTrigger::Auto);
    assert_eq!(request.reason, AgentCompactionReason::ObservationThreshold);
    assert_eq!(request.phase, AgentCompactionPhase::ModelLoopFollowUp);
    assert_eq!(request.compacted_item_count, 3);
    assert_eq!(request.retained_item_count, 1);
    assert_eq!(request.tool_codes, vec!["rag.search", "github.repo.read"]);
}

#[test]
fn runtime_remote_compaction_request_retains_user_and_previous_summary() {
    let budget = AgentRuntimeBudget {
        max_turns: 8,
        max_tool_calls: 4,
        compact_after_observations: Some(1),
    };
    let mut state = AgentRuntimeState::with_budget("run-1", budget);
    state.push_item(AgentTurnItem::ContextCompaction {
        summary: "previous compacted context".to_owned(),
    });
    state.push_item(AgentTurnItem::user_message("continue"));
    state.push_item(AgentTurnItem::tool_observation(
        "call-2",
        ToolObservationStatus::Succeeded,
        json!({"text":"new evidence"}),
    ));

    let request = state.remote_compaction_request(vec![]).unwrap();

    assert!(request
        .retained_history
        .iter()
        .any(|item| matches!(item, AgentTurnItem::UserMessage { .. })));
    assert!(!request
        .retained_history
        .iter()
        .any(|item| matches!(item, AgentTurnItem::ToolObservation { .. })));
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test -p novex-agent-runtime remote_compaction --offline
```

Expected: FAIL because remote compaction request types and method do not exist.

**Step 3: Implement minimal runtime contract**

Add enums:

- `AgentRemoteCompactionImplementation`
- `AgentCompactionTrigger`
- `AgentCompactionReason`
- `AgentCompactionPhase`

Add `AgentRemoteCompactionRequest`.

Add:

```rust
pub fn remote_compaction_request(
    &self,
    tool_codes: Vec<String>,
) -> Option<AgentRemoteCompactionRequest>
```

Retain only `UserMessage` and `ContextCompaction` items from the current window.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p novex-agent-runtime remote_compaction --offline
cargo test -p novex-agent-runtime runtime_compaction --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-agent-runtime/src/lib.rs
git commit -m "feat: add remote compaction request contract"
```

### Task 2: Backend Remote Compaction Prompt And Event Payload

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add:

```rust
#[test]
fn model_loop_remote_compaction_prompt_includes_endpoint_metadata() {
    let request = test_remote_compaction_request();
    let messages = build_model_loop_remote_context_compaction_messages(
        "Find refund policy",
        "Observation for call-1: refund within 7 days",
        &["rag.search".to_owned()],
        Some(&request),
    );

    assert!(messages[0].content.contains("remote compaction endpoint adapter"));
    assert!(messages[1].content.contains("responses_compaction_v2"));
    assert!(messages[1].content.contains("observation_threshold"));
    assert!(messages[1].content.contains("inputHistoryCount"));
}

#[test]
fn agent_service_model_loop_records_remote_compaction_request() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("remote_compaction_request"));
    assert!(source.contains("\"remoteCompaction\""));
    assert!(source.contains("\"compactionImplementation\""));
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend-rust remote_compaction --offline
```

Expected: FAIL until the remote prompt builder and event payload wiring exist.

**Step 3: Implement backend adapter**

- Import `AgentRemoteCompactionRequest`.
- Add `build_model_loop_remote_context_compaction_messages(...)`.
- Keep `build_model_loop_context_compaction_messages(...)` as a compatibility wrapper.
- Update `model_loop_context_compaction_outcome(...)` to accept `Option<&AgentRemoteCompactionRequest>` and use the remote prompt builder.
- In the model-loop compaction branch, build `runtime_state.remote_compaction_request(tool_codes.clone())`.
- Insert serialized `remoteCompaction` and `compactionImplementation` into `compaction_payload`.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p backend-rust remote_compaction --offline
cargo test -p backend-rust model_loop_compaction --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: record remote compaction endpoint metadata"
```

### Task 3: Eval Tags For Remote Compaction

**Files:**
- Modify: `crates/novex-eval/src/lib.rs`

**Step 1: Write failing test**

Add:

```rust
#[test]
fn remote_compaction_trace_eval_candidate_tags_endpoint_contract() {
    let bundle = TraceBundle::new("trace-remote-compact")
        .with_event(TraceEvent::context_compaction(
            1,
            json!({
                "compactionStrategy": "model",
                "compactionStatus": "succeeded",
                "compactionImplementation": "responses_compaction_v2",
                "remoteCompaction": {
                    "implementation": "responses_compaction_v2",
                    "trigger": "auto",
                    "reason": "observation_threshold"
                }
            }),
        ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["remoteCompactionCount"], 1);
    assert_eq!(candidate.tags["compactionImplementation"], "responses_compaction_v2");
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test -p novex-eval remote_compaction --offline
```

Expected: FAIL until eval extraction reads remote compaction metadata.

**Step 3: Implement extraction**

Extend `TraceCompactionSummary` with:

- `remote_count`
- `implementation`

Increment remote count when `remoteCompaction` exists or `compactionImplementation` is `responses_compaction_v2`.

Insert tags:

- `remoteCompactionCount`
- `compactionImplementation`

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p novex-eval remote_compaction --offline
cargo test -p novex-eval compaction --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-eval/src/lib.rs
git commit -m "feat: tag remote compaction evidence"
```

### Task 4: Matrix Update And Merge Verification

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update docs**

Update Runtime loop and Rollout trace rows to mention:

- remote compaction endpoint contract,
- retained-history/checkpoint request metadata,
- eval tags for remote compaction,
- provider-native remote compact transport remains next.

Add this plan to follow-up implementation plans.

**Step 2: Verify**

Run:

```bash
cargo fmt -- --check
cargo test -p novex-agent-runtime remote_compaction --offline
cargo test -p backend-rust remote_compaction --offline
cargo test -p backend-rust model_loop_compaction --offline
cargo test -p novex-eval remote_compaction --offline
cargo test --workspace --offline
```

Expected: all pass.

**Step 3: Commit**

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-remote-compaction-design.md docs/plans/2026-06-17-agent-remote-compaction.md
git commit -m "docs: record remote compaction contract progress"
```

**Step 4: Merge**

No-ff merge the feature worktree back into local `main`, rerun `cargo fmt -- --check` and `cargo test --workspace --offline` on `main`, then fast-forward the preserved feature worktree.
