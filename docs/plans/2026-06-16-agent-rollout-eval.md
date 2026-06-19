# Agent Rollout Eval Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Codex-inspired rollout trace, replay, and eval gates so every agent run can be inspected, replayed into a timeline, converted into eval cases, and used as a release-quality signal.

**Architecture:** Introduce `novex-trace` as the canonical trace/rollout crate and keep `novex-eval` focused on scoring and reports. Backend maps existing `ai_run_event` rows into trace bundles first, then persists optional `ai_rollout` rows for replay and eval case capture. This is an adapter port from Codex `rollout` and `rollout-trace`, backed by Novex Run Graph and tenant control plane.

**Tech Stack:** Rust, SQLx/PostgreSQL, `novex-agent-protocol`, `novex-eval`, new `novex-trace`, backend `AgentService` and `EvalService`.

---

## Scope

In scope:

- Trace bundle and rollout event types.
- Run-event to trace mapping.
- Replay API for agent runs.
- Eval case candidate capture from real agent traces.
- Eval run mode that scores saved traces for tool choice, grounded answer, latency, cost, and approval policy.

Out of scope:

- Distributed tracing backend integration.
- OpenTelemetry export.
- Full deterministic replay of external tool side effects.
- UI dashboards beyond API contracts.

## Task 1: Add `novex-trace` Crate

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/novex-trace/Cargo.toml`
- Create: `crates/novex-trace/src/lib.rs`

**Step 1: Write failing tests**

Create `crates/novex-trace/src/lib.rs` with tests first:

```rust
#[test]
fn trace_bundle_orders_events_and_counts_tool_calls() {
    let bundle = TraceBundle::new("agent-1")
        .with_event(TraceEvent::user_message(2, "hi"))
        .with_event(TraceEvent::tool_call(3, "call-1", "rag.search"))
        .with_event(TraceEvent::final_answer(4, "done"));

    assert_eq!(bundle.trace_id, "agent-1");
    assert_eq!(bundle.tool_call_count(), 1);
    assert_eq!(bundle.events[0].sequence_no, 2);
}
```

**Step 2: Run failing test**

Run:

```bash
cargo test -p novex-trace --offline
```

Expected: FAIL because crate is not in the workspace.

**Step 3: Add workspace member and manifest**

Add `crates/novex-trace` to workspace members and dependencies.

Manifest dependencies:

```toml
serde.workspace = true
serde_json = "1"
novex-agent-protocol.workspace = true
novex-ai-core.workspace = true
```

**Step 4: Implement pure trace model**

Add:

- `TraceEventKind`
- `TraceEvent`
- `TraceBundle`
- `TraceReplaySummary`
- helpers for user message, assistant message, tool call, observation, final answer, error.

**Step 5: Verify**

Run:

```bash
cargo test -p novex-trace --offline
cargo test --workspace --offline
```

Expected: PASS.

**Step 6: Commit**

```bash
git add Cargo.toml crates/novex-trace
git commit -m "feat: add trace rollout crate"
```

## Task 2: Map Agent Run Events to Trace Bundles

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`
- Modify: `backend/src/interfaces/http/ai/agent.rs`

**Step 1: Write failing tests**

Add tests:

- `agent_run_events_convert_to_trace_bundle`
- `agent_trace_snapshot_contains_replay_summary`
- `agent_trace_route_is_registered_and_requires_auth`

Example pure helper test:

```rust
#[test]
fn agent_run_events_convert_to_trace_bundle() {
    let events = vec![
        fake_event("input_received", 1, serde_json::json!({"item":{"type":"user_message","content":"hi"}})),
        fake_event("tool_called", 2, serde_json::json!({"toolCode":"rag.search"})),
        fake_event("final_output", 3, serde_json::json!({"answer":"done"})),
    ];

    let bundle = agent_events_to_trace_bundle("agent-1", events);

    assert_eq!(bundle.trace_id, "agent-1");
    assert_eq!(bundle.tool_call_count(), 1);
}
```

**Step 2: Run failing test**

Run:

```bash
cargo test -p backend agent_run_events_convert_to_trace_bundle --offline
```

Expected: FAIL.

**Step 3: Implement mapping and API**

Add:

- `agent_events_to_trace_bundle(trace_id, events)`
- `AgentTraceReplayResp`
- `GET /ai/agents/runs/:runId/trace`

The route should read current tenant-scoped run events and return:

```json
{
  "traceId": "agent-...",
  "events": [],
  "summary": {
    "toolCallCount": 1,
    "finalStatus": "succeeded",
    "hasApprovalPause": false
  }
}
```

**Step 4: Verify**

Run:

```bash
cargo test -p backend agent_run_events_convert_to_trace_bundle --offline
cargo test -p backend agent_trace_route_is_registered_and_requires_auth --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs backend/src/interfaces/http/ai/agent.rs backend/src/infrastructure/persistence/ai_agent_repository.rs
git commit -m "feat: expose agent trace replay bundle"
```

## Task 3: Persist Optional Rollout Bundles

**Files:**
- Create: `backend/migrations/202606160002_create_ai_rollout.sql`
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing migration test**

Add:

```rust
#[test]
fn agent_rollout_migration_defines_replay_bundle_table() {
    let migration = include_str!("../../../migrations/202606160002_create_ai_rollout.sql");

    assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_rollout"));
    assert!(migration.contains("trace_id"));
    assert!(migration.contains("event_bundle"));
    assert!(migration.contains("summary_payload"));
}
```

**Step 2: Run failing test**

Run:

```bash
cargo test -p backend agent_rollout_migration_defines_replay_bundle_table --offline
```

Expected: FAIL.

**Step 3: Add table and repository**

`ai_rollout` fields:

- `id`
- `tenant_id`
- `run_id`
- `trace_id`
- `event_bundle`
- `summary_payload`
- `source`
- audit columns

Repository:

- `upsert_rollout_bundle`
- `find_rollout_by_run_id`

**Step 4: Save bundle at terminal run update**

When an agent run succeeds, fails, cancels, or pauses for approval, build a trace bundle and upsert `ai_rollout`. Keep existing `ai_agent_trace.event_snapshot` as a lightweight UI snapshot.

**Step 5: Verify**

Run:

```bash
cargo test -p backend agent_rollout_migration_defines_replay_bundle_table --offline
cargo test -p backend agent_runtime_records_poc_trace_contract_events --offline
```

Expected: PASS.

**Step 6: Commit**

```bash
git add backend/migrations/202606160002_create_ai_rollout.sql backend/src/infrastructure/persistence/ai_agent_repository.rs backend/src/application/ai/agent_service.rs
git commit -m "feat: persist agent rollout bundles"
```

## Task 4: Capture Eval Case Candidates From Traces

**Files:**
- Modify: `crates/novex-eval/src/lib.rs`
- Modify: `backend/src/application/ai/eval_service.rs`
- Modify: `backend/src/infrastructure/persistence/ai_eval_repository.rs`

**Step 1: Write failing tests**

Add to `novex-eval`:

```rust
#[test]
fn trace_eval_candidate_extracts_tool_and_final_answer() {
    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle_with_tool_and_final());

    assert_eq!(candidate.target_kind, EvalTargetKind::ReAct);
    assert_eq!(candidate.expected.tool_code.as_deref(), Some("rag.search"));
    assert!(candidate.prompt.contains("customer data"));
}
```

**Step 2: Run failing test**

Run:

```bash
cargo test -p novex-eval trace_eval_candidate_extracts_tool_and_final_answer --offline
```

Expected: FAIL.

**Step 3: Implement candidate model**

Add:

- `EvalCaseCandidate`
- `TraceEvalPolicy`
- `from_trace_bundle`

Candidate fields should include prompt, expected tool, final answer snippets, citations if present, approval pause, latency and cost tags.

**Step 4: Add backend capture endpoint**

Add:

- `POST /ai/eval/cases/from-agent-run/:runId`

It should:

1. Load rollout bundle.
2. Build candidate.
3. Save disabled eval case or return preview when `dryRun=true`.

**Step 5: Verify**

Run:

```bash
cargo test -p novex-eval --offline
cargo test -p backend eval_case_capture --offline
```

Expected: PASS.

**Step 6: Commit**

```bash
git add crates/novex-eval backend/src/application/ai/eval_service.rs backend/src/infrastructure/persistence/ai_eval_repository.rs
git commit -m "feat: capture eval cases from agent traces"
```

## Task 5: Add Trace-backed Eval Run Mode

**Files:**
- Modify: `backend/src/application/ai/eval_service.rs`
- Modify: `backend/src/interfaces/http/ai/eval.rs`
- Modify: `crates/novex-eval/src/lib.rs`

**Step 1: Write failing tests**

Add:

- `eval_runtime_normalizes_trace_replay_run_mode`
- `eval_runtime_scores_agent_trace_tool_and_answer`
- `eval_runtime_report_response_exposes_trace_gate_summary`

**Step 2: Run failing test**

Run:

```bash
cargo test -p backend eval_runtime_normalizes_trace_replay_run_mode --offline
```

Expected: FAIL.

**Step 3: Implement run mode**

Add run mode:

- `trace_replay`

For each eval case:

1. Locate trace by `tags.agentRunId` or `expectedPayload.traceId`.
2. Convert trace to actual payload.
3. Score tool, answer, citation, latency, and cost.
4. Persist result and aggregate report.

**Step 4: Verify**

Run:

```bash
cargo test -p backend eval_runtime_scores_agent_trace_tool_and_answer --offline
cargo test -p backend --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/eval_service.rs backend/src/interfaces/http/ai/eval.rs crates/novex-eval/src/lib.rs
git commit -m "feat: score eval runs from agent traces"
```

## Task 6: Full Verification

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
```

Expected: PASS.

Acceptance is met only when an agent run can be converted to a trace bundle, replayed via API, captured as an eval case, and scored by a trace-backed eval run.
