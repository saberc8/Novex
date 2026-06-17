# Agent Provider Compact Response Transport Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Codex-shaped provider transport boundary for Agent compaction requests, including Responses payload construction and compaction output parsing.

**Architecture:** Keep Novex `ModelChatCommand` stable. Add an internal provider request plan that selects a Responses-compatible compaction transport for supported routes and otherwise falls back to chat completions. Parse JSON and SSE-style Responses compaction output into the existing `ModelChatResp` contract.

**Tech Stack:** Rust, serde_json, reqwest, Cargo offline tests.

---

### Task 1: Provider Request Plan And Payload

Status: Pending.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add tests that prove:

- compaction commands for OpenAI-compatible routes build a provider request plan using `/responses`;
- compaction payload input ends with `{ "type": "compaction_trigger" }`;
- compaction payload includes flat metadata with `request_kind = compaction`;
- compaction payload uses `max_output_tokens`;
- unsupported providers keep the existing chat-completions endpoint.

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend-rust provider_compact_transport --offline
```

Expected: FAIL because no provider request plan or Responses compaction payload helpers exist.

**Step 3: Implement minimal request plan**

Add internal helpers:

- `ModelChatProviderTransport`
- `ModelChatProviderRequest`
- `model_chat_provider_request(route, command)`
- `model_chat_responses_compaction_endpoint(route)`
- `model_chat_responses_compaction_payload(route, command)`
- `model_chat_message_input_items(command)`

`execute_normalized_chat_completion_with_route(...)` uses the request plan endpoint and payload instead of always using `route.endpoint()` and `model_chat_request_payload(...)`.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p backend-rust provider_compact_transport --offline
```

Expected: PASS.

### Task 2: Responses Compaction Parser

Status: Pending.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Step 1: Write failing tests**

Add tests that prove:

- JSON `output` arrays with one `compaction` item return its encrypted content;
- JSON `compaction_summary` aliases are accepted;
- SSE `response.output_item.done` plus `response.completed` returns the compaction content;
- unrelated output items before compaction are ignored;
- zero, duplicate, or incomplete SSE compaction responses fail.

**Step 2: Verify RED**

Run:

```bash
cargo test -p backend-rust provider_compact_transport --offline
```

Expected: FAIL until parser helpers exist.

**Step 3: Implement parser**

Add helpers:

- `model_chat_response_from_responses_compaction_body(...)`
- `model_chat_response_from_responses_compaction_text(...)`
- `model_chat_compaction_output_from_provider_body(...)`
- `model_chat_compaction_output_from_sse_text(...)`
- `model_chat_compaction_output_item_text(...)`

For Responses compaction transport, read the provider response body as text and parse either JSON or SSE depending on content shape.

**Step 4: Verify GREEN**

Run:

```bash
cargo test -p backend-rust provider_compact_transport --offline
cargo test -p backend-rust remote_compaction --offline
cargo test -p backend-rust model_loop_compaction --offline
```

Expected: PASS.

### Task 3: Documentation, Full Verification, Merge

Status: Pending.

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-provider-compact-response-transport.md`

**Step 1: Update migration matrix**

Move provider compact transport contract and Responses compaction parser into runtime-loop implemented evidence. Leave full Codex `ResponseItem` history installation, WebSocket streaming transport, provider-native cancel endpoints, and provider-call lease tables as follow-ups.

**Step 2: Full verification**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
```

**Step 3: Commit, merge, clean**

Commit the feature worktree, merge into `main`, rerun verification on `main`, then run `cargo clean` in both worktrees and sync the feature worktree to `main`.
