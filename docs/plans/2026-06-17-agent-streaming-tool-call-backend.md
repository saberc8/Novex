# Agent Streaming Tool Call Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect the runtime streaming tool-call parser to the backend model-loop provider stream so Novex can detect complete streamed tool-call JSON and expose it through trace/eval metadata.

**Architecture:** Keep the existing final-answer model loop as the execution authority for this slice. Add a small backend stream state beside `drain_model_delta_events` that feeds provider delta chunks into `StreamingModelTurnParser`, emits one `model_stream_tool_call` inference event when a complete `tool_call` or `tool_calls` JSON object is recognized, and leaves malformed or non-tool JSON streams to the existing final parser. Extend trace/eval classification so enterprise rollout gates can detect streamed tool-call behavior without changing approval or tool execution semantics yet.

**Tech Stack:** Rust, `novex-agent-runtime`, existing `ModelProviderStreamEvent`, backend run events, `novex-trace`, `novex-eval`.

## Global Constraints

- Do not execute tools early in this slice; existing `parse_model_turn_output(&model_response.answer)` remains the execution authority.
- Preserve existing `model_delta` events and final `model_inference` metadata.
- Stream parser errors must not fail the model call; invalid/non-tool streams fall back to existing final parsing.
- Emit at most one streamed tool-call detection event per provider call.
- Keep the event payload small, typed, and usable by trace/eval tags.

---

### Task 1: Backend Stream Detection Event

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Interfaces:**
- Consumes: `StreamingModelTurnParser::push_delta(&mut self, delta: &str)`.
- Produces: `ModelLoopProviderStreamState::observe_tool_call(&mut self, event: &ModelProviderStreamEvent) -> Option<Value>`.
- Produces event payload with `item.type = "model_stream_tool_call"`, `source = "provider_stream"`, `deltaIndex`, `toolCallCount`, and `toolCalls`.

- [ ] **Step 1: Write the failing detection test**

Add a backend unit test that pushes two chunks into `ModelLoopProviderStreamState`, verifies the first returns `None`, and verifies the second returns a `model_stream_tool_call` payload with route/provider/model metadata and the parsed tool call.

- [ ] **Step 2: Run the focused test red**

Run: `cargo test -p backend provider_stream_tool_call --offline`

Expected: FAIL because `ModelLoopProviderStreamState` does not exist.

- [ ] **Step 3: Implement minimal stream state**

Add `ModelLoopProviderStreamState` near the provider stream helper functions. It owns `StreamingModelTurnParser`, disables itself after parser error, and emits at most one detection payload after `StreamingModelTurnParseStatus::Ready`.

- [ ] **Step 4: Wire state into provider stream drain**

Instantiate the state in `await_model_loop_provider_future_or_cancelled_with_delta_events`, pass it to `drain_model_delta_events`, always append the existing `model_delta` event first, then append the `model_stream_tool_call` thought event when available.

- [ ] **Step 5: Verify green**

Run: `cargo test -p backend provider_stream_tool_call --offline`

Expected: PASS.

---

### Task 2: Trace And Eval Classification

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `crates/novex-eval/src/lib.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Consumes: backend run-event payload with `item.type = "model_stream_tool_call"`.
- Produces: trace inference events for `model_stream_tool_call`.
- Produces eval tags `streamingToolCallDetected`, `streamingToolCallCount`, and `streamingToolCodes`.

- [ ] **Step 1: Write failing trace/eval tests**

Add a backend trace conversion test proving a thought event with `model_stream_tool_call` becomes `TraceEventKind::Inference`. Add an eval test proving a trace bundle containing this inference span produces the streamed tool-call tags.

- [ ] **Step 2: Run focused tests red**

Run: `cargo test -p backend model_stream_tool_call --offline && cargo test -p novex-eval streaming_tool_call --offline`

Expected: FAIL because the new item type is not classified and eval tags are missing.

- [ ] **Step 3: Implement trace/eval classification**

Add `model_stream_tool_call` to `is_model_inference_trace_item`. Extend `TraceInferenceSummary` with stream tool-call count and tool-code collection, and insert the three eval tags when count is greater than zero.

- [ ] **Step 4: Update migration matrix**

Move the backend gap from "backend stream-native execution of parsed tool calls" to "backend early execution/approval from streamed tool-call detection" and add the new focused test commands.

- [ ] **Step 5: Verify green**

Run:

```bash
cargo fmt -- --check
cargo test -p backend provider_stream_tool_call --offline
cargo test -p backend model_stream_tool_call --offline
cargo test -p novex-eval streaming_tool_call --offline
cargo test --workspace --offline
git diff --check
```

Expected: all commands pass.
