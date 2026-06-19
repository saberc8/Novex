# Agent Multi-Turn Loop Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade the current `runtimeMode=model_loop` POC from a one-tool follow-up into a Codex-shaped, budget-bounded multi-turn loop that can continue model sampling after each tool observation.

**Architecture:** Keep `AgentService` as the tenant/RBAC/database adapter and use `novex-agent-runtime` for turn-state and budget semantics. The first slice loops over configured `runtime.llm.code_agent`, model-visible tools, policy checks, tool execution, observation injection, and final output while preserving the existing `ai_run` status machine and trace/rollout events.

**Tech Stack:** Rust, Axum service layer, SQLx repositories, `novex-agent-runtime`, `novex-agent-protocol`, `novex-model`, `novex-tools`.

---

## Scope

In scope:

- Convert `TaskBudget` into runtime turn/tool limits.
- Add pure runtime tests for budget boundary semantics.
- Refactor `AgentService::create_model_loop_run` into an explicit loop over turns.
- Keep high-risk approval pause behavior.
- Stop with a clear budget-exceeded final event when the model asks for more tools than allowed.
- Keep configured-model calls routed through `ModelRuntimeService::chat_completion_for_purpose(ModelRoutePurpose::CodeAgent, ...)`.

Out of scope:

- Sandbox command execution.
- Parallel tool calls.
- Context compaction.
- Live MCP streaming transport.
- New database tables.

## Task 1: Runtime Budget Contract

**Files:**
- Modify: `crates/novex-agent-runtime/src/lib.rs`

**Step 1: Write the failing test**

Add tests:

- `runtime_budget_allows_tool_calls_up_to_limit`
- `runtime_budget_exceeds_when_tool_calls_reach_limit_before_next_call`

The tests should prove that a loop with `max_tool_calls = 1` may execute the first tool, then must stop before a second tool call is executed.

**Step 2: Run failing test**

Run:

```bash
cargo test -p novex-agent-runtime runtime_budget_exceeds_when_tool_calls_reach_limit_before_next_call --offline
```

Expected: FAIL because the helper does not exist yet.

**Step 3: Implement minimal runtime helper**

Add:

- `AgentRuntimeState::can_execute_tool_call() -> bool`
- `AgentRuntimeState::is_tool_call_budget_exhausted() -> bool`

The helper should use `>= max_tool_calls` for the next-call gate, while existing `next_outcome` may continue to indicate the current item needs follow-up.

**Step 4: Verify**

Run:

```bash
cargo test -p novex-agent-runtime --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-agent-runtime/src/lib.rs docs/plans/2026-06-17-agent-multi-turn-loop.md
git commit -m "feat: define agent multi-turn budget contract"
```

## Task 2: Backend Multi-Turn Loop Adapter

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Write the failing tests**

Add tests:

- `model_loop_prompt_allows_budget_bounded_multiple_tool_calls`
- `agent_service_model_loop_uses_runtime_state_budget_gate`
- `agent_service_model_loop_records_budget_stop_when_tool_call_budget_exhausted`

Use source-level tests for the service orchestration and pure prompt assertions for loop contract. Do not require a live model or database.

**Step 2: Run failing test**

Run:

```bash
cargo test -p backend model_loop_prompt_allows_budget_bounded_multiple_tool_calls --offline
```

Expected: FAIL because the prompt still says one tool call.

**Step 3: Implement loop skeleton**

Refactor `create_model_loop_run` so it:

1. Creates `AgentRuntimeState` from `run_id` and command budget.
2. Pushes the user item.
3. Calls the configured CodeAgent model.
4. On final answer, records `FinalOutput` and succeeds.
5. On tool call, checks `state.can_execute_tool_call()` before execution.
6. Executes allowed tool calls and records `ToolCalled` + `Observation`.
7. Pushes observation and continues the model loop until final answer, approval pause, tool budget stop, or turn budget stop.

**Step 4: Keep policy behavior**

If tool policy requires approval, preserve existing `waiting_approval` behavior and do not continue the model loop.

**Step 5: Update migration tracker**

Update the runtime loop row in `docs/plans/2026-06-16-codex-migration-matrix.md` from one-tool slice wording to multi-turn bounded loop wording, while leaving compaction as follow-up.

**Step 6: Verify**

Run:

```bash
cargo test -p backend model_loop agent_service_model_loop --offline
cargo test -p backend agent_runtime_records_poc_trace_contract_events --offline
```

Expected: PASS.

**Step 7: Commit**

```bash
git add backend/src/application/ai/agent_service.rs docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "feat: run bounded multi-turn agent model loop"
```

## Task 3: Final Verification

Run:

```bash
cargo fmt -- --check
cargo test -p novex-agent-runtime --offline
cargo test -p backend model_loop agent_service_model_loop --offline
cargo test --workspace --offline
```

Expected: PASS. `live_rag_e2e` may remain ignored unless external infra is configured.
