# Agent Response Item History Projection Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make `AgentTurnItem` history drive model-loop sampling so Novex no longer keeps a parallel mutable chat transcript beside runtime state.

**Architecture:** Keep typed state in `AgentRuntimeState`. Add a backend projection helper that adapts typed runtime items into provider-facing `ModelChatMessage` values and respects the latest context-compaction window.

**Tech Stack:** Rust, Novex agent protocol/runtime crates, backend model loop tests.

---

### Task 1: Response Item Projection Tests

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add tests for:
- tool-call plus observation history projecting into provider messages;
- latest compaction window preserving original input and summary while dropping compacted observation payloads;
- source contract proving model-loop sampling uses `runtime_state.items` instead of a mutable `messages` transcript.

**Step 2: Run red tests**

Run: `cargo test -p backend-rust model_loop_history_messages --offline`

Expected: FAIL because `build_model_loop_messages_from_history` does not exist yet.

### Task 2: Projection Helper

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Implement helper**

Add:
- `build_model_loop_messages_from_history`
- `append_model_loop_history_messages`
- canonical tool-call serialization for single/batch calls
- observation prompt projection with call id, tool code, status, and payload
- compaction summary prompt projection

**Step 2: Run green tests**

Run:
- `cargo test -p backend-rust model_loop_history_messages --offline`
- `cargo test -p backend-rust agent_service_model_loop_installs_response_item_history_for_sampling --offline`
- `cargo test -p backend-rust observation_prompt_includes_tool_result_and_final_answer_instruction --offline`

Expected: PASS.

### Task 3: Model Loop Integration

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Replace mutable prompt transcript**

Remove:
- initial `let mut messages = vec![...]`
- tool-observation `batch_observations` transcript staging
- `messages = build_compacted_model_loop_messages(...)`
- post-tool `messages.push(...)`

Build provider messages inside each model-attempt from:

```rust
build_model_loop_messages_from_history(&command.input, &tool_codes, &runtime_state.items)
```

### Task 4: Docs, Verification, Merge

Status: Completed.

**Files:**
- Create: `docs/plans/2026-06-17-agent-response-item-history-design.md`
- Create: `docs/plans/2026-06-17-agent-response-item-history.md`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Update migration matrix**

Move ResponseItem-history projection into implemented runtime-loop evidence while keeping durable provider-native ResponseItem persistence/replay parity as next work.

**Step 2: Run verification**

Run:
- `cargo fmt -- --check`
- `cargo test --workspace --offline`

Expected: PASS.

**Step 3: Commit, merge, clean**

Commit feature work, merge `feat/enterprise-agent-foundation` into `main`, rerun full verification on `main`, run `cargo clean` in both worktrees, and sync feature to main.

**Verification evidence so far:**
- Red: `cargo test -p backend-rust model_loop_history_messages --offline` failed on missing `build_model_loop_messages_from_history`.
- Green: `cargo test -p backend-rust model_loop_history_messages --offline`
- Green: `cargo test -p backend-rust agent_service_model_loop_installs_response_item_history_for_sampling --offline`
- Green: `cargo test -p backend-rust observation_prompt_includes_tool_result_and_final_answer_instruction --offline`
- Green: `cargo fmt -- --check`
- Green: `cargo test --workspace --offline`
