# Agent Tool Timeout Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add runtime-level tool I/O timeouts that produce cancelled tool executions and cancelled observations.

**Architecture:** Extend `PreparedAgentToolCall` with a timeout and wrap every call inside `execute_agent_tool_io_batch`. Timeout returns an in-memory `ExecutedAgentToolCall` with `AgentToolExecution::cancelled`; existing serial persistence then writes audit, step, and observation records in deterministic order.

**Tech Stack:** Rust, tokio timeout, serde_json, Cargo offline tests.

---

## Task 1: Add Cancelled Tool Execution Contract

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add tests:

```rust
#[tokio::test]
async fn tool_io_timeout_returns_cancelled_execution() {
    let calls = vec![test_prepared_tool_call(0, "call-1", "rag.search")
        .with_timeout(std::time::Duration::from_millis(10))];

    let result = execute_agent_tool_io_batch(ToolBatchExecutionMode::Serial, calls, |prepared| async move {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        Ok(test_executed_tool_call(prepared))
    })
    .await
    .unwrap();

    assert_eq!(result[0].execution.status, "cancelled");
    assert_eq!(result[0].terminal_status, RunStatus::Cancelled);
    assert_eq!(result[0].execution.response_payload["cancelReason"], "tool_io_timeout");
}

#[test]
fn cancelled_tool_execution_maps_to_cancelled_observation_status() {
    let execution = AgentToolExecution::cancelled(
        serde_json::json!({"cancelReason":"tool_io_timeout"}),
        "timeout".to_owned(),
    );

    assert_eq!(
        tool_observation_status_for_execution(&execution),
        ToolObservationStatus::Cancelled
    );
}

#[test]
fn agent_service_model_loop_maps_cancelled_tool_observations() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("tool_observation_status_for_execution"));
}
```

Run:

```bash
cargo test -p backend tool_io_timeout_returns_cancelled_execution --offline
```

Expected: FAIL because timeout and cancelled execution do not exist.

**Step 2: Implement cancelled execution and timeout**

Add:

```rust
const AGENT_TOOL_IO_TIMEOUT: Duration = Duration::from_secs(45);
fn AgentToolExecution::cancelled(...)
fn AgentToolExecution::cancelled_status(&self) -> bool
fn tool_observation_status_for_execution(...)
```

Add `timeout: Duration` to `PreparedAgentToolCall`. Default prepared calls use `AGENT_TOOL_IO_TIMEOUT`.

Wrap each `execute(prepared)` inside `tokio::time::timeout(prepared.timeout, ...)` in `execute_agent_tool_io_batch`.

**Step 3: Verify and commit**

Run:

```bash
cargo test -p backend tool_io_timeout --offline
cargo test -p backend parallel_tool --offline
cargo fmt -- --check
```

Commit:

```bash
git add backend/src/application/ai/agent_service.rs docs/plans/2026-06-17-agent-tool-timeout-design.md docs/plans/2026-06-17-agent-tool-timeout.md
git commit -m "feat: cancel timed out agent tool io"
```

## Task 2: Wire Cancelled Observation in Model Loop and Matrix

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Write failing source-level test**

Add:

```rust
#[test]
fn agent_service_model_loop_records_tool_timeout_cancel_reason() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("\"cancelReason\""));
    assert!(source.contains("tool_io_timeout"));
}
```

Run:

```bash
cargo test -p backend agent_service_model_loop_records_tool_timeout_cancel_reason --offline
```

Expected: FAIL before cancel payload is implemented.

**Step 2: Update matrix**

Update Parallel tools row to say timeout-driven cancelled tool execution exists, while external cancel-token propagation remains next.

**Step 3: Verify and commit**

Run:

```bash
cargo test -p backend model_loop --offline
cargo test -p backend agent_service --offline
cargo fmt -- --check
```

Commit:

```bash
git add backend/src/application/ai/agent_service.rs docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "feat: record cancelled agent tool observations"
```

## Task 3: Final Verification

Run:

```bash
cargo fmt -- --check
cargo test -p backend tool_io_timeout --offline
cargo test -p backend parallel_tool --offline
cargo test -p backend model_loop --offline
cargo test -p backend agent_service --offline
cargo test --workspace --offline
git status --short
```

Expected: all commands pass; live RAG E2E may remain ignored because it requires external infra.
