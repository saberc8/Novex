# Agent Runtime Supervisor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade the active agent runtime registry into a Codex-shaped supervised run handle registry with cancellation metadata, trace evidence, and eval tags.

**Architecture:** Keep Novex's current request-driven model loop. Replace the registry's raw cancellation-sender map with active run state snapshots, add a richer cancellation signal result for service trace payloads, and teach eval extraction to tag supervisor evidence from cancellation events.

**Tech Stack:** Rust, Tokio `watch`, SQLx service layer tests, `serde_json`, `novex-trace`, `novex-eval`, Cargo offline tests.

---

### Task 1: Runtime Supervisor Registry Contract

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add tests that assert:

```rust
#[test]
fn agent_runtime_registry_snapshots_active_model_loop_runs() {
    let registry = AgentRuntimeRegistry::default();
    let (_guard, _token) =
        registry.register_run_with_kind(42, 1001, AgentRuntimeTaskKind::ModelLoop);

    let snapshots = registry.active_run_snapshots();

    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].tenant_id, 42);
    assert_eq!(snapshots[0].run_id, 1001);
    assert_eq!(snapshots[0].task_kind, AgentRuntimeTaskKind::ModelLoop);
    assert_eq!(snapshots[0].status, AgentRuntimeRunStatus::Running);
    assert!(!snapshots[0].cancel_requested);
}

#[test]
fn active_run_guard_unregisters_runtime_snapshot_on_drop() {
    let registry = AgentRuntimeRegistry::default();
    let (guard, _token) = registry.register_run(42, 1001);
    assert_eq!(registry.active_run_snapshots().len(), 1);

    drop(guard);

    assert!(registry.active_run_snapshots().is_empty());
}

#[tokio::test]
async fn runtime_cancel_signal_marks_snapshot_cancelling() {
    let registry = AgentRuntimeRegistry::default();
    let (_guard, token) = registry.register_run(42, 1001);

    let signal = registry.cancel_run_signal(42, 1001);

    assert!(signal.sent);
    assert!(signal.active_before_cancel);
    assert_eq!(signal.snapshot.unwrap().status, AgentRuntimeRunStatus::Cancelling);
    token.clone().cancelled().await;
    assert!(token.is_cancelled());
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend runtime_supervisor --offline
```

Expected: FAIL because supervisor snapshot types and APIs do not exist.

**Step 3: Implement registry state**

Add:

- `AgentRuntimeTaskKind`
- `AgentRuntimeRunStatus`
- `AgentRuntimeRunSnapshot`
- `AgentRuntimeCancelSignal`
- private `AgentRuntimeRunState`

Store `HashMap<AgentRunKey, AgentRuntimeRunState>` in `AgentRuntimeRegistry`.

Keep `cancel_run(...) -> bool` as a compatibility wrapper over `cancel_run_signal`.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p backend runtime_supervisor --offline
cargo test -p backend runtime_registry --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: supervise active agent runtime runs"
```

### Task 2: Attach Supervisor Metadata To Cancel Events

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add tests that assert:

```rust
#[test]
fn agent_runtime_cancel_payload_includes_supervisor_snapshot() {
    let registry = AgentRuntimeRegistry::default();
    let (_guard, _token) = registry.register_run(42, 1001);
    let signal = registry.cancel_run_signal(42, 1001);

    let payload = runtime_cancelled_event_payload(signal);

    assert_eq!(payload["cancelled"], true);
    assert_eq!(payload["runtimeSignalSent"], true);
    assert_eq!(payload["runtimeSupervisor"]["activeBeforeCancel"], true);
    assert_eq!(payload["runtimeSupervisor"]["taskKind"], "model_loop");
    assert_eq!(payload["runtimeSupervisor"]["status"], "cancelling");
}

#[test]
fn agent_service_cancel_run_uses_supervisor_cancel_signal_payload() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("cancel_run_signal"));
    assert!(source.contains("runtime_cancelled_event_payload"));
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend runtime_supervisor --offline
```

Expected: FAIL until payload helper and service wiring exist.

**Step 3: Implement payload and service wiring**

- Add `runtime_cancelled_event_payload(signal: AgentRuntimeCancelSignal) -> Value`.
- Update `AgentService::cancel_run` to call `cancel_run_signal`.
- Persist the helper payload in the `Cancelled` event.
- Keep `refresh_trace_snapshot(... {"cancelled": true})` unchanged unless test evidence requires widening.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p backend runtime_supervisor --offline
cargo test -p backend external_cancel --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: trace agent runtime supervisor cancellation"
```

### Task 3: Eval Tags For Runtime Supervisor Evidence

**Files:**
- Modify: `crates/novex-eval/src/lib.rs`

**Step 1: Write failing test**

Add:

```rust
#[test]
fn trace_eval_candidate_tags_runtime_supervisor_cancellation() {
    let bundle = TraceBundle::new("agent-supervisor")
        .with_event(TraceEvent::user_message(1, "stop"))
        .with_event(TraceEvent::cancellation(
            2,
            json!({
                "cancelReason": "external_cancel",
                "runtimeSignalSent": true,
                "runtimeSupervisor": {
                    "activeBeforeCancel": true,
                    "taskKind": "model_loop",
                    "status": "cancelling"
                }
            }),
        ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["runtimeSupervisorTaskKind"], "model_loop");
    assert_eq!(candidate.tags["runtimeSupervisorCancelSignalSent"], true);
    assert_eq!(candidate.tags["runtimeSupervisorActiveBeforeCancel"], true);
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test -p novex-eval runtime_supervisor --offline
```

Expected: FAIL until eval extraction is implemented.

**Step 3: Implement extraction**

Add a small helper that reads the first cancellation payload and extracts:

- `runtimeSignalSent`
- `runtimeSupervisor.activeBeforeCancel`
- `runtimeSupervisor.taskKind`

Insert the tags when the fields exist.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p novex-eval runtime_supervisor --offline
cargo test -p novex-eval runtime_spans --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-eval/src/lib.rs
git commit -m "feat: tag runtime supervisor cancellations"
```

### Task 4: Matrix Update And Merge Verification

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update docs**

Update Runtime loop and Rollout trace rows to mention:

- active runtime supervisor snapshots,
- cancellation events with supervisor metadata,
- eval tags for active runtime cancellation evidence,
- durable background worker queue and provider-native abort remain next.

Add this plan to follow-up implementation plans.

**Step 2: Verify**

Run:

```bash
cargo fmt -- --check
cargo test -p backend runtime_supervisor --offline
cargo test -p backend runtime_registry --offline
cargo test -p backend external_cancel --offline
cargo test -p novex-eval runtime_supervisor --offline
cargo test --workspace --offline
```

Expected: all pass.

**Step 3: Commit**

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-runtime-supervisor-design.md docs/plans/2026-06-17-agent-runtime-supervisor.md
git commit -m "docs: record agent runtime supervisor progress"
```

**Step 4: Merge**

No-ff merge the feature worktree back into local `main`, rerun `cargo fmt -- --check` and `cargo test --workspace --offline` on `main`, then fast-forward the preserved feature worktree.
