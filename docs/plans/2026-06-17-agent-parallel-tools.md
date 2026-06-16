# Agent Parallel Tools Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Codex-shaped parallel tool policy contract to `novex-tools` and surface selected tool scheduling metadata in backend model-loop events.

**Architecture:** Keep actual backend tool execution serial in this slice. `novex-tools` owns the reusable concurrency policy and batch planner; `AgentService` records that policy for trace/eval/rollout so future multi-call parsing and async execution can use the same contract.

**Tech Stack:** Rust, serde, serde_json, Cargo offline tests.

---

## Task 1: Add Tool Concurrency Policy

**Files:**
- Modify: `crates/novex-tools/src/lib.rs`

**Step 1: Write failing tests**

Add tests:

```rust
#[test]
fn tool_router_reports_parallel_policy_for_read_only_tools() {
    let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions()).unwrap();

    let rag = router.tool_concurrency_policy("rag.search").unwrap();
    assert_eq!(rag.lock, ToolExecutionLock::Shared);
    assert!(rag.supports_parallel_calls);

    let media = router.tool_concurrency_policy("media.image.generate").unwrap();
    assert_eq!(media.lock, ToolExecutionLock::Exclusive);
    assert!(!media.supports_parallel_calls);
}

#[test]
fn tool_batch_plan_allows_parallel_read_only_calls() {
    let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions()).unwrap();
    let calls = vec![
        router.route_tool_call("call-1", "rag.search", serde_json::json!({"query":"policy"})).unwrap(),
        router.route_tool_call("call-2", "github.repo.read", serde_json::json!({"repository":"org/repo","path":"README.md"})).unwrap(),
    ];

    let plan = ToolBatchPlan::from_routed_calls(calls);

    assert_eq!(plan.mode, ToolBatchExecutionMode::Parallel);
    assert_eq!(plan.serial_reason, None);
}

#[test]
fn tool_batch_plan_serializes_non_parallel_calls() {
    let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions()).unwrap();
    let calls = vec![
        router.route_tool_call("call-1", "rag.search", serde_json::json!({"query":"policy"})).unwrap(),
        router.route_tool_call("call-2", "media.image.generate", serde_json::json!({"prompt":"poster"})).unwrap(),
    ];

    let plan = ToolBatchPlan::from_routed_calls(calls);

    assert_eq!(plan.mode, ToolBatchExecutionMode::Serial);
    assert_eq!(plan.serial_reason.as_deref(), Some("exclusive_tool:media.image.generate"));
}

#[test]
fn tool_batch_plan_serializes_duplicate_exclusive_groups() {
    let mut first = test_tool_definition("connector.write.one");
    first.concurrency = ToolConcurrencyPolicy::exclusive("connector:crm");
    let mut second = test_tool_definition("connector.write.two");
    second.concurrency = ToolConcurrencyPolicy::exclusive("connector:crm");
    let router = ToolRouter::from_definitions(vec![first, second]).unwrap();
    let calls = vec![
        router.route_tool_call("call-1", "connector.write.one", serde_json::json!({})).unwrap(),
        router.route_tool_call("call-2", "connector.write.two", serde_json::json!({})).unwrap(),
    ];

    let plan = ToolBatchPlan::from_routed_calls(calls);

    assert_eq!(plan.mode, ToolBatchExecutionMode::Serial);
    assert_eq!(plan.serial_reason.as_deref(), Some("exclusive_group:connector:crm"));
}
```

Run:

```bash
cargo test -p novex-tools tool_batch_plan --offline
```

Expected: FAIL because policy and batch plan types do not exist.

**Step 2: Implement minimal policy**

Add:

```rust
pub enum ToolExecutionLock { Shared, Exclusive }
pub struct ToolConcurrencyPolicy { ... }
pub enum ToolBatchExecutionMode { Parallel, Serial }
pub struct ToolBatchPlan { ... }
```

Add `concurrency: ToolConcurrencyPolicy` to `ToolDefinition`.

Add router helper:

```rust
pub fn tool_concurrency_policy(&self, tool_code: &str) -> Option<&ToolConcurrencyPolicy>
```

Update all `ToolDefinition` literals.

**Step 3: Verify and commit**

Run:

```bash
cargo test -p novex-tools --offline
cargo fmt -- --check
```

Commit:

```bash
git add crates/novex-tools/src/lib.rs docs/plans/2026-06-17-agent-parallel-tools-design.md docs/plans/2026-06-17-agent-parallel-tools.md
git commit -m "feat: add agent tool concurrency policy"
```

## Task 2: Record Tool Policy in Backend Events

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Write failing backend test**

Add test:

```rust
#[test]
fn agent_service_model_loop_records_tool_concurrency_policy() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("\"concurrencyPolicy\""));
    assert!(source.contains("serde_json::to_value(&routed_call.tool.concurrency"));
}
```

Run:

```bash
cargo test -p backend-rust agent_service_model_loop_records_tool_concurrency_policy --offline
```

Expected: FAIL because backend does not record policy.

**Step 2: Add policy to ActionSelected**

After routing a tool call, enrich `action_payload` with:

```rust
"concurrencyPolicy": routed_call.tool.concurrency
```

Do this before appending `RunEventKind::ActionSelected`.

**Step 3: Update matrix**

Change Parallel tools row to `slice-1 implemented` with wording that policy/lock/cancellation contract and event visibility exist, while true async execution remains next.

**Step 4: Verify and commit**

Run:

```bash
cargo test -p backend-rust model_loop --offline
cargo test -p backend-rust agent_service --offline
cargo fmt -- --check
```

Commit:

```bash
git add backend/src/application/ai/agent_service.rs docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "feat: record agent tool concurrency policy"
```

## Task 3: Final Verification

Run:

```bash
cargo fmt -- --check
cargo test -p novex-tools --offline
cargo test -p backend-rust model_loop --offline
cargo test -p backend-rust agent_service --offline
cargo test --workspace --offline
git status --short
```

Expected: formatting clean, selected tests pass, workspace tests pass, worktree clean.
