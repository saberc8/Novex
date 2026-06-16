# Agent Runtime Registry Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an AppState-owned active agent run registry and cancellation token so in-flight model-loop model/tool awaits can be interrupted by `POST /cancel`.

**Architecture:** Introduce a cloneable `AgentRuntimeRegistry` in `backend/src/application/ai/agent_service.rs`, store it on `AppState`, and pass it into `AgentService` from agent HTTP handlers. Keep DB status as source of truth; use the registry as a fast in-process cancellation signal.

**Tech Stack:** Rust, Tokio `watch`, `tokio::select!`, SQLx-backed service methods, serde_json payloads, Cargo offline tests.

---

### Task 1: Runtime Registry Contract

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing test**

Add:

```rust
#[tokio::test]
async fn agent_runtime_registry_signals_registered_run_cancellation() {
    let registry = AgentRuntimeRegistry::default();
    let (_guard, token) = registry.register_run(42, 1001);

    assert!(!token.is_cancelled());
    assert!(registry.cancel_run(42, 1001));
    token.cancelled().await;
    assert!(token.is_cancelled());
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend-rust runtime_registry --offline
```

Expected: FAIL because `AgentRuntimeRegistry` does not exist.

**Step 3: Implement minimal registry**

Add:

```rust
#[derive(Debug, Clone, Default)]
pub struct AgentRuntimeRegistry {
    inner: Arc<Mutex<HashMap<AgentRunKey, watch::Sender<bool>>>>,
}

#[derive(Debug, Clone)]
pub struct AgentRunCancellationToken {
    receiver: watch::Receiver<bool>,
}
```

Methods:

- `register_run(tenant_id, run_id) -> (ActiveAgentRunGuard, AgentRunCancellationToken)`
- `cancel_run(tenant_id, run_id) -> bool`
- `AgentRunCancellationToken::is_cancelled() -> bool`
- `AgentRunCancellationToken::cancelled(self) -> impl Future`

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p backend-rust runtime_registry --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: add agent runtime registry"
```

### Task 2: Wire Registry Through AppState And AgentService

**Files:**
- Modify: `backend/src/interfaces/http/mod.rs`
- Modify: `backend/src/interfaces/http/ai/agent.rs`
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify test helpers that construct `AppState`.

**Step 1: Write failing source tests**

Add tests:

```rust
#[test]
fn agent_handlers_share_runtime_registry_from_app_state() {
    let source = include_str!("agent.rs");
    assert!(source.contains("state.agent_runtime"));
    assert!(source.contains("AgentService::for_tenant_with_runtime"));
}

#[test]
fn app_state_owns_agent_runtime_registry() {
    let source = include_str!("../mod.rs");
    assert!(source.contains("agent_runtime: AgentRuntimeRegistry"));
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend-rust runtime_registry --offline
```

Expected: FAIL until `AppState` and handlers are wired.

**Step 3: Implement wiring**

- Import `AgentRuntimeRegistry` in `interfaces/http/mod.rs`.
- Add `pub agent_runtime: AgentRuntimeRegistry` to `AppState`.
- Initialize it in `build_router_inner`.
- Add `AgentService::for_tenant_with_runtime`.
- Update agent create/resume/cancel/get/list/trace handlers to pass `state.agent_runtime`.
- Update `test_state()` helpers with `agent_runtime: AgentRuntimeRegistry::default()`.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p backend-rust runtime_registry --offline
cargo test -p backend-rust agent_handlers --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs backend/src/interfaces/http/mod.rs backend/src/interfaces/http/ai/agent.rs
git commit -m "feat: share agent runtime registry"
```

### Task 3: Interrupt Model And Tool Futures

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add source guards:

```rust
#[test]
fn agent_service_model_loop_awaits_model_with_runtime_token() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("await_model_loop_future_or_cancelled"));
    assert!(source.contains("model_call"));
}

#[test]
fn agent_service_tool_io_awaits_runtime_cancel_token() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("execute_agent_tool_io_with_timeout_and_cancel"));
    assert!(source.contains("cancelReason\": \"external_cancel"));
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend-rust runtime_registry --offline
```

Expected: FAIL until model/tool futures are wrapped.

**Step 3: Implement model-loop token usage**

- Register active run after run records are created:
  `let (_active_run_guard, cancel_token) = self.agent_runtime.register_run(self.tenant_id, run_id);`
- Wrap model runtime call in `await_model_loop_future_or_cancelled(cancel_token.clone(), "model_call", future)`.
- If cancelled, call `finish_model_loop_cancelled(..., "model_call")` and return `get_run`.

**Step 4: Implement tool I/O token usage**

- Add `cancel_token: AgentRunCancellationToken` to `PreparedAgentToolCall` or pass token into `execute_agent_tool_io_batch`.
- Race tool execution against both timeout and `cancel_token.cancelled()`.
- On token cancellation, return `AgentToolExecution::cancelled(... external_cancel ...)`.

**Step 5: Verify GREEN**

Run:

```bash
cargo test -p backend-rust runtime_registry --offline
cargo test -p backend-rust external_cancel --offline
cargo test -p backend-rust tool_io_timeout --offline
cargo test -p backend-rust parallel_tool --offline
cargo test -p backend-rust model_loop --offline
```

Expected: PASS.

**Step 6: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: interrupt agent model and tool futures"
```

### Task 4: Update Matrix And Final Verification

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update docs**

Update Runtime loop and Parallel tools notes:

- active runtime registry is implemented,
- in-process token cancellation interrupts model/tool awaits,
- background worker/join handles and cross-process cancellation remain next.

**Step 2: Verify**

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust runtime_registry --offline
cargo test -p backend-rust external_cancel --offline
cargo test -p backend-rust tool_io_timeout --offline
cargo test -p backend-rust parallel_tool --offline
cargo test -p backend-rust model_loop --offline
cargo test -p backend-rust agent_service --offline
cargo test --workspace --offline
```

Expected: all pass; `live_rag_e2e` may remain ignored unless POC infra is configured.

**Step 3: Commit**

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: record agent runtime registry progress"
```

