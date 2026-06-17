# Agent Provider Client Chat Request Plan Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move provider-neutral chat/Responses request planning and payload construction from backend route execution into `novex-provider-client`.

**Architecture:** Backend remains responsible for route resolution, command normalization, provider-call leases, fallback, trace/eval context, tenant state, and stream event emission. `novex-provider-client` owns a pure chat plan builder that accepts normalized provider inputs and returns endpoint, payload, and transport kind for chat-completions, Responses CodeAgent streaming, Responses compaction v2, and unary `/responses/compact`.

**Tech Stack:** Rust 2021, Cargo workspace, `serde_json`, `novex-model`, `novex-provider-client`, backend source-contract tests, provider-client unit tests, offline cargo verification.

## Global Constraints

- Do not move `ModelRuntimeRoute`, `ModelChatCommand`, provider-call leases, fallback, tenant context, persistence, trace/eval, stream event emission, or cost accounting into `novex-provider-client`.
- `novex-provider-client` may depend on `novex-model` route/provider vocabulary, but must not depend on `backend-rust`.
- Preserve existing payload shapes for chat-completions, Responses CodeAgent streaming, Responses compaction v2, and unary `/responses/compact`.
- Preserve endpoint selection rules: route endpoint for chat-completions and configured `/responses`, route base URL plus `responses` for compaction v2, and route base URL plus `responses/compact` for unary compaction.
- Preserve provider metadata rules: only OpenAI-compatible, Azure OpenAI, and local-runtime routes receive metadata.
- Keep stream dispatch selection explicit: a plan streams only for chat-completions or Responses CodeAgent transports with `payload.stream == true`.
- Verify with source-contract tests, provider-client planner unit tests, focused backend request-plan tests, formatting, diff checks, and the offline workspace suite.

---

### Task 1: Add Provider-Client Chat Plan Source Contract

**Files:**
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Consumes: existing backend `model_chat_provider_request(route, command)` tests and provider-client crate source.
- Produces: backend source-contract test `provider_client_chat_request_plan_lives_in_provider_client_crate`.

- [ ] **Step 1: Write the failing source-contract test**

Add this test near the existing provider-client source-contract tests:

```rust
#[test]
fn provider_client_chat_request_plan_lives_in_provider_client_crate() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend manifest should live below workspace root");
    let source = |path: &str| {
        std::fs::read_to_string(workspace_root.join(path))
            .unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
    };
    let provider_client_source = source("crates/novex-provider-client/src/lib.rs");
    let service_source = source("backend/src/application/ai/model_service.rs");
    let plan_path = &service_source[service_source
        .find("fn model_chat_provider_request(")
        .unwrap()
        ..service_source.find("fn model_chat_provider_request_streams_chat_completion").unwrap()];

    assert!(provider_client_source.contains("pub enum ModelProviderChatTransport"));
    assert!(provider_client_source.contains("pub struct ModelProviderChatPlanInput"));
    assert!(provider_client_source.contains("pub struct ModelProviderChatPlan"));
    assert!(provider_client_source.contains("pub fn build_model_provider_chat_plan"));
    assert!(provider_client_source.contains("pub fn model_provider_chat_plan_streams_chat_completion"));
    assert!(provider_client_source.contains("\"responses/compact\""));
    assert!(provider_client_source.contains("\"compaction_trigger\""));
    assert!(provider_client_source.contains("\"max_output_tokens\""));
    assert!(provider_client_source.contains("\"response_format\""));
    assert!(plan_path.contains("build_model_provider_chat_plan(ModelProviderChatPlanInput"));
    assert!(!plan_path.contains("json!({"));
    assert!(!plan_path.contains("model_chat_responses_compaction_payload"));
    assert!(!plan_path.contains("model_chat_request_payload"));
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p backend-rust provider_client_chat_request_plan_lives_in_provider_client_crate --offline`

Expected: FAIL because provider-client does not yet own chat request planning or payload construction.

---

### Task 2: Add Provider-Client Chat Plan DTOs And Builder

**Files:**
- Modify: `crates/novex-provider-client/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub enum ModelProviderChatTransport`
  - `pub struct ModelProviderChatMessage`
  - `pub struct ModelProviderChatFileContext`
  - `pub enum ModelProviderChatRequestKind`
  - `pub struct ModelProviderChatCompactionMetadata`
  - `pub struct ModelProviderChatRequestMetadata`
  - `pub struct ModelProviderChatPlanInput`
  - `pub struct ModelProviderChatPlan`
  - `pub fn build_model_provider_chat_plan(input: ModelProviderChatPlanInput) -> ModelProviderChatPlan`
  - `pub fn model_provider_chat_plan_streams_chat_completion(plan: &ModelProviderChatPlan) -> bool`

- [ ] **Step 1: Add provider-client planner tests**

Add unit tests in `crates/novex-provider-client/src/lib.rs`:

```rust
#[test]
fn chat_plan_builder_maps_regular_chat_completion_payload() {
    let input = ModelProviderChatPlanInput {
        provider: ModelProviderType::DeepSeek,
        model: Some("deepseek-v4-flash".to_owned()),
        base_url: "https://llm.internal/v1".to_owned(),
        endpoint: "https://llm.internal/v1/chat/completions".to_owned(),
        messages: vec![ModelProviderChatMessage {
            role: "user".to_owned(),
            content: "hello".to_owned(),
        }],
        file_contexts: Vec::new(),
        temperature: 0.2,
        max_tokens: 1024,
        response_format: Some(json!({"type": "json_object"})),
        request_metadata: None,
        should_stream_chat_completion: false,
    };

    let plan = build_model_provider_chat_plan(input);

    assert_eq!(plan.transport, ModelProviderChatTransport::ChatCompletions);
    assert_eq!(plan.endpoint, "https://llm.internal/v1/chat/completions");
    assert_eq!(plan.payload["model"], "deepseek-v4-flash");
    assert_eq!(plan.payload["messages"][0]["role"], "user");
    assert_eq!(plan.payload["messages"][0]["content"], "hello");
    assert_eq!(plan.payload["stream"], false);
    assert_eq!(plan.payload["response_format"]["type"], "json_object");
}

#[test]
fn chat_plan_builder_maps_responses_compaction_and_metadata() {
    let input = ModelProviderChatPlanInput {
        provider: ModelProviderType::OpenAiCompatible,
        model: Some("gpt-compatible".to_owned()),
        base_url: "https://llm.internal/v1/".to_owned(),
        endpoint: "https://llm.internal/v1/chat/completions".to_owned(),
        messages: vec![ModelProviderChatMessage {
            role: "user".to_owned(),
            content: "compact this context".to_owned(),
        }],
        file_contexts: Vec::new(),
        temperature: 0.2,
        max_tokens: 512,
        response_format: None,
        request_metadata: Some(test_provider_compaction_metadata("responses_compaction_v2")),
        should_stream_chat_completion: false,
    };

    let plan = build_model_provider_chat_plan(input);

    assert_eq!(plan.transport, ModelProviderChatTransport::ResponsesCompactionV2);
    assert_eq!(plan.endpoint, "https://llm.internal/v1/responses");
    assert_eq!(plan.payload["stream"], true);
    assert_eq!(plan.payload["background"], true);
    assert_eq!(plan.payload["store"], true);
    assert_eq!(plan.payload["metadata"]["request_kind"], "compaction");
    assert_eq!(plan.payload["metadata"]["tool_codes"], "rag.search");
    assert_eq!(
        plan.payload["input"].as_array().unwrap().last().unwrap()["type"],
        "compaction_trigger"
    );
}

#[test]
fn chat_plan_builder_maps_unary_compaction_endpoint() {
    let input = ModelProviderChatPlanInput {
        provider: ModelProviderType::OpenAiCompatible,
        model: Some("gpt-compatible".to_owned()),
        base_url: "https://llm.internal/v1".to_owned(),
        endpoint: "https://llm.internal/v1/chat/completions".to_owned(),
        messages: vec![ModelProviderChatMessage {
            role: "user".to_owned(),
            content: "compact this context".to_owned(),
        }],
        file_contexts: Vec::new(),
        temperature: 0.2,
        max_tokens: 512,
        response_format: None,
        request_metadata: Some(test_provider_compaction_metadata("responses_compaction_unary")),
        should_stream_chat_completion: false,
    };

    let plan = build_model_provider_chat_plan(input);

    assert_eq!(plan.transport, ModelProviderChatTransport::ResponsesCompactUnary);
    assert_eq!(plan.endpoint, "https://llm.internal/v1/responses/compact");
    assert!(plan.payload.get("stream").is_none());
    assert_eq!(plan.payload["tools"].as_array().unwrap().len(), 0);
    assert_eq!(plan.payload["parallel_tool_calls"], false);
}

#[test]
fn chat_plan_builder_maps_responses_code_agent_streaming() {
    let input = ModelProviderChatPlanInput {
        provider: ModelProviderType::OpenAiCompatible,
        model: Some("qwen-private".to_owned()),
        base_url: "https://llm.internal/v1".to_owned(),
        endpoint: "https://llm.internal/v1/responses".to_owned(),
        messages: vec![ModelProviderChatMessage {
            role: "user".to_owned(),
            content: "use tools if needed".to_owned(),
        }],
        file_contexts: Vec::new(),
        temperature: 0.2,
        max_tokens: 768,
        response_format: None,
        request_metadata: None,
        should_stream_chat_completion: true,
    };

    let plan = build_model_provider_chat_plan(input);

    assert_eq!(plan.transport, ModelProviderChatTransport::ResponsesCodeAgent);
    assert_eq!(plan.endpoint, "https://llm.internal/v1/responses");
    assert_eq!(plan.payload["stream"], true);
    assert!(plan.payload.get("input").is_some());
    assert!(plan.payload.get("messages").is_none());
    assert!(model_provider_chat_plan_streams_chat_completion(&plan));
}
```

- [ ] **Step 2: Add the minimal DTOs and pure builder**

Implement the DTOs and move the payload logic from backend into provider-client. Keep helper functions private: endpoint joining, file-context prompt formatting, Responses input item conversion, provider metadata gating, and metadata serialization.

- [ ] **Step 3: Verify provider-client planner tests**

Run: `cargo test -p novex-provider-client chat_plan --offline`

Expected: PASS.

---

### Task 3: Adapt Backend Route Planning To Provider-Client Builder

**Files:**
- Modify: `backend/src/application/ai/model_provider_transport.rs`
- Modify: `backend/src/application/ai/model_service.rs`

**Interfaces:**
- Consumes: provider-client chat plan DTOs and builder from Task 2.
- Produces: backend compatibility aliases and adapter input conversion:
  - `ModelProviderChatPlanInput`
  - `ModelProviderChatPlan`
  - `ModelProviderChatTransport`
  - `build_model_provider_chat_plan`
  - `model_provider_chat_plan_streams_chat_completion`

- [ ] **Step 1: Re-export provider-client planner APIs through the backend transport facade**

In `backend/src/application/ai/model_provider_transport.rs`, re-export the planner DTOs/functions alongside chat dispatch.

- [ ] **Step 2: Replace backend payload builders with provider-client plan conversion**

Keep `model_chat_provider_request(route, command)` as a backend adapter, but change it to build `ModelProviderChatPlanInput` from route/command and delegate to `build_model_provider_chat_plan(...)`. Remove backend-only payload helpers after tests are updated:

```rust
fn model_chat_provider_request(
    route: &ModelRuntimeRoute,
    command: &ModelChatCommand,
) -> ModelChatProviderRequest {
    build_model_provider_chat_plan(ModelProviderChatPlanInput {
        provider: route.provider(),
        model: route.model().map(str::to_owned),
        base_url: route.base_url().to_owned(),
        endpoint: route.endpoint().to_owned(),
        messages: command
            .messages
            .iter()
            .map(|message| ModelProviderChatMessage {
                role: message.role.clone(),
                content: message.content.clone(),
            })
            .collect(),
        file_contexts: command
            .file_contexts
            .iter()
            .map(|file| ModelProviderChatFileContext {
                name: file.name.clone(),
                content_type: file.content_type.clone(),
                content: file.content.clone(),
            })
            .collect(),
        temperature: command.temperature.unwrap_or(DEFAULT_MODEL_CHAT_TEMPERATURE),
        max_tokens: command.max_tokens.unwrap_or(DEFAULT_MODEL_CHAT_MAX_TOKENS),
        response_format: command.response_format.clone(),
        request_metadata: command
            .request_metadata
            .as_ref()
            .map(model_provider_chat_request_metadata_from_command),
        should_stream_chat_completion: model_chat_should_stream_chat_completion(command),
    })
}
```

- [ ] **Step 3: Update backend transport names and tests**

Alias backend-local names to provider-client names so existing route execution keeps its shape:

```rust
type ModelChatProviderRequest = ModelProviderChatPlan;
type ModelChatProviderTransport = ModelProviderChatTransport;
```

Update tests that call `model_chat_request_payload(...)` to call `model_chat_provider_request(...).payload`, and update source-contract tests to require provider-client planning ownership.

- [ ] **Step 4: Verify focused backend tests**

Run:

```bash
cargo test -p backend-rust provider_client_chat_request_plan_lives_in_provider_client_crate --offline
cargo test -p backend-rust model_chat_payload --offline
cargo test -p backend-rust provider_responses_transport --offline
cargo test -p backend-rust provider_compact_transport --offline
cargo test -p backend-rust provider_compact_unary --offline
cargo test -p backend-rust model_provider_stream_dispatch_route_path --offline
```

Expected: all commands pass.

---

### Task 4: Update Matrix, Verify, Commit, Merge, And Clean

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-18-agent-provider-client-chat-request-plan.md`

**Interfaces:**
- Consumes: provider-client chat plan builder and backend adapter.
- Produces: migration matrix slice update that provider-client owns provider-neutral chat/Responses request planning and payload construction, while backend owns route resolution and runtime context.

- [ ] **Step 1: Update migration matrix**

Change runtime-loop status to the next slice and update the runtime-loop notes and acceptance evidence to say provider-client owns chat plan DTOs, transport enum, endpoint selection, payload construction, streamability predicate, dispatch APIs, response text reading, and parser APIs.

- [ ] **Step 2: Run full verification**

Run:

```bash
cargo fmt -- --check
git diff --check
cargo test -p novex-provider-client chat_plan --offline
cargo test -p backend-rust provider_client_chat_request_plan_lives_in_provider_client_crate --offline
cargo test -p backend-rust model_chat_payload --offline
cargo test -p backend-rust provider_responses_transport --offline
cargo test -p backend-rust provider_compact_transport --offline
cargo test -p backend-rust provider_compact_unary --offline
cargo test -p backend-rust model_provider_stream_dispatch_route_path --offline
cargo test --workspace --offline
```

Expected: all commands pass.

- [ ] **Step 3: Commit implementation**

```bash
git add crates/novex-provider-client/src/lib.rs backend/src/application/ai/model_provider_transport.rs backend/src/application/ai/model_service.rs docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-18-agent-provider-client-chat-request-plan.md
git commit -m "feat: extract provider client chat request planning"
```

- [ ] **Step 4: Merge into main and verify main**

```bash
git -C /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex merge --ff-only feat/enterprise-agent-foundation
git -C /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex status --short --branch
```

Then run in `/Users/yusenlin/Avalon/freedom/github/zm-agent/Novex`:

```bash
cargo fmt -- --check
git diff --check
cargo test --workspace --offline
```

- [ ] **Step 5: Sync feature worktree and clean both workspaces**

```bash
git -C /Users/yusenlin/Avalon/freedom/github/zm-agent/Novex/.worktrees/enterprise-agent-foundation merge --ff-only main
cargo clean
```

Run `cargo clean` in both main and feature worktree after main verification.
