# Novex MCP Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize `crates/novex-mcp` from a 2,201-line `src/lib.rs` into focused MCP protocol modules while preserving the crate-root public API.

**Architecture:** Keep `src/lib.rs` as the crate facade and move existing behavior unchanged into modules for core types, tool-code normalization, JSON-RPC, Streamable HTTP, OAuth, stdio, client errors, and registration. Add integration-level structure tests that prove `lib.rs` is a facade and existing root imports still work for backend consumers.

**Tech Stack:** Rust 2021, Cargo workspace, `serde`, `serde_json`, `url`, `novex-ai-core`, `novex-tools`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No model routing behavior changes.
- No frontend changes.
- No new MCP runtime behavior.
- Preserve root-level exports such as `novex_mcp::McpOAuthAuthorizationPlan`, `novex_mcp::McpStreamableHttpRequestPlan`, `novex_mcp::McpStdioLaunchPlan`, and `novex_mcp::validate_mcp_registration_policy`.
- Keep cross-crate dependency direction as `novex-mcp -> novex-ai-core / novex-tools`.
- Run `cargo fmt --all -- --check`, `cargo test -p novex-mcp`, and `git diff --check` before considering this slice complete.

---

## File Structure

- Create: `crates/novex-mcp/tests/module_structure.rs`
  - Proves the new module files exist, `lib.rs` is a facade, and root-level public APIs keep working.
- Create: `crates/novex-mcp/src/types.rs`
  - Owns server/transport/auth enums, tool descriptors, discovered tool conversion, invocation DTOs.
- Create: `crates/novex-mcp/src/tool_code.rs`
  - Owns MCP tool-code normalization.
- Create: `crates/novex-mcp/src/json_rpc.rs`
  - Owns JSON-RPC request and notification builders.
- Create: `crates/novex-mcp/src/client_error.rs`
  - Owns provider-neutral MCP client error kinds and helper constructors.
- Create: `crates/novex-mcp/src/streamable_http.rs`
  - Owns Streamable HTTP request plans, response DTO, JSON/SSE parsing, JSON-RPC result mapping.
- Create: `crates/novex-mcp/src/oauth.rs`
  - Owns OAuth authorization planning, token exchange/refresh planning, token response/session material, and OAuth validation.
- Create: `crates/novex-mcp/src/stdio.rs`
  - Owns stdio env values, lifecycle policy, launch/tool-call plans, and stdio validation.
- Create: `crates/novex-mcp/src/registration.rs`
  - Owns registration policy, discovery plan, endpoint allow-list validation.
- Modify: `crates/novex-mcp/src/lib.rs`
  - Keep only module declarations, root re-exports, crate constants, and `module()`.

---

### Task 1: Add MCP Structure and Public-Facade Characterization Tests

**Files:**
- Create: `crates/novex-mcp/tests/module_structure.rs`

**Interfaces:**
- Consumes: existing crate-root public API from `novex_mcp`.
- Produces: failing structure tests that later tasks must satisfy.

- [ ] **Step 1: Write the failing structure and facade tests**

Create `crates/novex-mcp/tests/module_structure.rs` with:

```rust
use std::fs;
use std::path::Path;

use novex_mcp::{
    mcp_oauth_session_from_token_response, mcp_tool_code, parse_mcp_tool_call_response,
    validate_mcp_registration_policy, McpAuthScope, McpAuthType, McpDiscoveredTool,
    McpJsonRpcRequest, McpOAuthAuthorizationConfig, McpOAuthAuthorizationPlan,
    McpOAuthClientAuth, McpOAuthPkceMethod, McpOAuthTokenResponse, McpRegistrationPolicy,
    McpServerStatus, McpStdioEnvValue, McpStdioLaunchConfig, McpStdioLaunchPlan,
    McpStdioLifecyclePolicy, McpStreamableHttpRequestPlan, McpStreamableHttpResponse,
    McpToolInvocationRequest, McpTransportKind, MCP_PROTOCOL_VERSION,
};
use novex_tools::ToolRiskLevel;

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_mcp_modules() {
    let lib = crate_file("src/lib.rs");

    for module in [
        "client_error",
        "json_rpc",
        "oauth",
        "registration",
        "stdio",
        "streamable_http",
        "tool_code",
        "types",
    ] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub struct McpOAuthAuthorizationPlan",
        "pub struct McpStreamableHttpRequestPlan",
        "pub struct McpStdioLaunchPlan",
        "pub fn parse_mcp_tool_call_response",
        "pub fn validate_mcp_registration_policy",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn mcp_domain_modules_exist() {
    for module in [
        "src/client_error.rs",
        "src/json_rpc.rs",
        "src/oauth.rs",
        "src/registration.rs",
        "src/stdio.rs",
        "src/streamable_http.rs",
        "src/tool_code.rs",
        "src/types.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_core_tool_contracts() {
    assert_eq!(mcp_tool_code("Docs Server", "Search/File"), "mcp.docs_server.search_file");

    let tool = McpDiscoveredTool {
        server_code: "docs".to_owned(),
        tool_name: "search".to_owned(),
        description: "Search docs".to_owned(),
        input_schema: serde_json::json!({"type": "object"}),
        output_schema: None,
        risk_level: ToolRiskLevel::Low,
    };
    let definition = tool.to_tool_definition("ai:mcp:docs:search");

    assert_eq!(definition.code, "mcp.docs.search");
    assert_eq!(definition.permission_code.as_deref(), Some("ai:mcp:docs:search"));
}

#[test]
fn root_facade_preserves_json_rpc_and_streamable_http_contracts() {
    let request = McpToolInvocationRequest {
        server_code: "docs".to_owned(),
        tool_name: "search".to_owned(),
        arguments: serde_json::json!({"query": "codex"}),
    };
    let rpc = McpJsonRpcRequest::tools_call("call-1", &request).into_value();
    assert_eq!(rpc["jsonrpc"], "2.0");
    assert_eq!(rpc["method"], "tools/call");

    let plan = McpStreamableHttpRequestPlan::tools_call(
        "https://mcp.example.com/mcp",
        "call-1",
        &request,
        Some("env:DOCS_MCP_TOKEN"),
    );
    assert_eq!(
        plan.header_value("MCP-Protocol-Version").as_deref(),
        Some(MCP_PROTOCOL_VERSION)
    );

    let response = McpStreamableHttpResponse::new(
        200,
        "application/json",
        serde_json::json!({"jsonrpc":"2.0","result":{"content":[],"isError":false}}).to_string(),
    );
    let result = parse_mcp_tool_call_response("mcp.docs.search", &response).unwrap();
    assert_eq!(result.status, "succeeded");
}

#[test]
fn root_facade_preserves_oauth_stdio_and_registration_contracts() {
    let oauth = McpOAuthAuthorizationPlan::new(McpOAuthAuthorizationConfig {
        server_code: "docs".to_owned(),
        authorization_endpoint: "https://auth.example.com/oauth/authorize".to_owned(),
        token_endpoint: "https://auth.example.com/oauth/token".to_owned(),
        client_id: "novex-mcp-client".to_owned(),
        redirect_uri: "https://novex.example.com/mcp/oauth/callback".to_owned(),
        scopes: vec!["mcp:tools".to_owned()],
        state: "tenant-42-state".to_owned(),
        pkce_challenge: "s256-code-challenge".to_owned(),
        pkce_method: McpOAuthPkceMethod::S256,
        client_auth: McpOAuthClientAuth::None,
    })
    .unwrap();
    assert!(oauth.authorization_url.contains("code_challenge_method=S256"));

    let session = mcp_oauth_session_from_token_response(
        "docs",
        &McpOAuthTokenResponse {
            access_token: "access-token-value".to_owned(),
            token_type: "Bearer".to_owned(),
            expires_in_seconds: Some(3600),
            refresh_token: None,
            scope: Some("mcp:tools".to_owned()),
        },
        100,
        "env:DOCS_MCP_ACCESS_TOKEN",
        None,
    )
    .unwrap();
    assert_eq!(session.expires_at_epoch_seconds, Some(3700));

    let mut env = std::collections::BTreeMap::new();
    env.insert(
        "DOCS_MCP_TOKEN".to_owned(),
        McpStdioEnvValue::SecretRef("env:DOCS_MCP_TOKEN".to_owned()),
    );
    let launch = McpStdioLaunchPlan::new(McpStdioLaunchConfig {
        command: "node".to_owned(),
        args: vec!["server.js".to_owned()],
        env,
        working_dir: None,
        lifecycle_policy: McpStdioLifecyclePolicy::default(),
    })
    .unwrap();
    assert_eq!(launch.command, "node");

    let discovery = validate_mcp_registration_policy(&McpRegistrationPolicy {
        server_code: "docs".to_owned(),
        endpoint_url: Some("https://mcp.example.com/sse".to_owned()),
        transport_kind: McpTransportKind::StreamableHttp,
        auth_scope: McpAuthScope::Tenant,
        auth_type: McpAuthType::BearerEnv,
        secret_ref: Some("env:DOCS_MCP_TOKEN".to_owned()),
        network_allowlist: vec!["mcp.example.com".to_owned()],
        tool_allowlist: vec!["search".to_owned()],
    })
    .unwrap();
    assert_eq!(discovery.status, McpServerStatus::Discovering);
}
```

- [ ] **Step 2: Run the new test and verify it fails for structure**

Run:

```bash
cargo test -p novex-mcp --test module_structure
```

Expected: FAIL because the module files do not exist yet and `src/lib.rs` still contains moved items.

---

### Task 2: Split MCP Implementation Modules

**Files:**
- Create: `crates/novex-mcp/src/types.rs`
- Create: `crates/novex-mcp/src/tool_code.rs`
- Create: `crates/novex-mcp/src/json_rpc.rs`
- Create: `crates/novex-mcp/src/client_error.rs`
- Create: `crates/novex-mcp/src/streamable_http.rs`
- Create: `crates/novex-mcp/src/oauth.rs`
- Create: `crates/novex-mcp/src/stdio.rs`
- Create: `crates/novex-mcp/src/registration.rs`
- Modify: `crates/novex-mcp/src/lib.rs`

**Interfaces:**
- Consumes: existing definitions in `src/lib.rs`.
- Produces: the same root public API through facade re-exports.

- [ ] **Step 1: Move definitions to their modules unchanged**

Move existing items using this ownership map:

```text
types.rs:
  McpServerStatus, McpTransportKind, McpAuthScope, McpAuthType, impl McpAuthType,
  McpToolDescriptor, McpDiscoveredTool, impl McpDiscoveredTool,
  McpToolInvocationRequest, McpToolInvocationResult

tool_code.rs:
  mcp_tool_code, normalize_mcp_code_segment

json_rpc.rs:
  McpJsonRpcRequest, impl McpJsonRpcRequest,
  McpJsonRpcNotification, impl McpJsonRpcNotification

client_error.rs:
  McpClientErrorKind, McpClientError, impl McpClientError

streamable_http.rs:
  McpStreamableHttpRequestPlan, impl McpStreamableHttpRequestPlan,
  McpStreamableHttpResponse, impl McpStreamableHttpResponse,
  parse_mcp_tool_call_response,
  parse_mcp_json_payload, parse_mcp_sse_payload, mcp_tool_result_from_json_rpc

oauth.rs:
  McpOAuthPkceMethod, McpOAuthClientAuth, McpOAuthAuthorizationConfig,
  McpOAuthAuthorizationPlan, McpOAuthAuthorizationError,
  OAuth validation helpers, McpOAuthGrantType, token exchange/refresh configs,
  McpOAuthTokenExchangePlan, McpOAuthTokenResponse, McpOAuthSessionMaterial,
  mcp_oauth_session_from_token_response, McpOAuthSessionError,
  OAuth secret-ref helpers, oauth_client_auth_evidence

stdio.rs:
  McpStdioEnvValue, McpStdioLifecyclePolicy, McpStdioLifecyclePhase,
  MCP_STDIO_LIFECYCLE_PHASES, McpStdioLaunchConfig, McpStdioLaunchPlan,
  McpStdioToolCallPlan, McpStdioLaunchError, stdio validation helpers

registration.rs:
  McpRegistrationPolicy, McpDiscoveryPlan, McpRegistrationError,
  validate_mcp_registration_policy, ensure_endpoint_allowed
```

- [ ] **Step 2: Replace `src/lib.rs` with facade declarations**

`src/lib.rs` should declare modules, re-export all public items listed above, keep crate constants, and keep `module()`:

```rust
mod client_error;
mod json_rpc;
mod oauth;
mod registration;
mod stdio;
mod streamable_http;
mod tool_code;
mod types;

use novex_ai_core::FoundationModule;

pub use client_error::{McpClientError, McpClientErrorKind};
pub use json_rpc::{McpJsonRpcNotification, McpJsonRpcRequest};
pub use oauth::{
    mcp_oauth_session_from_token_response, McpOAuthAuthorizationConfig,
    McpOAuthAuthorizationError, McpOAuthAuthorizationPlan, McpOAuthClientAuth,
    McpOAuthGrantType, McpOAuthPkceMethod, McpOAuthSessionError, McpOAuthSessionMaterial,
    McpOAuthTokenExchangeConfig, McpOAuthTokenExchangePlan, McpOAuthTokenRefreshConfig,
    McpOAuthTokenResponse,
};
pub use registration::{
    validate_mcp_registration_policy, McpDiscoveryPlan, McpRegistrationError,
    McpRegistrationPolicy,
};
pub use stdio::{
    McpStdioEnvValue, McpStdioLaunchConfig, McpStdioLaunchError, McpStdioLaunchPlan,
    McpStdioLifecyclePhase, McpStdioLifecyclePolicy, McpStdioToolCallPlan,
};
pub use streamable_http::{
    parse_mcp_tool_call_response, McpStreamableHttpRequestPlan, McpStreamableHttpResponse,
};
pub use tool_code::mcp_tool_code;
pub use types::{
    McpAuthScope, McpAuthType, McpDiscoveredTool, McpServerStatus, McpToolDescriptor,
    McpToolInvocationRequest, McpToolInvocationResult, McpTransportKind,
};

pub const CRATE_ID: &str = "novex-mcp";
pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
pub const MCP_STDIO_MIN_TIMEOUT_MS: u64 = 100;
pub const MCP_STDIO_MAX_TIMEOUT_MS: u64 = 60_000;
pub const MCP_STDIO_DEFAULT_STARTUP_TIMEOUT_MS: u64 = 10_000;
pub const MCP_STDIO_DEFAULT_SHUTDOWN_TIMEOUT_MS: u64 = 5_000;

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "MCP Gateway",
        "ai-foundation",
        "MCP server registration, tool discovery, tenant authorization, secret, and audit boundaries.",
    )
}
```

- [ ] **Step 3: Run full MCP tests**

Run:

```bash
cargo test -p novex-mcp
```

Expected: PASS, including `tests/module_structure.rs`.

---

### Task 3: Move Existing Tests Out of lib.rs

**Files:**
- Create: `crates/novex-mcp/tests/module_contract.rs`
- Create: `crates/novex-mcp/tests/registration.rs`
- Create: `crates/novex-mcp/tests/types.rs`
- Create: `crates/novex-mcp/tests/streamable_http.rs`
- Create: `crates/novex-mcp/tests/oauth.rs`
- Create: `crates/novex-mcp/tests/stdio.rs`
- Modify: `crates/novex-mcp/src/lib.rs`

**Interfaces:**
- Consumes: public crate-root re-exports.
- Produces: domain-focused integration tests equivalent to the original 29 tests.

- [ ] **Step 1: Move tests by domain**

Move the original `#[cfg(test)] mod tests` contents as follows:

```text
module_describes_mcp_boundary -> tests/module_contract.rs
registration_policy_* -> tests/registration.rs
mcp_discovered_tool_converts_to_tenant_tool_definition -> tests/types.rs
mcp_streamable_http_* -> tests/streamable_http.rs
mcp_oauth_* -> tests/oauth.rs
mcp_stdio_* -> tests/stdio.rs
```

Use root imports such as `use novex_mcp::*;` in integration tests, plus `use novex_ai_core::FoundationStatus;`, `use novex_tools::ToolRiskLevel;`, `use std::collections::BTreeMap;`, or `use url::Url;` only where each test file needs them.

- [ ] **Step 2: Confirm `lib.rs` has no test module**

Run:

```bash
rg -n '#\\[cfg\\(test\\)\\]|mod tests' crates/novex-mcp/src/lib.rs
```

Expected: no matches.

- [ ] **Step 3: Run MCP tests**

Run:

```bash
cargo test -p novex-mcp
```

Expected: PASS.

---

### Task 4: Update MCP Source-Location Docs

**Files:**
- Modify docs reported by `rg "crates/novex-mcp/src/lib.rs|novex-mcp/src/lib.rs" docs/plans docs/superpowers/specs`.

**Interfaces:**
- Consumes: new MCP module paths.
- Produces: docs that point future MCP work at focused modules instead of `src/lib.rs`.

- [ ] **Step 1: Find stale MCP `lib.rs` instructions**

Run:

```bash
rg -n 'crates/novex-mcp/src/lib.rs|novex-mcp/src/lib.rs' docs/plans docs/superpowers/specs
```

Expected: matches in older plans.

- [ ] **Step 2: Update contributor-facing references**

Replace future-work instructions according to ownership:

```text
MCP tool descriptors and invocation DTOs -> crates/novex-mcp/src/types.rs
MCP tool code normalization -> crates/novex-mcp/src/tool_code.rs
JSON-RPC request/notification builders -> crates/novex-mcp/src/json_rpc.rs
Streamable HTTP request/response parsing -> crates/novex-mcp/src/streamable_http.rs
OAuth authorization/session/token exchange -> crates/novex-mcp/src/oauth.rs
Stdio lifecycle and launch planning -> crates/novex-mcp/src/stdio.rs
Registration/discovery policy -> crates/novex-mcp/src/registration.rs
Client error vocabulary -> crates/novex-mcp/src/client_error.rs
crate facade only -> crates/novex-mcp/src/lib.rs
```

Do not rewrite historical skeleton creation records.

---

### Task 5: Final Verification and Commit

**Files:**
- Verify all files changed by Tasks 1-4.

**Interfaces:**
- Consumes: normalized MCP modules.
- Produces: committed, verified `novex-mcp` module architecture slice.

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
cargo test -p novex-mcp
```

Expected: PASS.

- [ ] **Step 3: Run backend root-import smoke**

Run:

```bash
cargo test -p backend-rust application::ai::foundation_service::tests::summary_lists_required_foundation_crates
```

Expected: PASS.

- [ ] **Step 4: Commit the completed MCP split**

Run:

```bash
git add crates/novex-mcp/src crates/novex-mcp/tests docs/plans docs/superpowers/specs docs/superpowers/plans/2026-06-19-novex-mcp-module-architecture.md
git commit -m "refactor: split novex mcp into focused modules"
```

Expected: commit succeeds.
