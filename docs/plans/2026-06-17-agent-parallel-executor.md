# Agent Parallel Executor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Execute `ToolBatchPlan::Parallel` tool I/O concurrently while persisting audit, steps, and events in deterministic batch order.

**Architecture:** Split backend tool execution into an I/O phase and a record phase. `execute_agent_tool_io_batch` runs prepared calls with `join_all` only for `ToolBatchExecutionMode::Parallel`; persistence remains serial in model-loop order.

**Tech Stack:** Rust, tokio, futures-util, serde_json, Cargo offline tests.

---

## Task 1: Add Parallel I/O Batch Contract

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add tests:

```rust
#[tokio::test]
async fn parallel_tool_io_batch_polls_calls_concurrently_and_preserves_order() {
    use std::sync::Arc;
    use tokio::sync::Barrier;

    let barrier = Arc::new(Barrier::new(2));
    let calls = vec![
        test_prepared_tool_call(0, "call-1", "rag.search"),
        test_prepared_tool_call(1, "call-2", "github.repo.read"),
    ];

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(250),
        execute_agent_tool_io_batch(ToolBatchExecutionMode::Parallel, calls, {
            let barrier = barrier.clone();
            move |prepared| {
                let barrier = barrier.clone();
                async move {
                    barrier.wait().await;
                    Ok(test_executed_tool_call(prepared))
                }
            }
        }),
    )
    .await
    .expect("parallel execution should not deadlock")
    .unwrap();

    assert_eq!(result[0].prepared.call_id, "call-1");
    assert_eq!(result[1].prepared.call_id, "call-2");
}

#[tokio::test]
async fn serial_tool_io_batch_runs_calls_in_sequence() {
    use std::sync::{Arc, Mutex};

    let order = Arc::new(Mutex::new(Vec::new()));
    let calls = vec![
        test_prepared_tool_call(0, "call-1", "media.image.generate"),
        test_prepared_tool_call(1, "call-2", "feishu.message.send"),
    ];

    let result = execute_agent_tool_io_batch(ToolBatchExecutionMode::Serial, calls, {
        let order = order.clone();
        move |prepared| {
            let order = order.clone();
            async move {
                order.lock().unwrap().push(prepared.call_id.clone());
                Ok(test_executed_tool_call(prepared))
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(*order.lock().unwrap(), vec!["call-1".to_owned(), "call-2".to_owned()]);
}
```

Run:

```bash
cargo test -p backend-rust parallel_tool_io_batch --offline
```

Expected: FAIL because prepared/executed types and helper do not exist.

**Step 2: Implement helper types and function**

Add:

```rust
struct PreparedAgentToolCall { ... }
struct ExecutedAgentToolCall { ... }
async fn execute_agent_tool_io_batch<F, Fut>(...) -> Result<Vec<ExecutedAgentToolCall>, AppError>
```

Use `futures_util::future::join_all` for `ToolBatchExecutionMode::Parallel`. Preserve input order by collecting `join_all` results directly.

**Step 3: Verify and commit**

Run:

```bash
cargo test -p backend-rust parallel_tool_io_batch --offline
cargo fmt -- --check
```

Commit:

```bash
git add backend/src/application/ai/agent_service.rs docs/plans/2026-06-17-agent-parallel-executor-design.md docs/plans/2026-06-17-agent-parallel-executor.md
git commit -m "feat: add agent parallel tool io executor"
```

## Task 2: Split Tool I/O From Persistence

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing source-level tests**

Add tests:

```rust
#[test]
fn agent_service_parallel_tool_execution_separates_io_from_persistence() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("execute_agent_tool_io_batch"));
    assert!(source.contains("execute_agent_tool_io"));
    assert!(source.contains("record_agent_tool_execution"));
}

#[test]
fn agent_service_model_loop_evaluates_batch_approval_before_execution() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    let approval_index = source.find("batch_policy.requires_approval").unwrap();
    let execution_index = source.find("execute_agent_tool_io_batch").unwrap();
    assert!(approval_index < execution_index);
}
```

Run:

```bash
cargo test -p backend-rust agent_service_parallel_tool_execution_separates_io_from_persistence --offline
```

Expected: FAIL because split helpers do not exist yet.

**Step 2: Refactor**

Move the current execution part of `execute_and_record_tool_call` into:

```rust
async fn execute_agent_tool_io(&self, user_id: i64, prepared: PreparedAgentToolCall) -> Result<ExecutedAgentToolCall, AppError>
```

Move audit/step/media persistence into:

```rust
async fn record_agent_tool_execution(&self, user_id: i64, run_id: i64, prepared: &PreparedAgentToolCall, execution: AgentToolExecution) -> Result<RecordedToolExecution, AppError>
```

Keep `execute_and_record_tool_call` as a wrapper.

**Step 3: Verify and commit**

Run:

```bash
cargo test -p backend-rust agent_service_parallel_tool_execution_separates_io_from_persistence --offline
cargo test -p backend-rust agent_service --offline
cargo fmt -- --check
```

Commit:

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "refactor: split agent tool io from persistence"
```

## Task 3: Use Parallel I/O in Model Loop Batches

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Write failing source-level test**

Add test:

```rust
#[test]
fn agent_service_model_loop_executes_parallel_batches_via_io_executor() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("execute_agent_tool_io_batch(batch_execution_mode"));
    assert!(source.contains("for executed_call in executed_calls"));
}
```

Run:

```bash
cargo test -p backend-rust agent_service_model_loop_executes_parallel_batches_via_io_executor --offline
```

Expected: FAIL because model loop still executes inside the per-call loop.

**Step 2: Implement model loop batch execution**

In the model-loop tool-call branch:

- build `PreparedAgentToolCall` values after routing and policy lookup.
- evaluate every `batch_policy.requires_approval` before calling `execute_agent_tool_io_batch`.
- append `ActionSelected` events before execution.
- call `execute_agent_tool_io_batch(batch_execution_mode, prepared_calls, ...)`.
- record each returned execution serially with `record_agent_tool_execution`.
- append `ToolCalled` and `Observation` events in returned order.

**Step 3: Update matrix**

Change Parallel tools row to note true parallel tool I/O is in place while cancellation propagation remains next.

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
git commit -m "feat: execute parallel agent tool batches"
```

## Task 4: Final Verification

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust parallel_tool --offline
cargo test -p backend-rust model_loop --offline
cargo test -p backend-rust agent_service --offline
cargo test --workspace --offline
git status --short
```

Expected: all commands pass; live RAG E2E may remain ignored because it requires external infra.
