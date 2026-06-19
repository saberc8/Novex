# Agent Tool Call Batch Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow one model turn to request multiple tool calls, plan them with `ToolBatchPlan`, and record batch scheduling metadata while keeping actual execution serial for this slice.

**Architecture:** `novex-agent-runtime` owns parsing and budget helpers. `novex-tools` already owns routed-call batch planning. Backend model loop consumes `ParsedModelTurnOutput.items`, rejects over-budget batches before execution, records batch metadata, and then executes calls in deterministic order.

**Tech Stack:** Rust, serde_json, Cargo offline tests.

---

## Task 1: Runtime Batch Parser Contract

**Files:**
- Modify: `crates/novex-agent-runtime/src/lib.rs`

**Step 1: Write failing parser tests**

Add tests:

```rust
#[test]
fn parser_reads_json_tool_call_batch_from_model_answer() {
    let parsed = parse_model_turn_output(
        r#"{"type":"tool_calls","calls":[{"callId":"call-1","toolCode":"rag.search","arguments":{"query":"policy"}},{"callId":"call-2","toolCode":"github.repo.read","arguments":{"repository":"org/repo","path":"README.md"}}]}"#,
    )
    .unwrap();

    assert_eq!(parsed.outcome, TurnOutcome::NeedsFollowUp);
    assert_eq!(parsed.items.len(), 2);
    assert_eq!(
        parsed.items[0],
        AgentTurnItem::tool_call("call-1", "rag.search", serde_json::json!({"query":"policy"}))
    );
    assert_eq!(
        parsed.items[1],
        AgentTurnItem::tool_call(
            "call-2",
            "github.repo.read",
            serde_json::json!({"repository":"org/repo","path":"README.md"})
        )
    );
}

#[test]
fn parser_rejects_empty_tool_call_batch() {
    let err = parse_model_turn_output(r#"{"type":"tool_calls","calls":[]}"#).unwrap_err();

    assert_eq!(err.message, "tool_calls requires at least one call");
}

#[test]
fn runtime_budget_reports_remaining_tool_call_capacity() {
    let mut state = AgentRuntimeState::with_budget(
        "run-1",
        AgentRuntimeBudget {
            max_turns: 4,
            max_tool_calls: 3,
            compact_after_observations: None,
        },
    );
    state.push_item(AgentTurnItem::tool_call("call-1", "rag.search", serde_json::json!({})));

    assert_eq!(state.remaining_tool_call_budget(), 2);
    assert!(state.can_execute_tool_calls(2));
    assert!(!state.can_execute_tool_calls(3));
}
```

Run:

```bash
cargo test -p novex-agent-runtime parser_reads_json_tool_call_batch_from_model_answer --offline
```

Expected: FAIL because `ParsedModelTurnOutput.items` and batch parsing do not exist.

**Step 2: Implement parser and budget helpers**

Add `items: Vec<AgentTurnItem>` to `ParsedModelTurnOutput`.

Add helpers:

```rust
pub fn remaining_tool_call_budget(&self) -> usize
pub fn can_execute_tool_calls(&self, requested: usize) -> bool
```

Update `parse_model_turn_output` so final answers and single tool calls populate both `item` and `items`. Add support for `{"type":"tool_calls","calls":[...]}`.

**Step 3: Verify and commit**

Run:

```bash
cargo test -p novex-agent-runtime --offline
cargo fmt -- --check
```

Commit:

```bash
git add crates/novex-agent-runtime/src/lib.rs docs/plans/2026-06-17-agent-tool-call-batch-design.md docs/plans/2026-06-17-agent-tool-call-batch.md
git commit -m "feat: parse agent tool call batches"
```

## Task 2: Backend Batch Plan Visibility

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Write failing backend tests**

Add source-level tests:

```rust
#[test]
fn model_loop_prompt_advertises_tool_call_batches() {
    let prompt = build_model_loop_system_prompt(&["rag.search".to_owned(), "github.repo.read".to_owned()]);

    assert!(prompt.contains("\"type\":\"tool_calls\""));
    assert!(prompt.contains("\"calls\""));
}

#[test]
fn agent_service_model_loop_plans_parsed_tool_call_batches() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("parsed.items"));
    assert!(source.contains("ToolBatchPlan::from_routed_calls"));
    assert!(source.contains("\"batchExecutionMode\""));
    assert!(source.contains("\"toolCallBatch\""));
}
```

Run:

```bash
cargo test -p backend model_loop_prompt_advertises_tool_call_batches --offline
```

Expected: FAIL because prompt and backend loop do not mention batches.

**Step 2: Add backend batch plan handling**

Import `ToolBatchPlan`.

In model loop:

- read `parsed.items`.
- collect tool calls into a batch when every parsed item is `ToolCall`.
- reject batch size greater than `runtime_state.remaining_tool_call_budget()`.
- route every call with `tool_router.route_tool_call`.
- build `ToolBatchPlan::from_routed_calls`.
- append `batchExecutionMode`, `serialReason`, and `toolCallBatch` metadata to each `ActionSelected` event.
- execute in deterministic order for this slice.

Update the prompt to show both single-call and batch-call JSON shapes.

**Step 3: Update matrix**

Update Runtime loop and Parallel tools rows to say batch parsing and batch plan event visibility exist, but true async execution remains next.

**Step 4: Verify and commit**

Run:

```bash
cargo test -p backend model_loop --offline
cargo test -p backend agent_service --offline
cargo fmt -- --check
```

Commit:

```bash
git add backend/src/application/ai/agent_service.rs docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "feat: plan agent tool call batches"
```

## Task 3: Final Verification

Run:

```bash
cargo fmt -- --check
cargo test -p novex-agent-runtime --offline
cargo test -p novex-tools --offline
cargo test -p backend model_loop --offline
cargo test -p backend agent_service --offline
cargo test --workspace --offline
git status --short
```

Expected: all commands pass; only live RAG E2E may remain ignored because it requires external infra.
