# Agent Streaming Tool Call Decision Adoption Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the backend model loop use the parsed streamed tool-call turn detected during provider streaming as the authoritative turn decision for that model call.

**Architecture:** Keep `model_delta` and `model_stream_tool_call` event emission unchanged. Extend the provider-stream wait helper to return a small completion envelope containing the provider response plus the streamed parsed `ParsedModelTurnOutput`, and route model-loop parsing through a helper that prefers that streamed parsed output over reparsing the final answer text. This moves streamed tool-call detection into the decision path while still waiting for provider completion; provider early cancellation and immediate approval/execution remain the next slice.

**Tech Stack:** Rust, backend model loop, `novex-agent-runtime::ParsedModelTurnOutput`, existing provider stream state and `parse_model_turn_output`.

## Global Constraints

- Do not change tool approval or execution semantics in this slice.
- Do not cancel provider calls early in this slice.
- Keep compaction and non-stream provider wait helpers unchanged.
- If no streamed tool-call output exists, preserve the existing full-answer parse behavior and error message.
- Stream parser errors remain non-terminal; final answer parsing is still the fallback authority.

---

### Task 1: Retain Streamed Parsed Tool Call Output

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Interfaces:**
- Consumes: `ModelLoopProviderStreamState::observe_tool_call(&mut self, event: &ModelProviderStreamEvent) -> Option<Value>`.
- Produces: `ModelLoopProviderStreamState::detected_tool_call_output(&self) -> Option<ParsedModelTurnOutput>`.

- [ ] **Step 1: Write the failing retention test**

Add a backend unit test that feeds two JSON chunks into `ModelLoopProviderStreamState`, verifies the detection payload is emitted, then calls `detected_tool_call_output()` and asserts the retained `ParsedModelTurnOutput` contains `AgentTurnItem::tool_call("call-1", "rag.search", {"query":"policy"})`.

- [ ] **Step 2: Run the focused test red**

Run: `cargo test -p backend-rust streamed_tool_call_output --offline`

Expected: FAIL because `detected_tool_call_output` does not exist.

- [ ] **Step 3: Implement minimal retention**

Add `detected_tool_call_output: Option<ParsedModelTurnOutput>` to `ModelLoopProviderStreamState`. When `observe_tool_call` receives `Ready(parsed)`, clone/store `parsed` before building the event payload. Add `detected_tool_call_output(&self) -> Option<ParsedModelTurnOutput>` that returns a clone.

- [ ] **Step 4: Verify green**

Run: `cargo test -p backend-rust streamed_tool_call_output --offline`

Expected: PASS.

---

### Task 2: Prefer Streamed Parsed Turn In Model Loop Decision

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Produces: `ModelLoopProviderCompletion<T> { response: T, streamed_tool_call_output: Option<ParsedModelTurnOutput> }`.
- Produces: `model_loop_parse_turn_output(response: &ModelChatResp, streamed_tool_call_output: Option<&ParsedModelTurnOutput>) -> Result<ParsedModelTurnOutput, AppError>`.

- [ ] **Step 1: Write failing model-loop parse helper test**

Add a backend unit test that builds a streamed `ParsedModelTurnOutput` for `rag.search`, a `ModelChatResp` whose `answer` is plain text, and asserts `model_loop_parse_turn_output(&response, Some(&streamed))` returns the streamed tool call rather than a final answer.

- [ ] **Step 2: Write failing source contract test**

Add a backend source-contract test proving `execute_model_loop_existing_run` receives `streamed_tool_call_output` from `ModelLoopProviderCompletion` and calls `model_loop_parse_turn_output(&model_response, streamed_tool_call_output.as_ref())`.

- [ ] **Step 3: Run focused tests red**

Run: `cargo test -p backend-rust streamed_tool_call_decision --offline`

Expected: FAIL because the helper/envelope/wiring do not exist.

- [ ] **Step 4: Implement completion envelope and helper**

Change `await_model_loop_provider_future_or_cancelled_with_delta_events` to return `ModelLoopFutureAwait<ModelLoopProviderCompletion<T>>`. On provider completion, drain remaining stream events, then return `ModelLoopProviderCompletion { response, streamed_tool_call_output: stream_state.detected_tool_call_output() }`. Add `model_loop_parse_turn_output` that returns the streamed output when provided, otherwise calls `parse_model_turn_output(&response.answer)` with the existing Chinese error message.

- [ ] **Step 5: Wire model loop and update matrix**

In `execute_model_loop_existing_run`, store `streamed_tool_call_output` alongside `model_response`, and replace the direct `parse_model_turn_output(&model_response.answer)` call with `model_loop_parse_turn_output(&model_response, streamed_tool_call_output.as_ref())`. Update the migration matrix to say streamed parsed tool-call output now participates in backend model-loop turn decisions; provider early cancellation and immediate approval/execution remain next.

- [ ] **Step 6: Verify green**

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust streamed_tool_call_output --offline
cargo test -p backend-rust streamed_tool_call_decision --offline
cargo test -p backend-rust provider_stream_tool_call --offline
cargo test --workspace --offline
git diff --check
```

Expected: all commands pass.
