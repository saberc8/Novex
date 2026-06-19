# Agent Context Compaction Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add the first Codex-shaped context compaction slice to `runtimeMode=model_loop`: a budgeted loop can compact prior tool-observation history, record a `ContextCompaction` turn item, and continue sampling with a shorter model input.

**Architecture:** Keep full remote/model compaction deferred, but introduce the semantic boundary Codex relies on: compaction window accounting, summary item, replacement history, and run-event traceability. `novex-agent-runtime` owns deterministic compaction policy and summary construction; `AgentService` triggers it inside the configured-model loop and persists it through existing Run Graph events.

**Tech Stack:** Rust, serde, serde_json, Axum application service tests, Cargo offline tests.

---

## Codex Reference

- `codex-rs/core/src/compact.rs`: compaction turn item lifecycle, compacted summary, replacement history install.
- `codex-rs/core/src/compact_remote.rs`: trace checkpoint and remote compact replacement semantics.
- `codex-rs/core/src/state/auto_compact_window.rs`: window id and prefill accounting.

## Scope

- Add a runtime-level deterministic compaction contract.
- Trigger compaction inside `AgentService::create_model_loop_run` after enough tool observations have accumulated.
- Persist a `ContextCompaction` item as a Run Graph event before the next model call.
- Replace the following model input with system prompt, original user request, and compacted summary.

Deferred:

- Calling a dedicated compaction model route.
- Rich token accounting from provider usage.
- Remote compact endpoint parity.
- Full rollout trace checkpoint schema.

### Task 1: Runtime Compaction Contract

**Files:**
- Modify: `crates/novex-agent-runtime/src/lib.rs`

**Step 1: Write failing runtime tests**

Add tests:

```rust
#[test]
fn runtime_compaction_is_needed_after_observation_threshold() {
    let budget = AgentRuntimeBudget {
        max_turns: 8,
        max_tool_calls: 4,
        compact_after_observations: Some(2),
    };
    let mut state = AgentRuntimeState::with_budget("run-1", budget);
    state.push_item(AgentTurnItem::user_message("find policy"));
    state.push_item(AgentTurnItem::tool_observation(
        "call-1",
        ToolObservationStatus::Succeeded,
        json!({"hits":[{"title":"A"}]}),
    ));
    assert!(!state.should_compact_context());
    state.push_item(AgentTurnItem::tool_observation(
        "call-2",
        ToolObservationStatus::Succeeded,
        json!({"hits":[{"title":"B"}]}),
    ));
    assert!(state.should_compact_context());
}

#[test]
fn runtime_compaction_pushes_summary_and_advances_window() {
    let budget = AgentRuntimeBudget {
        max_turns: 8,
        max_tool_calls: 4,
        compact_after_observations: Some(1),
    };
    let mut state = AgentRuntimeState::with_budget("run-1", budget);
    state.push_item(AgentTurnItem::user_message("find policy"));
    state.push_item(AgentTurnItem::tool_call("call-1", "rag.search", json!({"query":"policy"})));
    state.push_item(AgentTurnItem::tool_observation(
        "call-1",
        ToolObservationStatus::Succeeded,
        json!({"hits":[{"citation":"doc#1","text":"refund within 7 days"}]}),
    ));

    let compaction = state.compact_context().unwrap();

    assert_eq!(compaction.window_id, 1);
    assert!(compaction.summary.contains("refund within 7 days"));
    assert!(!state.should_compact_context());
    assert!(matches!(state.items.last(), Some(AgentTurnItem::ContextCompaction { .. })));
}
```

Run:

```bash
cargo test -p novex-agent-runtime runtime_compaction --offline
```

Expected: FAIL because the budget field, compaction method, and return type do not exist.

**Step 2: Implement minimal runtime support**

Add:

```rust
pub compact_after_observations: Option<usize>,
```

to `AgentRuntimeBudget`, defaulting to `None`.

Add `AgentContextCompaction`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContextCompaction {
    pub window_id: u64,
    pub summary: String,
    pub retained_item_count: usize,
    pub compacted_item_count: usize,
}
```

Add `compaction_window_id: u64` to `AgentRuntimeState`.

Implement:

```rust
pub fn should_compact_context(&self) -> bool
pub fn compact_context(&mut self) -> Option<AgentContextCompaction>
```

The minimal summary should be deterministic and include recent user message, tool calls, observations, and any existing compaction summaries. It must append a `ContextCompaction` item and advance `compaction_window_id`.

**Step 3: Verify runtime package**

Run:

```bash
cargo test -p novex-agent-runtime --offline
cargo fmt -- --check
```

Expected: all runtime tests pass and formatting is clean.

**Step 4: Commit**

```bash
git add crates/novex-agent-runtime/src/lib.rs docs/plans/2026-06-17-agent-context-compaction.md
git commit -m "feat: add agent context compaction contract"
```

### Task 2: Backend Model Loop Compaction

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Write failing backend tests**

Add source-level tests:

```rust
#[test]
fn agent_service_model_loop_records_context_compaction_event() {
    let source = include_str!("agent_service.rs");

    assert!(source.contains("runtime_state.should_compact_context()"));
    assert!(source.contains("runtime_state.compact_context()"));
    assert!(source.contains("AgentTurnItem::ContextCompaction"));
    assert!(source.contains("\"compactionWindowId\""));
}

#[test]
fn agent_service_model_loop_uses_compacted_messages_for_next_sample() {
    let source = include_str!("agent_service.rs");

    assert!(source.contains("build_compacted_model_loop_messages"));
    assert!(source.contains("messages = build_compacted_model_loop_messages"));
}
```

Run:

```bash
cargo test -p backend agent_service_model_loop_records_context_compaction_event --offline
```

Expected: FAIL because the backend does not trigger or persist compaction.

**Step 2: Enable compact threshold from task budget**

Map `TaskBudget` into `AgentRuntimeBudget` with:

```rust
compact_after_observations: Some(2)
```

for model-loop POC. Keep existing turn/tool limits unchanged.

**Step 3: Trigger compaction after observation**

After appending the `Observation` event and before pushing the next model messages:

```rust
if runtime_state.should_compact_context() {
    if let Some(compaction) = runtime_state.compact_context() {
        let compaction_item = AgentTurnItem::ContextCompaction {
            summary: compaction.summary.clone(),
        };
        let mut compaction_payload = agent_turn_item_event_payload(&compaction_item);
        // insert runtimeMode, compactionWindowId, retainedItemCount, compactedItemCount
        self.append_event(... RunEventKind::Observation ..., compaction_payload).await?;
        messages = build_compacted_model_loop_messages(&command.input, &compaction.summary);
        continue;
    }
}
```

Use `RunEventKind::Observation` for the first slice to avoid adding a new enum/storage migration.

**Step 4: Add message builder**

Add:

```rust
fn build_compacted_model_loop_messages(original_input: &str, summary: &str) -> Vec<ModelChatMessage>
```

It returns system prompt, original user input, and one compacted context user message instructing the model to continue from the summary.

**Step 5: Update matrix**

Change the runtime loop row to state that deterministic context compaction slice is implemented and remote/model compaction remains next. Change the verification command to the real cargo filter:

```bash
cargo test -p backend model_loop --offline
```

**Step 6: Verify backend package**

Run:

```bash
cargo test -p backend model_loop --offline
cargo test -p backend agent_service --offline
cargo fmt -- --check
```

Expected: all selected tests pass and formatting is clean.

**Step 7: Commit**

```bash
git add backend/src/application/ai/agent_service.rs docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "feat: compact agent model loop context"
```

### Task 3: Final Verification

**Files:**
- Verify only.

**Step 1: Run complete verification**

Run:

```bash
cargo fmt -- --check
cargo test -p novex-agent-runtime --offline
cargo test -p backend model_loop --offline
cargo test -p backend agent_service --offline
cargo test --workspace --offline
git status --short
```

Expected: formatting clean, tests pass, and worktree clean.
