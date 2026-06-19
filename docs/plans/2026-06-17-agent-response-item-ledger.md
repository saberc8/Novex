# Agent Response Item Ledger Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist ordered Agent turn items in a durable ledger and expose them in run replay responses.

**Architecture:** Add `ai_agent_turn_item` as a Novex control-plane ledger linked to `ai_run_event`. Update `append_event` so turn-item event payloads are inserted with their source run event transactionally. Expose ordered `turnItems` from `get_run_trace` without changing SSE event cursors or trace event generation.

**Tech Stack:** Rust, SQLx, PostgreSQL, serde, existing Novex Agent runtime and Run Graph repository.

## Global Constraints

- Keep `ai_run_event` as the event stream and trace source; do not replace it with the new ledger.
- Store normalized `AgentTurnItem` JSON exactly as serialized by `novex-agent-protocol`.
- Insert the source event and ledger row atomically.
- Preserve event `sequence_no` ordering as the replay order for ledger rows.
- Do not backfill old runs in this slice.
- Follow TDD: write and run failing tests before production code.

---

### Task 1: Ledger Schema And Contract Tests

Status: Completed.

**Files:**
- Create: `backend/migrations/202606170009_create_ai_agent_turn_item.sql`
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`
- Modify: `backend/src/application/ai/agent_service.rs`

**Interfaces:**
- Produces future structs:
  - `AgentTurnItemSaveRecord`
  - `AgentTurnItemRecord`
  - `AgentTurnItemFilter`
- Produces future repository methods:
  - `create_event_with_turn_item`
  - `list_turn_items`

- [x] **Step 1: Write failing tests**

Add tests for:
- migration contains `CREATE TABLE IF NOT EXISTS ai_agent_turn_item`, `source_event_id`, `sequence_no`, `item_type`, `call_id`, `tool_code`, `item_payload`, and replay indexes;
- repository source exposes `AgentTurnItemSaveRecord`, `AgentTurnItemRecord`, `create_event_with_turn_item`, `list_turn_items`, transaction usage, and ordered `FROM ai_agent_turn_item`;
- service helper source exposes `agent_turn_item_save_record_from_event_payload`, `agent_turn_item_from_record`, and `load_model_loop_turn_item_history`.

- [x] **Step 2: Run red tests**

Run: `cargo test -p backend agent_turn_item --offline`

Result: FAIL as expected because `AgentTurnItemRecord`,
`agent_turn_item_save_record_from_event_payload`, and `agent_turn_item_from_record` did not exist yet.

### Task 2: Repository And Serialization Helpers

Status: Completed.

**Files:**
- Create: `backend/migrations/202606170009_create_ai_agent_turn_item.sql`
- Modify: `backend/src/infrastructure/persistence/ai_agent_repository.rs`
- Modify: `backend/src/application/ai/agent_service.rs`

**Interfaces:**
- Produces:
  - `fn agent_turn_item_type_code(item: &AgentTurnItem) -> &'static str`
  - `fn agent_turn_item_call_id(item: &AgentTurnItem) -> Option<String>`
  - `fn agent_turn_item_tool_code(item: &AgentTurnItem) -> Option<String>`
  - `fn agent_turn_item_save_record_from_event_payload(...) -> Option<AgentTurnItemSaveRecord>`
  - `fn agent_turn_item_from_record(record: AgentTurnItemRecord) -> Result<AgentTurnItem, AppError>`

- [x] **Step 1: Implement migration and repository**

Create the ledger table and repository methods. `create_event_with_turn_item` must insert into `ai_run_event` and optional `ai_agent_turn_item` inside one SQLx transaction.

- [x] **Step 2: Implement serialization helpers**

Parse only payloads shaped as:

```json
{
  "eventSource": "novex-agent-runtime",
  "item": { "type": "tool_call", "...": "..." }
}
```

Ignore non-turn-item events.

- [x] **Step 3: Run green focused tests**

Run: `cargo test -p backend agent_turn_item --offline`

Result: PASS.

### Task 3: Runtime Integration And Replay Response

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Interfaces:**
- Consumes: repository `create_event_with_turn_item`, `list_turn_items`.
- Produces: `AgentTraceReplayResp.turn_items: Vec<AgentTurnItem>`.

- [x] **Step 1: Wire append_event**

Change `append_event` to:
- allocate the source event id and sequence;
- derive an optional `AgentTurnItemSaveRecord` from the payload;
- call `create_event_with_turn_item`.

- [x] **Step 2: Wire get_run_trace**

Load ordered turn items through `load_model_loop_turn_item_history(run_id)` and return them in `AgentTraceReplayResp`.

- [x] **Step 3: Run focused runtime tests**

Run:
- `cargo test -p backend agent_runtime_event_payload_preserves_turn_item_shape --offline`
- `cargo test -p backend agent_trace_snapshot_contains_replay_summary --offline`
- `cargo test -p backend agent_run_events_convert_to_trace_bundle --offline`

Result: PASS.

### Task 4: Docs, Matrix, Verification, Merge

Status: In Progress.

**Files:**
- Create: `docs/plans/2026-06-17-agent-response-item-ledger-design.md`
- Create: `docs/plans/2026-06-17-agent-response-item-ledger.md`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

- [x] **Step 1: Update migration matrix**

Move adapter-port durable `AgentTurnItem` persistence/replay from remaining runtime-loop gap into implemented evidence as a Novex item ledger. Provider-native wire-shape parity remains a later transport concern.

- [x] **Step 2: Run full verification**

Run:
- `cargo fmt -- --check`
- `cargo test --workspace --offline`
- `git diff --check`

Expected: PASS.

- [ ] **Step 3: Commit, merge, clean**

Commit feature work, merge `feat/enterprise-agent-foundation` into `main`, rerun full verification on `main`, run `cargo clean` in both worktrees, and sync feature to `main`.

**Verification evidence so far:**
- Red: `cargo test -p backend agent_turn_item --offline` failed before implementation on missing `AgentTurnItemRecord` and replay helper functions.
- Green: `cargo test -p backend agent_turn_item --offline`
- Green: `cargo test -p backend agent_service_response_item_ledger --offline`
- Green: `cargo test -p backend agent_runtime_event_payload_preserves_turn_item_shape --offline`
- Green: `cargo test -p backend agent_trace_snapshot_contains_replay_summary --offline`
- Green: `cargo test -p backend agent_run_events_convert_to_trace_bundle --offline`
- Green: `cargo fmt -- --check`
- Green: `cargo test --workspace --offline`
- Green: `git diff --check`
