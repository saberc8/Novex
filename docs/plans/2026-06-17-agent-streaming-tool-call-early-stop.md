# Agent Streaming Tool Call Early Stop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop waiting for the provider once a complete streamed tool-call turn is detected, and let the existing model loop immediately continue into tool approval/execution with that parsed turn.

**Architecture:** Keep provider delta and `model_stream_tool_call` event persistence as the detection source. Extend the provider completion envelope so `response` can be absent when the helper exits because streamed tool-call JSON was complete; the model loop then skips final `ModelChatResp` parsing and uses the streamed `ParsedModelTurnOutput`. Dropping the pinned provider future is the local provider early-stop mechanism for this slice; provider-native remote cancellation remains a follow-up because many providers do not expose a response id until later in the stream.

**Tech Stack:** Rust, Tokio `select!`, backend model loop, `novex-agent-runtime::StreamingModelTurnParser`, existing Agent run-event persistence.

## Global Constraints

- Do not change tool approval, risk, or execution policy.
- Do not fabricate a full `ModelChatResp` when early-stopping before provider completion.
- Preserve normal provider-completion behavior for final answers, non-tool streams, and parser-disabled streams.
- Preserve local runtime-registry and persistent DB cancellation priority over streamed tool-call early-stop.
- Keep `model_delta` and `model_stream_tool_call` event payloads backward compatible.

---

### Task 1: Make Provider Completion Reason Explicit

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Interfaces:**
- Produces: `ModelLoopProviderCompletionReason::{ProviderCompleted, StreamedToolCallDetected}`.
- Changes: `ModelLoopProviderCompletion<T> { response: Option<T>, streamed_tool_call_output: Option<ParsedModelTurnOutput>, completion_reason: ModelLoopProviderCompletionReason }`.

- [ ] **Step 1: Write the failing source-contract test**

Add a backend unit test named `streamed_tool_call_early_stop_completion_contract_is_explicit` that reads `agent_service.rs` and asserts the production source contains `enum ModelLoopProviderCompletionReason`, `ProviderCompleted`, `StreamedToolCallDetected`, `response: Option<T>`, and `completion_reason: ModelLoopProviderCompletionReason`.

- [ ] **Step 2: Run the focused test red**

Run: `cargo test -p backend-rust streamed_tool_call_early_stop --offline`

Expected: FAIL because the completion reason enum and optional response contract do not exist.

- [ ] **Step 3: Implement the minimal contract**

Add the completion reason enum and update `ModelLoopProviderCompletion<T>` to carry `Option<T>` plus the reason. Update the provider-completed branch to wrap `Some(response)` and set `ProviderCompleted`.

- [ ] **Step 4: Verify green**

Run: `cargo test -p backend-rust streamed_tool_call_early_stop --offline`

Expected: PASS for the contract test.

---

### Task 2: Return Early When Streamed Tool Call Is Complete

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Interfaces:**
- Consumes: `ModelLoopProviderStreamState::detected_tool_call_output(&self) -> Option<ParsedModelTurnOutput>`.
- Produces helper behavior: `await_model_loop_provider_future_or_cancelled_with_delta_events` returns `Completed(ModelLoopProviderCompletion { response: None, streamed_tool_call_output: Some(parsed), completion_reason: StreamedToolCallDetected })` immediately after `drain_model_delta_events` records the detecting stream event.

- [ ] **Step 1: Write the failing async behavior test**

Add a backend Tokio test named `streamed_tool_call_early_stop_returns_before_provider_future_finishes`. It should create a provider stream receiver, send two chunks that form a complete `rag.search` tool call, call a small no-persistence helper around `ModelLoopProviderStreamState`, and assert the returned completion has `response == None`, `completion_reason == StreamedToolCallDetected`, and the parsed tool call output.

- [ ] **Step 2: Run the focused test red**

Run: `cargo test -p backend-rust streamed_tool_call_early_stop --offline`

Expected: FAIL because stream detection is retained but not yet a helper-level early-stop signal.

- [ ] **Step 3: Implement early-stop decision helper**

Introduce `fn model_loop_streamed_tool_call_completion<T>(stream_state: &ModelLoopProviderStreamState) -> Option<ModelLoopProviderCompletion<T>>` that returns the early-stop completion when a parsed streamed tool call exists. In `await_model_loop_provider_future_or_cancelled_with_delta_events`, call it immediately after `drain_model_delta_events`; if it returns `Some`, return `ModelLoopFutureAwait::Completed(completion)` and let dropping the provider future abort local work.

- [ ] **Step 4: Verify green**

Run: `cargo test -p backend-rust streamed_tool_call_early_stop --offline`

Expected: PASS.

---

### Task 3: Let The Model Loop Continue Without A Full Provider Response

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Produces: `model_loop_parse_turn_output(response: Option<&ModelChatResp>, streamed_tool_call_output: Option<&ParsedModelTurnOutput>) -> Result<ParsedModelTurnOutput, AppError>`.
- Preserves: provider-completed final-answer path still appends `model_inference` and parses final answer when no streamed tool call exists.

- [ ] **Step 1: Write failing parse/source tests**

Add tests proving `model_loop_parse_turn_output(None, Some(&streamed))` returns the streamed output, and `execute_model_loop_existing_run` only requires a `ModelChatResp` when the provider completion has `Some(response)`.

- [ ] **Step 2: Run focused tests red**

Run: `cargo test -p backend-rust streamed_tool_call_early_stop --offline`

Expected: FAIL because `model_loop_parse_turn_output` still requires `&ModelChatResp` and the loop still unwraps a response.

- [ ] **Step 3: Wire the model loop**

Change the retry loop to store `Option<ModelChatResp>` from `completion.response`. If a response exists, append the existing `model_inference` event. Then call `model_loop_parse_turn_output(model_response.as_ref(), streamed_tool_call_output.as_ref())`. If neither response nor streamed output exists, return the existing parse-style bad request error with a clear missing-response message.

- [ ] **Step 4: Update matrix and verify**

Update the Runtime loop and Runtime loop POC rows to say streamed tool-call detection now early-stops local provider await and immediately enters the existing approval/execution path. Add `cargo test -p backend-rust streamed_tool_call_early_stop --offline` to the focused command list.

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust streamed_tool_call_early_stop --offline
cargo test -p backend-rust streamed_tool_call_decision --offline
cargo test -p backend-rust provider_stream_tool_call --offline
cargo test --workspace --offline
git diff --check
```

Expected: all commands pass.
