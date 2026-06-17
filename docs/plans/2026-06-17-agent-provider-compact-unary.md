# Agent Provider Compact Unary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Codex-style unary `/responses/compact` provider transport parity while preserving the existing Responses v2 compaction path.

**Architecture:** Keep `ModelChatCommand` as the stable Novex model-runtime boundary. Extend the internal provider request plan with `ResponsesCompactUnary`, selected by `ModelChatCompactionMetadata.implementation = "responses_compaction_unary"`. Reuse the existing strict JSON compaction response parser and leave the v2 `compaction_trigger` transport unchanged.

**Tech Stack:** Rust, serde_json, reqwest, existing backend model runtime tests.

## Global Constraints

- Do not change the default Agent model-loop compaction implementation in this slice.
- Do not add WebSocket transport or `x-codex-turn-state` header persistence.
- Keep unsupported providers on chat completions.
- Follow TDD: write red tests before production code.
- Commit, merge to `main`, rerun verification on `main`, run `cargo clean` in both worktrees, then sync feature back to `main`.

---

### Task 1: Unary Compact Request Contract

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Produces: `ModelChatProviderTransport::ResponsesCompactUnary`
- Produces: `model_chat_responses_compact_unary_endpoint(route: &ModelRuntimeRoute) -> String`
- Produces: `model_chat_responses_compact_unary_payload(route: &ModelRuntimeRoute, command: &ModelChatCommand) -> Value`

- [x] **Step 1: Write failing tests**

Add tests under `provider_compact_transport` coverage:

```rust
#[test]
fn provider_compact_unary_uses_responses_compact_endpoint_for_unary_implementation() {
    let route = openai_compatible_llm_route();
    let command = test_unary_compaction_chat_command();

    let request = model_chat_provider_request(&route, &command);

    assert_eq!(request.transport, ModelChatProviderTransport::ResponsesCompactUnary);
    assert_eq!(request.endpoint, "https://llm.internal/v1/responses/compact");
    assert_eq!(request.payload["model"], route.model().unwrap());
    assert!(request.payload.get("stream").is_none());
    assert_eq!(request.payload["metadata"]["request_kind"], "compaction");
    assert_eq!(
        request.payload["metadata"]["compaction_implementation"],
        "responses_compaction_unary"
    );
    assert_ne!(
        request
            .payload
            .get("input")
            .and_then(Value::as_array)
            .and_then(|items| items.last())
            .and_then(|item| item.get("type"))
            .and_then(Value::as_str),
        Some("compaction_trigger")
    );
}
```

Also add:

```rust
#[test]
fn provider_compact_v2_keeps_responses_trigger_transport() {
    let route = openai_compatible_llm_route();
    let command = test_compaction_chat_command();

    let request = model_chat_provider_request(&route, &command);

    assert_eq!(request.transport, ModelChatProviderTransport::ResponsesCompactionV2);
    assert_eq!(request.endpoint, "https://llm.internal/v1/responses");
    assert_eq!(request.payload["stream"], true);
    assert_eq!(
        request
            .payload
            .get("input")
            .and_then(Value::as_array)
            .and_then(|items| items.last())
            .and_then(|item| item.get("type"))
            .and_then(Value::as_str),
        Some("compaction_trigger")
    );
}
```

- [x] **Step 2: Run red tests**

Run: `cargo test -p backend-rust provider_compact_unary --offline`

Result: FAIL as expected because `ResponsesCompactUnary` did not exist.

- [x] **Step 3: Implement minimal request plan**

Extend:

```rust
enum ModelChatProviderTransport {
    ChatCompletions,
    ResponsesCompactionV2,
    ResponsesCompactUnary,
}
```

Add implementation selection:

```rust
fn model_chat_compaction_implementation(command: &ModelChatCommand) -> Option<&str> {
    command
        .request_metadata
        .as_ref()
        .and_then(|metadata| metadata.compaction.as_ref())
        .map(|compaction| compaction.implementation.as_str())
}
```

In `model_chat_provider_request`, choose unary only when:

```rust
model_chat_route_supports_responses_compaction(route)
    && matches!(model_chat_compaction_implementation(command), Some("responses_compaction_unary"))
```

Add:

```rust
fn model_chat_responses_compact_unary_endpoint(route: &ModelRuntimeRoute) -> String {
    join_model_endpoint(route.base_url(), Some("responses/compact"))
}
```

Add unary payload:

```rust
fn model_chat_responses_compact_unary_payload(
    route: &ModelRuntimeRoute,
    command: &ModelChatCommand,
) -> Value {
    let mut payload = json!({
        "model": route.model().unwrap_or_default(),
        "input": model_chat_message_input_items(command),
        "tools": [],
        "parallel_tool_calls": false,
    });
    if let Some(metadata) = model_chat_provider_metadata(route, command) {
        payload["metadata"] = metadata;
    }
    payload
}
```

- [x] **Step 4: Run green focused tests**

Run: `cargo test -p backend-rust provider_compact_unary --offline`

Result: PASS.

### Task 2: Unary Response Parser Wiring

Status: Completed.

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Consumes: `ModelChatProviderTransport::ResponsesCompactUnary`
- Consumes: `model_chat_compaction_provider_output_from_body`

- [x] **Step 1: Write failing parser wiring test**

Add:

```rust
#[test]
fn provider_compact_unary_reuses_json_compaction_response_parser() {
    let route = openai_compatible_llm_route();
    let body = json!({
        "output": [
            { "type": "compaction", "encrypted_content": "unary compact summary" }
        ],
        "usage": {
            "input_tokens": 21,
            "output_tokens": 4,
            "total_tokens": 25
        }
    });

    let response =
        model_chat_response_from_responses_compaction_body(&route, body, 55, None).unwrap();

    assert_eq!(response.answer, "unary compact summary");
    assert_eq!(response.usage.prompt_tokens, Some(21));
    assert_eq!(response.usage.completion_tokens, Some(4));
    assert_eq!(response.usage.total_tokens, Some(25));
}
```

- [x] **Step 2: Run red tests**

Run: `cargo test -p backend-rust provider_compact_unary --offline`

Result: Covered by the first red compile failure for the new unary transport; parser wiring was then added before green.

- [x] **Step 3: Wire execution parser**

Add a match arm:

```rust
ModelChatProviderTransport::ResponsesCompactUnary => {
    model_chat_response_from_responses_compaction_text(
        route,
        &body_text,
        started.elapsed().as_millis(),
        conversation_id,
    )
}
```

This parser already accepts JSON body text and enforces exactly one compaction output item.

- [x] **Step 4: Run focused regression tests**

Run:

```bash
cargo test -p backend-rust provider_compact_unary --offline
cargo test -p backend-rust provider_compact_transport --offline
cargo test -p backend-rust remote_compaction --offline
cargo test -p backend-rust model_loop_compaction --offline
```

Expected: PASS.

### Task 3: Docs, Matrix, Verification, Merge

Status: In Progress.

**Files:**
- Create: `docs/plans/2026-06-17-agent-provider-compact-unary-design.md`
- Create: `docs/plans/2026-06-17-agent-provider-compact-unary.md`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

- [x] **Step 1: Update migration matrix**

Move unary provider `/responses/compact` parity from remaining runtime-loop work into implemented evidence. Keep WebSocket streaming transport and provider-native cancel endpoints as next.

- [x] **Step 2: Run full verification**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
git diff --check
```

Expected: PASS.

- [ ] **Step 3: Commit, merge, clean**

Commit feature work, merge `feat/enterprise-agent-foundation` into `main`, rerun full verification on `main`, run `cargo clean` in both worktrees, and sync feature to `main`.

**Verification evidence so far:**
- Red: `cargo test -p backend-rust provider_compact_unary --offline` failed on missing `ModelChatProviderTransport::ResponsesCompactUnary`.
- Green: `cargo test -p backend-rust provider_compact_unary --offline`
- Green: `cargo test -p backend-rust provider_compact_transport --offline`
- Green: `cargo test -p backend-rust remote_compaction --offline`
- Green: `cargo test -p backend-rust model_loop_compaction --offline`
- Green: `cargo fmt -- --check`
- Green: `cargo test --workspace --offline`
- Green: `git diff --check`
