# Agent Provider Token Delta Stream Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist live provider token deltas from CodeAgent model-loop calls as replayable run events while keeping the existing unary `ModelChatResp` API.

**Architecture:** Extend `ModelChatCommand` with a local-only stream sink, enable chat-completions SSE for CodeAgent calls, parse provider delta chunks into the final response, and have AgentService drain the sink into `model_delta` run events. The final inference event carries streaming metadata for trace/eval consumers.

**Tech Stack:** Rust, `backend`, `tokio::sync::mpsc`, `serde_json`, existing `ai_run_event` persistence, existing SSE/WebSocket run-event transports.

## Global Constraints

- Do not change database schema in this slice.
- Do not touch browser/frontend rendering in this slice.
- Do not stream compaction or Responses output text deltas in this slice.
- Keep `ModelRuntimeService::chat_completion_for_purpose` as a unary API.
- Preserve existing provider-call lease, retry, fallback, and cancellation behavior.

---

### Task 1: Provider SSE Delta Contract

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Produces: `ModelProviderStreamChunk { index, content, provider_event }`
- Produces: `ModelChatResp.provider_delta_chunks`
- Produces: `model_chat_provider_output_from_sse_text(body_text) -> Result<ModelChatProviderOutput, AppError>`

- [ ] **Step 1: Write failing tests**

Add tests proving CodeAgent requests set `"stream": true` and provider SSE deltas assemble into `ModelChatResp.answer` while preserving chunk order.

- [ ] **Step 2: Verify red**

Run:

```bash
cargo test -p backend provider_token_delta --offline
```

Expected: FAIL because stream chunk types and parser do not exist.

- [ ] **Step 3: Implement minimal provider parser**

Add the stream chunk type, add `provider_delta_chunks` to `ModelChatResp`, parse OpenAI-compatible chat-completions SSE `choices[].delta.content`, and return a normal response assembled from chunks.

- [ ] **Step 4: Verify green**

Run:

```bash
cargo test -p backend provider_token_delta --offline
```

Expected: PASS.

### Task 2: Agent Delta Event Sink

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `backend/src/application/ai/agent_service.rs`

**Interfaces:**
- Consumes: `ModelProviderStreamChunk`
- Produces: `model_delta_event_payload(response metadata, chunk) -> serde_json::Value`
- Produces: model-loop channel drain that appends `RunEventKind::Thought` events with `item.type = "model_delta"`

- [ ] **Step 1: Write failing tests**

Add tests proving AgentService source includes a CodeAgent stream channel and that `model_inference_event_payload` exposes `streaming`, `deltaChunkCount`, and `deltaTextLength`.

- [ ] **Step 2: Verify red**

Run:

```bash
cargo test -p backend model_delta --offline
```

Expected: FAIL because the event sink and inference metadata do not exist.

- [ ] **Step 3: Implement minimal event sink**

Attach an `mpsc::UnboundedSender<ModelProviderStreamChunk>` to CodeAgent commands, drain the receiver while the provider future is active, append `model_delta` events, and preserve existing cancellation behavior.

- [ ] **Step 4: Verify green**

Run:

```bash
cargo test -p backend model_delta --offline
```

Expected: PASS.

### Task 3: Matrix and Full Verification

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Updates Runtime loop row from slice-47 to slice-48.
- Adds focused verification commands for provider token delta streaming.

- [ ] **Step 1: Update migration matrix**

Record that CodeAgent provider token-delta capture and run-event emission are implemented. Keep fully stream-native runtime API, Responses delta streaming, and frontend rendering as follow-up work.

- [ ] **Step 2: Run verification**

Run:

```bash
cargo fmt -- --check
cargo test -p backend provider_token_delta --offline
cargo test -p backend model_delta --offline
cargo test --workspace --offline
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 3: Commit and integrate**

Commit the feature branch, merge it into `main` with `--no-ff`, verify on `main`, run `cargo clean` in both worktrees, and fast-forward the feature branch to `main`.
