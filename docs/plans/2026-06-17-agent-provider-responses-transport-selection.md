# Agent Provider Responses Transport Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let configured CodeAgent model routes opt into OpenAI Responses transport by setting the route deployment `api_path` to `/responses`.

**Architecture:** Keep chat-completions as the default runtime transport. When a compatible CodeAgent route resolves to an endpoint ending in `/responses`, build a Responses-style `input` payload with streaming enabled and parse it through the existing provider delta channel. This uses the existing model registry `ai_model_deployment.api_path` configuration surface instead of adding a new schema column in this slice.

**Tech Stack:** Rust backend, `novex-model` route resolution, existing model provider request builder, OpenAI-compatible Responses payload shape.

## Global Constraints

- Preserve current `/chat/completions` behavior for configured and env routes.
- Do not switch all CodeAgent routes to Responses implicitly.
- Do not add a DB migration in this slice.
- Preserve provider metadata for model-loop request evidence.
- Keep compaction transports unchanged.

---

### Task 1: Configured Responses Endpoint Selects Responses Transport

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Consumes: `ModelRuntimeRoute.endpoint()` resolved from `ai_model_deployment.api_path`.
- Produces: `ModelChatProviderTransport::ResponsesCodeAgent` and `model_chat_responses_code_agent_payload(route, command)`.

- [ ] **Step 1: Write the failing transport test**

Add a unit test near `provider_token_delta_code_agent_chat_request_enables_streaming`:

```rust
#[test]
fn provider_responses_transport_code_agent_request_uses_configured_responses_endpoint() {
    let row = dynamic_route_test_row(
        "tenant42.code_agent.responses",
        "code_agent",
        "llm",
        Some("/responses"),
        Some("env:LLM_PRIVATE_KEY"),
    );
    let route = runtime_route_from_registry_row(&row, |key| match key {
        "LLM_PRIVATE_KEY" => Some("sk-fake-private-secret-0001".to_owned()),
        _ => None,
    })
    .unwrap();
    let command = test_code_agent_chat_command();

    let request = model_chat_provider_request(&route, &command);

    assert_eq!(
        request.transport,
        ModelChatProviderTransport::ResponsesCodeAgent
    );
    assert_eq!(request.endpoint, "https://llm.internal/v1/responses");
    assert_eq!(request.payload["model"], "qwen-private");
    assert_eq!(request.payload["stream"], true);
    assert_eq!(
        request.payload["max_output_tokens"],
        json!(command.max_tokens.unwrap())
    );
    assert!(request.payload.get("input").is_some());
    assert!(request.payload.get("messages").is_none());
    assert!(request.payload.get("max_tokens").is_none());
}
```

- [ ] **Step 2: Run red verification**

Run:

```bash
cargo test -p backend-rust provider_responses_transport_code_agent_request_uses_configured_responses_endpoint --offline
```

Expected: FAIL because `ResponsesCodeAgent` and the payload branch do not exist.

- [ ] **Step 3: Implement minimal transport selection**

Add:

```rust
ResponsesCodeAgent,
```

to `ModelChatProviderTransport`.

Add a branch before the default chat-completions request:

```rust
if model_chat_route_uses_responses_code_agent(route, command) {
    return ModelChatProviderRequest {
        endpoint: route.endpoint().to_owned(),
        payload: model_chat_responses_code_agent_payload(route, command),
        transport: ModelChatProviderTransport::ResponsesCodeAgent,
    };
}
```

Add helper behavior:

- provider must support Responses (`OpenAiCompatible` or `LocalRuntime`)
- command must be non-compaction CodeAgent
- endpoint must end with `/responses`
- payload uses `model_chat_message_input_items(command)`, `max_output_tokens`, `temperature`, `stream: true`, and optional provider metadata

- [ ] **Step 4: Run green verification**

Run:

```bash
cargo test -p backend-rust provider_responses_transport_code_agent_request_uses_configured_responses_endpoint --offline
```

Expected: PASS.

### Task 2: Preserve Existing Chat Transport

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Consumes: existing `provider_token_delta_code_agent_chat_request_enables_streaming` behavior.
- Produces: regression coverage proving `/chat/completions` routes stay on `ChatCompletions`.

- [ ] **Step 1: Extend the existing chat transport test**

Keep the existing `provider_token_delta_code_agent_chat_request_enables_streaming` assertions and add:

```rust
assert!(request.payload.get("messages").is_some());
assert!(request.payload.get("input").is_none());
assert_eq!(request.endpoint, route.endpoint());
```

- [ ] **Step 2: Run focused verification**

Run:

```bash
cargo test -p backend-rust provider_token_delta_code_agent_chat_request_enables_streaming --offline
```

Expected: PASS after Task 1 implementation.

### Task 3: Matrix Update And Integration

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Updates the Runtime loop row and Runtime loop POC evidence.

- [ ] **Step 1: Update matrix wording**

Mention configured `/responses` CodeAgent transport selection in the Runtime loop row and Runtime loop POC evidence. Keep "fully stream-native model runtime API" and "partial tool-call JSON parsing while streaming" as remaining gaps.

- [ ] **Step 2: Run full verification**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 3: Commit and integrate**

Commit feature work, merge `feat/enterprise-agent-foundation` into `main` with `--no-ff`, rerun verification on `main`, run `cargo clean` in both worktrees, and fast-forward the feature branch to `main`.
