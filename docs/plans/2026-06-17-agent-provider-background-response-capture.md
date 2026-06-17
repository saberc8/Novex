# Agent Provider Background Response Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Capture background OpenAI-compatible Responses ids/statuses from provider calls and persist them into provider-call leases so native cancellation has a durable provider join key.

**Architecture:** Extend the existing Responses compaction v2 transport rather than adding a new runtime path. The request remains a streaming Responses compaction request, but now opts into background/store mode. The parser captures provider response metadata from JSON and SSE payloads, carries it through `ModelChatResp`, and writes it to the lease completion payload.

**Tech Stack:** Rust, serde_json, reqwest response parsing, existing `ModelRuntimeService`, existing `ai_model_provider_call_lease` table.

## Global Constraints

- TDD: write failing tests first and verify red before production code.
- Do not add WebSocket transport in this slice.
- Do not add a provider polling worker in this slice.
- Do not leak final answer text or prompt content into provider-call lease payloads.
- Do not mark the persistent goal complete after this slice.
- Merge the feature worktree back to main after verification.
- Run `cargo clean` in both worktrees after the stage completes.

---

### Task 1: Background Responses Request Contract

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`
- Modify: `docs/plans/2026-06-17-agent-provider-background-response-capture.md`

**Interfaces:**
- Produces: `model_chat_responses_compaction_payload(...)` with `background=true`, `store=true`, `stream=true`.

- [ ] **Step 1: Write failing test**

Add:

```rust
#[test]
fn provider_background_response_capture_payload_marks_responses_background() {
    let route = openai_compatible_llm_route();
    let command = test_compaction_chat_command();

    let request = model_chat_provider_request(&route, &command);

    assert_eq!(
        request.transport,
        ModelChatProviderTransport::ResponsesCompactionV2
    );
    assert_eq!(request.payload["background"], true);
    assert_eq!(request.payload["store"], true);
    assert_eq!(request.payload["stream"], true);
    assert_eq!(request.payload["metadata"]["request_kind"], "compaction");
}
```

- [ ] **Step 2: Run red test**

Run:

```bash
cargo test -p backend-rust provider_background_response_capture_payload --offline
```

Expected: FAIL because `background` and `store` are not present.

- [ ] **Step 3: Implement minimal request payload change**

Add `background=true` and `store=true` to `model_chat_responses_compaction_payload`.

- [ ] **Step 4: Run green test**

Run:

```bash
cargo test -p backend-rust provider_background_response_capture_payload --offline
```

Expected: PASS.

### Task 2: Provider Response Metadata Parser

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Modifies: `ModelChatCompactionProviderOutput`
- Produces: `provider_response_id: Option<String>`
- Produces: `provider_response_status: Option<String>`

- [ ] **Step 1: Write failing tests**

Add:

```rust
#[test]
fn provider_background_response_capture_parses_json_response_metadata() {
    let body = json!({
        "id": "resp_bg_123",
        "status": "completed",
        "output": [
            { "type": "compaction", "encrypted_content": "compact summary" }
        ],
        "usage": {
            "input_tokens": 10,
            "output_tokens": 2,
            "total_tokens": 12
        }
    });

    let output = model_chat_compaction_provider_output_from_body(&body).unwrap();

    assert_eq!(output.answer, "compact summary");
    assert_eq!(output.provider_response_id.as_deref(), Some("resp_bg_123"));
    assert_eq!(output.provider_response_status.as_deref(), Some("completed"));
}

#[test]
fn provider_background_response_capture_parses_sse_response_metadata() {
    let sse = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_bg_123\",\"status\":\"in_progress\"}}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"compaction\",\"encrypted_content\":\"sse summary\"}}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_bg_123\",\"status\":\"completed\",\"usage\":{\"input_tokens\":14,\"output_tokens\":3,\"total_tokens\":17}}}\n\n",
    );

    let output = model_chat_compaction_provider_output_from_sse_text(sse).unwrap();

    assert_eq!(output.answer, "sse summary");
    assert_eq!(output.provider_response_id.as_deref(), Some("resp_bg_123"));
    assert_eq!(output.provider_response_status.as_deref(), Some("completed"));
}
```

- [ ] **Step 2: Run red test**

Run:

```bash
cargo test -p backend-rust provider_background_response_capture_parses --offline
```

Expected: FAIL because the output type does not expose provider response metadata.

- [ ] **Step 3: Implement parser fields**

Add optional metadata fields to `ModelChatCompactionProviderOutput`, extract root JSON `id/status`, and update SSE parsing so terminal `response.completed.response.id/status` wins over earlier in-progress values.

- [ ] **Step 4: Run green test**

Run:

```bash
cargo test -p backend-rust provider_background_response_capture_parses --offline
```

Expected: PASS.

### Task 3: Lease Completion Evidence

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Modifies: `ModelChatResp`
- Modifies: `model_provider_call_lease_completion_from_response(...)`

- [ ] **Step 1: Write failing test**

Add:

```rust
#[test]
fn provider_background_response_capture_persists_provider_id_for_cancel() {
    let now =
        NaiveDateTime::parse_from_str("2026-06-17 10:00:05", "%Y-%m-%d %H:%M:%S").unwrap();
    let response = ModelChatResp {
        conversation_id: None,
        answer: "sensitive-user-content-needle".to_owned(),
        route_id: "tenant42.code_agent".to_owned(),
        provider: "openai-compatible".to_owned(),
        model: Some("gpt-compatible".to_owned()),
        latency_ms: 42,
        usage: ModelChatUsage {
            prompt_tokens: Some(11),
            completion_tokens: Some(7),
            total_tokens: Some(18),
        },
        cost_cents: Some(0.42),
        provider_attempts: vec![],
        provider_call_lease_id: None,
        provider_response_id: Some("resp_bg_123".to_owned()),
        provider_response_status: Some("completed".to_owned()),
    };

    let completion = model_provider_call_lease_completion_from_response(&response, 42, now);

    assert_eq!(completion.response_payload["providerResponseId"], "resp_bg_123");
    assert_eq!(
        completion.response_payload["providerResponseStatus"],
        "completed"
    );
    assert!(!completion
        .response_payload
        .to_string()
        .contains("sensitive-user-content-needle"));
}
```

- [ ] **Step 2: Run red test**

Run:

```bash
cargo test -p backend-rust provider_background_response_capture_persists --offline
```

Expected: FAIL because `ModelChatResp` has no provider response metadata fields and lease completion does not persist them.

- [ ] **Step 3: Implement lease propagation**

Add optional provider response metadata to `ModelChatResp`, set it from compaction parser output, default it to `None` in non-Responses constructors and tests, and persist non-empty values in the lease completion payload.

- [ ] **Step 4: Run green test**

Run:

```bash
cargo test -p backend-rust provider_background_response_capture --offline
cargo test -p backend-rust provider_call_lease_cancel --offline
```

Expected: PASS.

### Task 4: Matrix, Verification, Merge

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-17-agent-provider-background-response-capture.md`

- [ ] **Step 1: Update matrix**

Move background Responses response-id capture into Runtime loop evidence, keep WebSocket streaming transport as remaining work, and add this plan to the follow-up implementation list.

- [ ] **Step 2: Verify**

Run:

```bash
cargo fmt -- --check
cargo test -p backend-rust provider_background_response_capture --offline
cargo test -p backend-rust provider_compact_transport --offline
cargo test -p backend-rust provider_call_lease_cancel --offline
cargo test -p backend-rust provider_call_lease --offline
cargo test --workspace --offline
git diff --check
```

Expected: all pass with exit code 0.

- [ ] **Step 3: Commit, merge, clean**

```bash
git add backend/src/application/ai/model_service.rs docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-17-agent-provider-background-response-capture-design.md docs/plans/2026-06-17-agent-provider-background-response-capture.md
git commit -m "feat: capture provider background response metadata"
cd /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex
git merge --no-ff feat/enterprise-agent-foundation -m "merge: enterprise agent foundation provider background response capture"
cargo fmt -- --check
cargo test --workspace --offline
git diff --check
cargo clean
cd /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex/.worktrees/enterprise-agent-foundation
cargo clean
git merge --ff-only main
```
