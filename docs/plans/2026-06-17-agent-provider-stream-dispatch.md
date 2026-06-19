# Agent Provider Stream Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the provider HTTP/SSE dispatch decision so Agent streaming calls explicitly execute through a stream provider path, while unary calls keep unary response parsing.

**Architecture:** Adapter-port Codex's explicit transport mode boundary without discarding Novex enterprise controls. Route resolution, fallback, circuit breaker, provider-call lease, heartbeat, usage/cost accounting, and stream event emission stay in the existing service path; only the final provider request execution receives an explicit `Unary` or `Stream` dispatch mode.

**Tech Stack:** Rust, Tokio, reqwest streaming response chunks, backend model runtime service, provider-call leases, source-contract tests.

## Global Constraints

- Do not change model request payload shape, SSE parsing semantics, provider-call lease persistence, fallback policy, cost accounting, or Agent event payload shape.
- `chat_completion_for_purpose`, chat flow, chat history, compaction, embedding, rerank, and media routes must keep unary behavior unless they already intentionally set stream payloads through their own request metadata.
- `chat_completion_stream_for_purpose` must be the path that selects stream dispatch.
- Stream dispatch must still use `ModelChatStreamCompletionBuilder` and emit `ModelProviderStreamEvent` chunks with provider response id/status and provider-call lease id.
- This slice only introduces the dispatch mode boundary. It does not add a new provider client crate or remove the existing `reqwest` client.

---

### Task 1: Add Provider Dispatch Mode Boundary

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Adds private enum `ModelProviderDispatchMode { Unary, Stream }`.
- `execute_normalized_chat_completion_with_fallback` receives a dispatch mode and passes it through fallback attempts.
- `execute_normalized_chat_completion_with_provider_call_lease` receives a dispatch mode and passes it into route execution.
- `execute_normalized_chat_completion_with_route` receives a dispatch mode and uses it to select stream or unary response parsing.

- [ ] **Step 1: Write failing source-contract test**

Add backend test `model_provider_stream_dispatch_mode_is_explicit`. It should inspect production source before `#[cfg(test)]` and assert:

```rust
assert!(source.contains("enum ModelProviderDispatchMode"));
assert!(source.contains("ModelProviderDispatchMode::Unary"));
assert!(source.contains("ModelProviderDispatchMode::Stream"));
```

Then inspect the body from `execute_chat_completion_stream_transport` through `execute_normalized_chat_completion_with_provider_call_lease` and assert it contains `ModelProviderDispatchMode::Stream`. Inspect the unary `chat_completion_for_purpose` body and assert it contains `ModelProviderDispatchMode::Unary`.

- [ ] **Step 2: Verify red**

Run: `cargo test -p backend model_provider_stream_dispatch_mode --offline`

Expected: FAIL because dispatch mode does not exist yet.

- [ ] **Step 3: Implement minimal mode threading**

Add the enum near `ModelChatProviderTransport`. Update method signatures and call sites:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelProviderDispatchMode {
    Unary,
    Stream,
}
```

Pass `ModelProviderDispatchMode::Unary` from existing unary entry points and `ModelProviderDispatchMode::Stream` from `execute_chat_completion_stream_transport`.

- [ ] **Step 4: Verify green**

Run: `cargo test -p backend model_provider_stream_dispatch_mode --offline`

Expected: PASS.

---

### Task 2: Make Route Execution Honor Dispatch Mode

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- `execute_normalized_chat_completion_with_route(route, command, conversation_id, dispatch_mode)` returns streamed provider output only when `dispatch_mode == Stream` and the request supports stream response parsing.
- Unary mode always parses the final response text by transport, including legacy bodies that look like SSE text.

- [ ] **Step 1: Write failing dispatch-path test**

Add backend test `model_provider_stream_dispatch_route_path_separates_unary_and_stream`. It should inspect `execute_normalized_chat_completion_with_route` and assert the stream branch is guarded by `matches!(dispatch_mode, ModelProviderDispatchMode::Stream)` and the unary body-text parsing happens outside that branch.

- [ ] **Step 2: Verify red**

Run: `cargo test -p backend model_provider_stream_dispatch_route_path --offline`

Expected: FAIL until route execution receives and checks the mode.

- [ ] **Step 3: Implement route-level split**

Inside `execute_normalized_chat_completion_with_route`, replace the implicit stream decision:

```rust
if model_chat_provider_request_streams_chat_completion(&provider_request) {
```

with an explicit dispatch-mode guard:

```rust
if matches!(dispatch_mode, ModelProviderDispatchMode::Stream)
    && model_chat_provider_request_streams_chat_completion(&provider_request)
{
```

Keep the existing unary text parsing path for all other cases.

- [ ] **Step 4: Verify green**

Run: `cargo test -p backend model_provider_stream_dispatch_route_path --offline`

Expected: PASS.

---

### Task 3: Update Matrix And Regression Gates

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Runtime loop matrix status advances from `slice-59 implemented` to `slice-60 implemented`.
- Runtime loop notes mention explicit provider dispatch mode for stream vs unary route execution.
- Acceptance evidence includes `model_provider_stream_dispatch_mode` and `model_provider_stream_dispatch_route_path`.
- Remaining gap narrows to extracting provider client modules and provider-native stream cancellation/drain hardening.

- [ ] **Step 1: Update migration matrix**

Add the new dispatch boundary to the Runtime loop row and Runtime loop POC evidence. Add this plan to Follow-up Implementation Plans.

- [ ] **Step 2: Verify**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p backend model_provider_stream_dispatch_mode --offline
cargo test -p backend model_provider_stream_dispatch_route_path --offline
cargo test -p backend model_stream_transport_executor --offline
cargo test -p backend model_stream_transport_task --offline
cargo test -p backend model_stream_completion_builder --offline
cargo test -p backend streamed_tool_call_early_stop --offline
cargo test -p backend provider_abort --offline
cargo test --workspace --offline
```

Expected: all commands pass.
