# Agent Tool Execution Envelope Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the shared Agent tool execution result envelope from `backend/src/application/ai/agent_service.rs` into `crates/novex-tools`, preserving existing status strings, payload shapes, and backend execution behavior.

**Architecture:** `novex-tools` owns tool schema, routing, policy, input adapters, and now the canonical `AgentToolExecution` envelope returned by concrete executor adapters. Backend remains responsible for DB persistence, audit writes, model/media route resolution, credentials, and live external I/O in this slice.

**Tech Stack:** Rust workspace crates, `serde_json::Value`, existing backend Agent tool execution path, `cargo test --offline`.

## Global Constraints

- Do not change `response_payload`, `status`, `dry_run`, `error_message`, or `final_output` semantics.
- Keep existing status strings exactly: `succeeded`, `failed`, `cancelled`.
- Do not move live HTTP, model route resolution, DB writes, credential lookup, or media persistence in this slice.
- Backend should import `AgentToolExecution` from `novex_tools` and should not define a local duplicate.
- Use TDD: add failing tests before production code, verify RED, implement minimally, verify GREEN.

---

### Task 1: Shared Agent Tool Execution Envelope

**Files:**
- Modify: `crates/novex-tools/src/lib.rs`
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Produces: `pub struct AgentToolExecution`
- Produces: `AgentToolExecution::succeeded(response_payload: Value, dry_run: bool, final_output: String) -> Self`
- Produces: `AgentToolExecution::failed(response_payload: Value, error_message: String, final_output: String) -> Self`
- Produces: `AgentToolExecution::cancelled(response_payload: Value, final_output: String) -> Self`
- Produces: `AgentToolExecution::succeeded_status(&self) -> bool`
- Produces: `AgentToolExecution::cancelled_status(&self) -> bool`

- [ ] **Step 1: Write failing `novex-tools` envelope tests**

Add tests in `crates/novex-tools/src/lib.rs` that call `AgentToolExecution::succeeded`, `failed`, and `cancelled`, asserting exact status strings, `dry_run`, `error_message`, `final_output`, and `response_payload`.

- [ ] **Step 2: Run RED**

Run: `cargo test -p novex-tools agent_tool_execution --offline`

Expected: fail because `AgentToolExecution` is not defined in `novex-tools`.

- [ ] **Step 3: Write failing backend source-contract test**

Add or update a backend source-contract test that expects `agent_service.rs` to import `AgentToolExecution` from `novex_tools` and not contain `struct AgentToolExecution`.

- [ ] **Step 4: Run RED**

Run: `cargo test -p backend agent_tool_execution_envelope_lives_in_novex_tools --offline`

Expected: fail because `agent_service.rs` still defines a local `AgentToolExecution`.

- [ ] **Step 5: Implement shared envelope**

Move the struct and methods into `crates/novex-tools/src/lib.rs` with public fields and public constructors/status helpers so backend can keep using the existing field access and constructors without payload changes.

- [ ] **Step 6: Consume shared envelope in backend**

Import `AgentToolExecution` from `novex_tools`, delete the local struct and impl block from `backend/src/application/ai/agent_service.rs`, and leave all concrete executor functions returning the same type.

- [ ] **Step 7: Update matrix**

Update the Tool router row and acceptance evidence in `docs/plans/2026-06-16-codex-migration-matrix.md` to record the shared execution envelope as the next extraction slice.

- [ ] **Step 8: Verify GREEN**

Run:

```bash
cargo fmt --all -- --check
git diff --check
cargo test -p novex-tools agent_tool_execution --offline
cargo test -p novex-tools --offline
cargo test -p backend agent_tool_execution_envelope_lives_in_novex_tools --offline
cargo test -p backend agent_tool_executor_selection --offline
cargo test -p backend agent_tool_input_adapters_live_in_novex_tools --offline
cargo test -p backend model_loop --offline
cargo test --workspace --offline
```

- [ ] **Step 9: Commit, merge, clean**

Commit plan and implementation separately, merge `feat/enterprise-agent-foundation` into `main` without losing any main-only commits, rerun verification on main, then run `cargo clean` in both the main worktree and `.worktrees/enterprise-agent-foundation`.
