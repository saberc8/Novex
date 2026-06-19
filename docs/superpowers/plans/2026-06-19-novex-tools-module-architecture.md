# Novex Tools Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize `crates/novex-tools` from a 1,789-line `src/lib.rs` into focused tool-governance modules while preserving the crate-root public API.

**Architecture:** Keep `src/lib.rs` as the crate facade and move existing behavior unchanged into modules for tool types, execution policy, concurrency planning, executor registry, routing, built-in definitions, input adapters, and media helpers. Add integration-level structure tests that prove `lib.rs` is a facade and existing root imports still work for backend, provider-client, MCP, and agent consumers.

**Tech Stack:** Rust 2021, Cargo workspace, `serde`, `serde_json`, `novex-ai-core`, `novex-connectors`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No model routing behavior changes.
- No frontend changes.
- No new tool behavior.
- Preserve root-level exports such as `novex_tools::ToolDefinition`, `novex_tools::ToolRouter`, `novex_tools::ToolExecutorRegistry`, `novex_tools::MediaImageGenerationRequest`, and `novex_tools::parse_media_image_generation_response`.
- Keep cross-crate dependency direction as `novex-tools -> novex-ai-core / novex-model / novex-connectors`.
- Run `cargo fmt --all -- --check`, `cargo test -p novex-tools`, and `git diff --check` before considering this slice complete.

---

## File Structure

- Create: `crates/novex-tools/tests/module_structure.rs`
  - Proves the new module files exist, `lib.rs` is a facade, and root-level public APIs keep working.
- Create: `crates/novex-tools/src/types.rs`
  - Owns tool kind, risk, approval policy, execution envelope, tool definitions, model-visible specs.
- Create: `crates/novex-tools/src/policy.rs`
  - Owns risk/policy code helpers and tool execution policy evaluation.
- Create: `crates/novex-tools/src/concurrency.rs`
  - Owns execution locks, concurrency policy, batch execution mode, batch planning.
- Create: `crates/novex-tools/src/executor.rs`
  - Owns executor kinds, bindings, dispatch plans, registry, and registry errors.
- Create: `crates/novex-tools/src/router.rs`
  - Owns route errors, routed calls, tool router, and `ToolDefinition::to_model_tool_spec`.
- Create: `crates/novex-tools/src/definitions.rs`
  - Owns built-in Agent model-loop and customer-service tool definitions plus executor bindings.
- Create: `crates/novex-tools/src/adapters.rs`
  - Owns Feishu, media, and GitHub input adapters plus private JSON/GitHub parsing helpers.
- Create: `crates/novex-tools/src/media.rs`
  - Owns media image request/result DTOs, provider payload shaping, and media response parsing.
- Modify: `crates/novex-tools/src/lib.rs`
  - Keep only module declarations, root re-exports, `CRATE_ID`, and `module()`.

---

### Task 1: Add Tools Structure and Public-Facade Characterization Tests

**Files:**
- Create: `crates/novex-tools/tests/module_structure.rs`

**Interfaces:**
- Consumes: existing crate-root public API from `novex_tools`.
- Produces: failing structure tests that later tasks must satisfy.

- [ ] **Step 1: Write the failing structure and facade tests**

Create `crates/novex-tools/tests/module_structure.rs` with:

```rust
use std::fs;
use std::path::Path;

use novex_tools::{
    agent_model_loop_tool_definitions, agent_model_loop_tool_executor_bindings,
    approval_policy_code, customer_service_tool_definitions, evaluate_tool_execution_policy,
    feishu_message_text_from_tool_input, github_read_request_from_tool_input,
    github_search_request_from_tool_input, media_image_request_from_tool_input,
    parse_media_image_generation_response, tool_risk_code, AgentToolExecution, ApprovalPolicy,
    MediaImageGenerationRequest, ToolBatchExecutionMode, ToolBatchPlan, ToolConcurrencyPolicy,
    ToolDefinition, ToolExecutionLock, ToolExecutionPolicyInput, ToolExecutorBinding,
    ToolExecutorDispatchPlan, ToolExecutorKind, ToolExecutorRegistry, ToolKind, ToolRiskLevel,
    ToolRouteErrorKind, ToolRouter,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_tool_modules() {
    let lib = crate_file("src/lib.rs");

    for module in [
        "adapters",
        "concurrency",
        "definitions",
        "executor",
        "media",
        "policy",
        "router",
        "types",
    ] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub struct ToolDefinition",
        "pub struct ToolRouter",
        "pub struct ToolExecutorRegistry",
        "pub fn agent_model_loop_tool_definitions",
        "pub fn evaluate_tool_execution_policy",
        "pub fn parse_media_image_generation_response",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn tool_domain_modules_exist() {
    for module in [
        "src/adapters.rs",
        "src/concurrency.rs",
        "src/definitions.rs",
        "src/executor.rs",
        "src/media.rs",
        "src/policy.rs",
        "src/router.rs",
        "src/types.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_core_policy_router_and_executor_contracts() {
    assert_eq!(tool_risk_code(ToolRiskLevel::High), "high");
    assert_eq!(approval_policy_code(ApprovalPolicy::Always), "always");
    assert_eq!(ToolKind::Mcp as u8, ToolKind::Mcp as u8);

    let decision = evaluate_tool_execution_policy(ToolExecutionPolicyInput {
        tool_code: "ticket.create".to_owned(),
        risk_level: ToolRiskLevel::High,
        approval_policy: ApprovalPolicy::OnRisk,
        permission_code: Some("ai:customer-service:ticket".to_owned()),
        auto_approved: true,
    });
    assert!(decision.requires_approval);
    assert_eq!(decision.pause_reason.as_deref(), Some("approval"));

    let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions()).unwrap();
    let call = router
        .route_tool_call("call-1", "rag.search", serde_json::json!({"query": "policy"}))
        .unwrap();
    assert_eq!(call.tool.code, "rag.search");
    assert_eq!(
        router
            .route_tool_call("call-2", "missing.tool", serde_json::json!({}))
            .unwrap_err()
            .kind,
        ToolRouteErrorKind::UnknownTool
    );

    let registry = ToolExecutorRegistry::from_bindings(agent_model_loop_tool_executor_bindings())
        .expect("agent executor registry should build");
    let web = registry.executor_for("web.search").unwrap();
    assert_eq!(web.kind, ToolExecutorKind::Builtin);

    let dispatch = ToolExecutorDispatchPlan::from_binding(&ToolExecutorBinding::new(
        "media.image.generate",
        "model.media.image.generate",
        ToolExecutorKind::Model,
    ));
    assert!(dispatch.requires_model_runtime);
}

#[test]
fn root_facade_preserves_concurrency_and_definition_contracts() {
    let read_only = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
        .unwrap()
        .route_tool_call("call-1", "github.repo.read", serde_json::json!({"repository":"org/repo","path":"README.md"}))
        .unwrap();
    let rag = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
        .unwrap()
        .route_tool_call("call-2", "rag.search", serde_json::json!({"query":"policy"}))
        .unwrap();
    let plan = ToolBatchPlan::from_routed_calls(vec![read_only, rag]);

    assert_eq!(plan.mode, ToolBatchExecutionMode::Parallel);
    assert_eq!(ToolConcurrencyPolicy::shared().lock, ToolExecutionLock::Shared);
    assert!(customer_service_tool_definitions()
        .iter()
        .any(|tool| tool.code == "ticket.create" && tool.risk_level == ToolRiskLevel::High));
}

#[test]
fn root_facade_preserves_adapter_media_and_execution_contracts() {
    let feishu = feishu_message_text_from_tool_input(&serde_json::json!({
        "message": "Complete training today",
        "input": "ignored"
    }));
    assert_eq!(feishu, "Complete training today");

    let media_request = media_image_request_from_tool_input(&serde_json::json!({
        "prompt": "Create poster",
        "size": "1024x1024",
        "count": 2
    }));
    assert_eq!(media_request.prompt, "Create poster");
    assert_eq!(media_request.count, 2);

    let provider_payload = MediaImageGenerationRequest::new("Create poster")
        .with_size("1024x1024")
        .with_count(2)
        .to_provider_payload();
    assert_eq!(provider_payload["n"], 2);

    let media_result = parse_media_image_generation_response(&serde_json::json!({
        "id": "img-1",
        "data": [{"url": "https://cdn.example.com/img.png"}]
    }))
    .unwrap();
    assert_eq!(media_result.provider_asset_id.as_deref(), Some("img-1"));

    let github_search = github_search_request_from_tool_input(&serde_json::json!({
        "input": "search GitHub repo acme/app for parser worker under src"
    }))
    .unwrap();
    assert_eq!(github_search.repository, "acme/app");
    assert_eq!(github_search.query, "parser worker");

    let github_read = github_read_request_from_tool_input(&serde_json::json!({
        "input": "read GitHub file acme/app src/lib.rs ref main"
    }))
    .unwrap();
    assert_eq!(github_read.path, "src/lib.rs");

    let execution = AgentToolExecution::succeeded(
        serde_json::json!({"status": "succeeded"}),
        true,
        "ok".to_owned(),
    );
    assert!(execution.succeeded_status());
}
```

- [ ] **Step 2: Run the new test and verify it fails for structure**

Run:

```bash
cargo test -p novex-tools --test module_structure
```

Expected: FAIL because the module files do not exist yet and `src/lib.rs` still contains moved items.

---

### Task 2: Split Tools Implementation Modules

**Files:**
- Create: `crates/novex-tools/src/types.rs`
- Create: `crates/novex-tools/src/policy.rs`
- Create: `crates/novex-tools/src/concurrency.rs`
- Create: `crates/novex-tools/src/executor.rs`
- Create: `crates/novex-tools/src/router.rs`
- Create: `crates/novex-tools/src/definitions.rs`
- Create: `crates/novex-tools/src/adapters.rs`
- Create: `crates/novex-tools/src/media.rs`
- Modify: `crates/novex-tools/src/lib.rs`

**Interfaces:**
- Consumes: existing definitions in `src/lib.rs`.
- Produces: the same root public API through facade re-exports.

- [ ] **Step 1: Move definitions to their modules unchanged**

Move existing items using this ownership map:

```text
types.rs:
  ToolKind, ToolRiskLevel, ApprovalPolicy, ToolExecutionPolicyInput,
  ToolExecutionPolicyDecision, AgentToolExecution, ToolDefinition, ModelToolSpec

policy.rs:
  tool_risk_code, approval_policy_code, evaluate_tool_execution_policy

concurrency.rs:
  ToolExecutionLock, ToolConcurrencyPolicy, impl ToolConcurrencyPolicy,
  impl Default for ToolConcurrencyPolicy, ToolBatchExecutionMode,
  ToolBatchPlan, impl ToolBatchPlan

executor.rs:
  ToolExecutorKind, ToolExecutorBinding, impl ToolExecutorBinding,
  ToolExecutorDispatchPlan, impl ToolExecutorDispatchPlan,
  ToolExecutorRegistryErrorKind, ToolExecutorRegistryError,
  ToolExecutorRegistry, impl ToolExecutorRegistry

router.rs:
  ToolRouteErrorKind, ToolRouteError, RoutedToolCall, ToolRouter,
  impl ToolRouter, impl ToolDefinition

definitions.rs:
  agent_model_loop_tool_definitions, agent_model_loop_tool_executor_bindings,
  customer_service_tool_definitions

adapters.rs:
  feishu_message_text_from_tool_input, media_image_request_from_tool_input,
  github_search_request_from_tool_input, github_read_request_from_tool_input,
  GitHub and JSON private helper functions

media.rs:
  MediaImageGenerationRequest, impl MediaImageGenerationRequest,
  MediaImageGenerationResult, parse_media_image_generation_response,
  media_image_url, media_provider_asset_id
```

- [ ] **Step 2: Replace `src/lib.rs` with facade declarations**

`src/lib.rs` should declare modules, re-export all public items listed above, keep `CRATE_ID`, and keep `module()`:

```rust
mod adapters;
mod concurrency;
mod definitions;
mod executor;
mod media;
mod policy;
mod router;
mod types;

use novex_ai_core::FoundationModule;

pub use adapters::{
    feishu_message_text_from_tool_input, github_read_request_from_tool_input,
    github_search_request_from_tool_input, media_image_request_from_tool_input,
};
pub use concurrency::{
    ToolBatchExecutionMode, ToolBatchPlan, ToolConcurrencyPolicy, ToolExecutionLock,
};
pub use definitions::{
    agent_model_loop_tool_definitions, agent_model_loop_tool_executor_bindings,
    customer_service_tool_definitions,
};
pub use executor::{
    ToolExecutorBinding, ToolExecutorDispatchPlan, ToolExecutorKind, ToolExecutorRegistry,
    ToolExecutorRegistryError, ToolExecutorRegistryErrorKind,
};
pub use media::{
    parse_media_image_generation_response, MediaImageGenerationRequest,
    MediaImageGenerationResult,
};
pub use policy::{approval_policy_code, evaluate_tool_execution_policy, tool_risk_code};
pub use router::{RoutedToolCall, ToolRouteError, ToolRouteErrorKind, ToolRouter};
pub use types::{
    AgentToolExecution, ApprovalPolicy, ModelToolSpec, ToolDefinition,
    ToolExecutionPolicyDecision, ToolExecutionPolicyInput, ToolKind, ToolRiskLevel,
};

pub const CRATE_ID: &str = "novex-tools";

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Tool Registry",
        "ai-foundation",
        "Tool schema, risk, permissions, approval, executor, audit, and replay boundaries.",
    )
}
```

- [ ] **Step 3: Run full Tools tests**

Run:

```bash
cargo test -p novex-tools
```

Expected: PASS, including `tests/module_structure.rs`.

---

### Task 3: Move Existing Tests Out of lib.rs

**Files:**
- Create: `crates/novex-tools/tests/module_contract.rs`
- Create: `crates/novex-tools/tests/media.rs`
- Create: `crates/novex-tools/tests/adapters.rs`
- Create: `crates/novex-tools/tests/types.rs`
- Create: `crates/novex-tools/tests/policy.rs`
- Create: `crates/novex-tools/tests/definitions.rs`
- Create: `crates/novex-tools/tests/router.rs`
- Create: `crates/novex-tools/tests/executor.rs`
- Create: `crates/novex-tools/tests/concurrency.rs`
- Modify: `crates/novex-tools/src/lib.rs`

**Interfaces:**
- Consumes: public crate-root re-exports.
- Produces: domain-focused integration tests equivalent to the original 24 tests.

- [ ] **Step 1: Move tests by domain**

Move the original `#[cfg(test)] mod tests` contents as follows:

```text
module_describes_tool_boundary -> tests/module_contract.rs
media_image_generation_request_builds_provider_payload -> tests/media.rs
parse_media_image_generation_response_extracts_common_url_shapes -> tests/media.rs
agent_tool_input_* -> tests/adapters.rs
agent_tool_execution_envelope_builds_success_failure_and_cancelled_statuses -> tests/types.rs
tool_execution_policy_evaluates_risk_permission_and_auto_approval -> tests/policy.rs
tool_definition_converts_to_model_visible_spec -> tests/router.rs
customer_service_tools_have_risk_and_schema_contracts -> tests/definitions.rs
agent_model_loop_tool_definitions_cover_builtin_agent_tools -> tests/definitions.rs
web_search_executor_binding_is_builtin -> tests/executor.rs
tool_executor_* -> tests/executor.rs
agent_model_loop_executor_bindings_cover_agent_model_loop_tools -> tests/executor.rs
tool_router_* -> tests/router.rs
tool_batch_plan_* and tool_router_reports_parallel_policy_for_read_only_tools -> tests/concurrency.rs
test_tool_definition helper -> tests/router.rs and tests/concurrency.rs where needed
```

Use root imports such as `use novex_tools::*;` in integration tests, plus `use novex_ai_core::FoundationStatus;` only where needed.

- [ ] **Step 2: Confirm `lib.rs` has no test module**

Run:

```bash
rg -n '#\\[cfg\\(test\\)\\]|mod tests' crates/novex-tools/src/lib.rs
```

Expected: no matches.

- [ ] **Step 3: Run Tools tests**

Run:

```bash
cargo test -p novex-tools
```

Expected: PASS.

---

### Task 4: Update Tools Source-Location Docs

**Files:**
- Modify docs reported by `rg "crates/novex-tools/src/lib.rs|novex-tools/src/lib.rs" docs/plans docs/superpowers`.

**Interfaces:**
- Consumes: new Tools module paths.
- Produces: docs that point future Tools work at focused modules instead of `src/lib.rs`.

- [ ] **Step 1: Find stale Tools `lib.rs` instructions**

Run:

```bash
rg -n 'crates/novex-tools/src/lib.rs|novex-tools/src/lib.rs' docs/plans docs/superpowers
```

Expected: matches in older plans.

- [ ] **Step 2: Update contributor-facing references**

Replace future-work instructions according to ownership:

```text
Tool types and execution envelope -> crates/novex-tools/src/types.rs
Risk/approval policy -> crates/novex-tools/src/policy.rs
Concurrency and batch planning -> crates/novex-tools/src/concurrency.rs
Executor registry and dispatch planning -> crates/novex-tools/src/executor.rs
Tool router and model-visible spec -> crates/novex-tools/src/router.rs
Built-in tool definitions and bindings -> crates/novex-tools/src/definitions.rs
Tool input adapters -> crates/novex-tools/src/adapters.rs
Media request/result parsing -> crates/novex-tools/src/media.rs
crate facade only -> crates/novex-tools/src/lib.rs
```

Do not rewrite historical skeleton creation records.

---

### Task 5: Final Verification and Commit

**Files:**
- Verify all files changed by Tasks 1-4.

**Interfaces:**
- Consumes: normalized Tools modules.
- Produces: committed, verified `novex-tools` module architecture slice.

- [ ] **Step 1: Format and check diff hygiene**

Run:

```bash
cargo fmt --all
cargo fmt --all -- --check
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 2: Run focused crate tests**

Run:

```bash
cargo test -p novex-tools
```

Expected: PASS.

- [ ] **Step 3: Run dependent crate smoke tests**

Run:

```bash
cargo test -p novex-mcp
cargo test -p novex-provider-client
cargo test -p backend application::ai::foundation_service::tests::summary_lists_required_foundation_crates
```

Expected: all commands pass.

- [ ] **Step 4: Commit the completed Tools split**

Run:

```bash
git add crates/novex-tools/src crates/novex-tools/tests docs/plans docs/superpowers docs/superpowers/plans/2026-06-19-novex-tools-module-architecture.md
git commit -m "refactor: split novex tools into focused modules"
```

Expected: commit succeeds.
