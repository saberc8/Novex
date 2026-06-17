# Agent Event WebSocket Transport Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a WebSocket transport for durable agent run events by reusing the existing SSE cursor semantics, tenant-scoped `AgentService`, and event-list permission.

**Architecture:** Implement WebSocket as an HTTP adapter over the existing run-event stream. The application service remains the source of truth for cursor reads and terminal checks; the WebSocket handler only upgrades, polls, serializes text frames, and closes.

**Tech Stack:** Rust, axum WebSocket extractor, existing `AgentService`, existing event stream query settings.

## Global Constraints

- TDD: write failing tests first and verify red before production code.
- Reuse `AgentRunEventStreamQuery` and `AgentRunEventStreamSettings`.
- Reuse `ai:agent:event:list`.
- Do not add browser query-token authentication in this slice.
- Do not add provider token-delta streaming in this slice.
- Do not mark the persistent goal complete after this slice.
- Merge the feature worktree back to main after verification.
- Run `cargo clean` in both worktrees after the stage completes.

---

### Task 1: WebSocket Route Contract

**Files:**
- Modify: `backend/src/interfaces/http/ai/agent.rs`
- Modify: `backend/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `docs/plans/2026-06-17-agent-event-websocket.md`

**Interfaces:**
- Adds: `GET /ai/agents/runs/:run_id/events/ws`
- Adds: `stream_events_ws(...)`

- [ ] **Step 1: Write failing tests**

Add tests proving the route contract includes:

```rust
assert!(source.contains("/ai/agents/runs/:run_id/events/ws"));
assert!(source.contains("WebSocketUpgrade"));
assert!(source.contains("stream_events_ws"));
assert!(source.contains("AGENT_EVENT_LIST_PERMISSION"));
assert!(source.contains("agent_run_event_ws_loop"));
```

- [ ] **Step 2: Run red test**

Run:

```bash
cargo test -p backend-rust agent_event_websocket --offline
```

Expected: FAIL because the route, handler, and helper names do not exist.

- [ ] **Step 3: Implement minimal route**

Enable axum `ws`, import `WebSocketUpgrade`, add the route, check permission, construct tenant-scoped `AgentService`, and call `ws.on_upgrade(...)`.

- [ ] **Step 4: Run green route test**

Run:

```bash
cargo test -p backend-rust agent_event_websocket_route --offline
```

Expected: PASS.

### Task 2: WebSocket Frame Contract

**Files:**
- Modify: `backend/src/interfaces/http/ai/agent.rs`

**Interfaces:**
- Adds: `agent_run_event_ws_message(event) -> String`
- Adds: `agent_run_event_ws_error_message(err) -> String`

- [ ] **Step 1: Write failing tests**

Add tests proving:

```rust
let message = agent_run_event_ws_message(event);
let body: Value = serde_json::from_str(&message).unwrap();
assert_eq!(body["type"], "agent_run_event");
assert_eq!(body["sequenceNo"], 9);
assert_eq!(body["event"]["eventType"], "thought");
```

And:

```rust
let message = agent_run_event_ws_error_message(AppError::NotFound);
let body: Value = serde_json::from_str(&message).unwrap();
assert_eq!(body["type"], "error");
assert!(body["message"].is_string());
```

- [ ] **Step 2: Run red test**

Run:

```bash
cargo test -p backend-rust agent_event_websocket_message --offline
```

Expected: FAIL because message helpers do not exist.

- [ ] **Step 3: Implement message helpers**

Serialize typed text JSON frames with `type`, top-level `sequenceNo`, and full event payload.

- [ ] **Step 4: Run green message test**

Run:

```bash
cargo test -p backend-rust agent_event_websocket_message --offline
```

Expected: PASS.

### Task 3: WebSocket Poll Loop

**Files:**
- Modify: `backend/src/interfaces/http/ai/agent.rs`
- Modify: `docs/development/CODEX_AGENT_MIGRATION_MATRIX.md`

**Interfaces:**
- Adds: `agent_run_event_ws_loop(socket, service, run_id, settings)`

- [ ] **Step 1: Implement polling loop**

Use the same behavior as SSE:

- Drain pending events first.
- Update `after_sequence_no` after each sent event.
- Fetch `list_events_after_sequence` in batches.
- Close when the run is terminal.
- Close after `max_idle_ms`.
- Send a typed error frame and close on service errors.

- [ ] **Step 2: Run focused verification**

Run:

```bash
cargo test -p backend-rust agent_event_websocket --offline
cargo test -p backend-rust agent_event_stream --offline
```

Expected: PASS.

- [ ] **Step 3: Update migration matrix**

Mark the Runtime loop streaming transport slice as implemented for durable run events, while keeping provider token-delta streaming and browser-specific WebSocket auth as follow-up work.

### Task 4: Final Verification and Merge

**Files:**
- All changed files.

- [ ] **Step 1: Run full verification**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
git diff --check
```

Expected: PASS.

- [ ] **Step 2: Commit feature branch**

Commit with:

```bash
git commit -m "feat: add agent run event websocket transport"
```

- [ ] **Step 3: Merge to main**

Merge with:

```bash
git merge --no-ff feat/enterprise-agent-foundation -m "merge: enterprise agent foundation event websocket"
```

- [ ] **Step 4: Verify main and clean**

Run the full verification on main, then run `cargo clean` in both main and feature worktrees and fast-forward the feature branch to main.
