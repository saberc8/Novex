# Agent Cross-Process Provider Abort Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Interrupt active model provider futures when another process marks the Agent run as cancelling or cancelled.

**Architecture:** Add a persistent cancellation watcher to `AgentService`, route provider awaits through a new helper that races the model future against both local runtime token and persistent run-status cancellation, and preserve existing cancellation finalization/events.

**Tech Stack:** Rust, tokio `select!`, existing `AiAgentRepository`, Cargo offline tests.

---

### Task 1: Provider Await Helper

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add tests that:

- a persistent cancellation future beats a pending model provider future;
- the existing local runtime token path still returns cancelled;
- source contracts require `model_call` and `context_compaction` to use the new provider abort helper.

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend-rust provider_abort --offline
```

Expected: FAIL because the provider abort helper does not exist.

**Step 3: Implement helper**

Add:

```rust
async fn await_model_loop_provider_future_or_cancelled<F, C, T>(
    cancel_token: AgentRunCancellationToken,
    persistent_cancel: C,
    stage: &str,
    future: F,
) -> Result<ModelLoopFutureAwait<T>, AppError>
where
    F: Future<Output = Result<T, AppError>>,
    C: Future<Output = Result<(), AppError>>,
```

It uses `tokio::select!` with local token, persistent cancel future, and provider future.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p backend-rust provider_abort --offline
```

Expected: PASS.

### Task 2: Persistent Cancellation Watcher

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add source-contract tests proving:

- `wait_for_model_loop_persistent_cancel` polls `self.repo.find_run`;
- the watcher checks `model_loop_cancel_requested`;
- the watcher uses a bounded poll interval constant.

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend-rust provider_abort --offline
```

Expected: FAIL until the watcher exists.

**Step 3: Implement watcher**

Add:

```rust
const MODEL_LOOP_PERSISTENT_CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(250);

async fn wait_for_model_loop_persistent_cancel(
    &self,
    run_id: i64,
) -> Result<(), AppError>
```

It loops, sleeps the interval, reloads the run row, and returns once status is `cancelling` or `cancelled`.

**Step 4: Wire model provider calls**

Use the new helper for:

- main `model_call`;
- `context_compaction`.

**Step 5: Verify GREEN**

Run:

```bash
cargo test -p backend-rust provider_abort --offline
cargo test -p backend-rust external_cancel --offline
cargo test -p backend-rust model_loop_compaction --offline
```

Expected: PASS.

### Task 3: Documentation And Merge

Status: Completed.

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-cross-process-provider-abort.md`

**Step 1: Update migration matrix**

Move active cross-process provider abort from remaining work into implemented runtime-loop evidence, while leaving provider-specific cancel endpoints and provider-call lease tables as future work.

**Step 2: Full verification**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
```

**Step 3: Commit, merge, clean**

Commit feature worktree, merge into `main`, rerun verification on `main`, then run `cargo clean` in both worktrees.
