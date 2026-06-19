# Agent MCP Stdio Process Supervisor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run live MCP `stdio` tools through a backend-owned subprocess supervisor so local MCP servers can participate in the same agent tool execution path as Streamable HTTP MCP servers.

**Architecture:** `crates/novex-mcp` remains pure protocol/contract code and owns JSON-RPC request planning plus sanitized stdio launch evidence. `backend/src/application/ai/mcp_stdio_process.rs` owns process spawn, env secret resolution, newline JSON-RPC exchange, timeout handling, child shutdown/kill, and safe response evidence. `backend/src/application/ai/agent_tool_executor.rs` routes live MCP tools by `transport_kind`, preserving Streamable HTTP behavior while enabling `stdio`.

**Tech Stack:** Rust, Tokio process I/O, serde_json, existing `novex-mcp` request/result contracts, existing env secret resolver, existing `AgentToolExecution` payload shape.

## Global Constraints

- Keep `novex-mcp` pure: no subprocess, env, database, HTTP, or backend imports.
- Do not expose resolved secret values, literal stdio env values, bearer tokens, stdin payload secrets, or child stderr bodies in persisted execution payloads.
- Gate live stdio execution with the existing `metadata.liveExecutionEnabled == true` flag.
- Treat this as a one-shot supervisor slice: spawn, initialize, `notifications/initialized`, `tools/call`, shutdown/kill.
- Use TDD: add RED tests before production code.
- After completion, merge into `main`, run `cargo clean`, and remove the temporary worktree/branch.

---

## Task 1: Protocol Request Evidence

**Files:**
- Modify: `crates/novex-mcp/src/json_rpc.rs`
- Modify: `crates/novex-mcp/src/stdio.rs`

**Interfaces:**
- Produces: `McpJsonRpcRequest::initialize(id)`
- Produces: `McpJsonRpcNotification::initialized()`
- Produces: `McpStdioToolCallPlan::new(launch, request_id, request)`
- Produces: `McpStdioToolCallPlan::sanitized_evidence()`

- [ ] **Step 1: Write failing tests**

Add tests beside existing `mcp_stdio_*` tests:

```rust
#[test]
fn mcp_stdio_tool_call_plan_builds_initialize_and_call_messages() {
    let launch = McpStdioLaunchPlan::new(McpStdioLaunchConfig {
        command: "node".to_owned(),
        args: vec!["server.js".to_owned()],
        env: BTreeMap::new(),
        working_dir: None,
        lifecycle_policy: McpStdioLifecyclePolicy::new(2_000, 1_000).unwrap(),
    })
    .unwrap();
    let request = McpToolInvocationRequest {
        server_code: "docs".to_owned(),
        tool_name: "search".to_owned(),
        arguments: serde_json::json!({"query": "codex"}),
    };

    let plan = McpStdioToolCallPlan::new(launch, "tool-call-1", &request);

    assert_eq!(plan.initialize["method"], "initialize");
    assert_eq!(plan.initialized["method"], "notifications/initialized");
    assert_eq!(plan.tools_call["method"], "tools/call");
    assert_eq!(plan.tools_call["params"]["name"], "search");
    assert_eq!(plan.tools_call["params"]["arguments"]["query"], "codex");
}
```

Add a second test:

```rust
#[test]
fn mcp_stdio_tool_call_plan_sanitized_evidence_hides_env_literals() {
    let mut env = BTreeMap::new();
    env.insert(
        "MCP_TOKEN".to_owned(),
        McpStdioEnvValue::Literal("plain-secret".to_owned()),
    );
    let launch = McpStdioLaunchPlan::new(McpStdioLaunchConfig {
        command: "node".to_owned(),
        args: vec!["server.js".to_owned()],
        env,
        working_dir: None,
        lifecycle_policy: McpStdioLifecyclePolicy::new(2_000, 1_000).unwrap(),
    })
    .unwrap();
    let request = McpToolInvocationRequest {
        server_code: "docs".to_owned(),
        tool_name: "search".to_owned(),
        arguments: serde_json::json!({"query": "codex"}),
    };

    let evidence = McpStdioToolCallPlan::new(launch, "tool-call-1", &request)
        .sanitized_evidence();

    assert_eq!(evidence["transportKind"], "stdio");
    assert_eq!(evidence["request"]["method"], "tools/call");
    assert_eq!(evidence["request"]["params"]["arguments"]["query"], "codex");
    assert!(!evidence.to_string().contains("plain-secret"));
}
```

- [ ] **Step 2: Run RED**

Run: `cargo test -p novex-mcp mcp_stdio_tool_call_plan --offline`

Expected: FAIL because `McpStdioToolCallPlan` and notification planning do not exist.

- [ ] **Step 3: Implement minimal protocol plan**

Add `McpJsonRpcNotification`, `McpJsonRpcRequest::initialize`, and `McpStdioToolCallPlan` using existing JSON-RPC/launch plan types.

- [ ] **Step 4: Run GREEN**

Run: `cargo test -p novex-mcp mcp_stdio --offline`

Expected: PASS.

## Task 2: Backend Stdio Supervisor

**Files:**
- Create: `backend/src/application/ai/mcp_stdio_process.rs`
- Modify: `backend/src/application/ai/mod.rs`

**Interfaces:**
- Produces: `execute_mcp_stdio_tool_with_env(plan, tool_code, env_get) -> Result<McpToolInvocationResult, McpStdioProcessError>`
- Produces: `McpStdioProcessError { phase, message, evidence }`

- [ ] **Step 1: Write failing supervisor tests**

Create tests in `mcp_stdio_process.rs`:

```rust
#[tokio::test]
async fn mcp_stdio_process_executes_local_json_rpc_server_without_leaking_env_secret() {
    let mut env = BTreeMap::new();
    env.insert(
        "MCP_TOKEN".to_owned(),
        McpStdioEnvValue::SecretRef("env:DOCS_MCP_TOKEN".to_owned()),
    );
    let plan = local_stdio_fixture_plan(env, 2_000, 1_000);

    let result = execute_mcp_stdio_tool_with_env(
        plan,
        "mcp.docs.search",
        |key| (key == "DOCS_MCP_TOKEN").then(|| "super-secret-token".to_owned()),
    )
    .await
    .expect("stdio MCP call should succeed");

    assert_eq!(result.status, "succeeded");
    assert_eq!(result.output["structuredContent"]["hits"], 1);
    assert!(!serde_json::to_string(&result).unwrap().contains("super-secret-token"));
}
```

Add a timeout test:

```rust
#[tokio::test]
async fn mcp_stdio_process_timeout_returns_safe_error_evidence() {
    let plan = hanging_stdio_fixture_plan(200, 200);

    let err = execute_mcp_stdio_tool_with_env(plan, "mcp.docs.search", |_| None)
        .await
        .unwrap_err();

    assert_eq!(err.phase, "initialize");
    assert!(err.message.contains("timed out"));
    assert_eq!(err.evidence["transportKind"], "stdio");
    assert!(!err.evidence.to_string().contains("super-secret-token"));
}
```

- [ ] **Step 2: Run RED**

Run: `cargo test -p backend mcp_stdio_process --offline`

Expected: FAIL because the backend module and supervisor API do not exist.

- [ ] **Step 3: Implement one-shot process runtime**

Spawn `Command` with piped stdin/stdout/stderr, resolve `McpStdioEnvValue` entries with the injected env getter, write newline-delimited JSON-RPC messages, read one response line per request, parse the `tools/call` response through `novex-mcp`, and always attempt shutdown/kill before returning.

- [ ] **Step 4: Run GREEN**

Run: `cargo test -p backend mcp_stdio_process --offline`

Expected: PASS.

## Task 3: Agent MCP Dispatch Routing

**Files:**
- Modify: `backend/src/application/ai/agent_tool_executor.rs`

**Interfaces:**
- Produces: live MCP branch routing `stdio` tools to the stdio supervisor.
- Preserves: existing Streamable HTTP injected-dispatch tests and local HTTP smoke.

- [ ] **Step 1: Write failing dispatch tests**

Add a source-contract test requiring `transport_kind` routing:

```rust
#[test]
fn mcp_tool_execution_live_dispatch_routes_stdio_separately_from_http() {
    let source = include_str!("agent_tool_executor.rs");

    assert!(source.contains("execute_mcp_tool_with_dispatch"));
    assert!(source.contains("transport_kind.as_str()"));
    assert!(source.contains("\"stdio\""));
    assert!(source.contains("execute_mcp_stdio_tool_with_env"));
    assert!(source.contains("dispatch_mcp_streamable_http_request"));
}
```

Add a behavior test using injected dispatchers:

```rust
#[tokio::test]
async fn mcp_tool_execution_live_stdio_dispatch_uses_stdio_plan() {
    let mut tool = live_mcp_tool_record(json!({"liveExecutionEnabled": true}));
    tool.transport_kind = "stdio".to_owned();
    tool.endpoint_url = None;
    tool.auth_type = "none".to_owned();
    tool.secret_ref = None;
    tool.metadata["stdioLaunch"] = json!({
        "command": "/bin/echo",
        "args": [],
        "env": {},
        "workingDir": null,
        "lifecyclePolicy": {"startupTimeoutMs": 2000, "shutdownTimeoutMs": 1000}
    });

    let execution = execute_mcp_tool_with_dispatch(
        "mcp.docs.search",
        &json!({"query": "codex"}),
        Some(&tool),
        |_| None,
        |_plan, _bearer_token| async move { unreachable!("stdio should not use HTTP") },
        |plan, tool_code, _env_get| async move {
            assert_eq!(tool_code, "mcp.docs.search");
            assert_eq!(plan.tools_call["params"]["name"], "search");
            Ok(McpToolInvocationResult {
                tool_code: "mcp.docs.search".to_owned(),
                status: "succeeded".to_owned(),
                output: json!({"structuredContent": {"hits": 1}, "isError": false}),
                dry_run: false,
            })
        },
    )
    .await;

    assert!(execution.succeeded_status());
    assert_eq!(execution.response_payload["liveRequest"]["transportKind"], "stdio");
    assert_eq!(execution.response_payload["response"]["structuredContent"]["hits"], 1);
}
```

- [ ] **Step 2: Run RED**

Run: `cargo test -p backend mcp_tool_execution_live_stdio_dispatch --offline`

Expected: FAIL because stdio dispatch routing and injected stdio dispatcher do not exist.

- [ ] **Step 3: Implement routing**

Generalize the current HTTP-only helper to accept both HTTP and stdio dispatch closures. Parse `metadata.stdioLaunch` into `McpStdioLaunchConfig`, create `McpStdioToolCallPlan`, resolve auth as before, call the stdio dispatcher, and return the same success/failure envelope shape with `liveRequest` evidence.

- [ ] **Step 4: Run GREEN**

Run: `cargo test -p backend mcp_tool_execution --offline`

Expected: PASS.

## Task 4: Matrix and Verification

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Produces: migration matrix evidence that MCP now has stdio lifecycle contract and backend one-shot stdio process supervisor.
- Preserves: remaining gaps for browser OAuth callback handoff, persisted long-lived MCP sessions, external deployed MCP smoke, and streaming resource subscriptions.

- [ ] **Step 1: Update matrix**

Move MCP status forward to include stdio process supervision and add the verification commands below as acceptance evidence.

- [ ] **Step 2: Run final verification**

Run:

```bash
cargo fmt --all -- --check
git diff --check
cargo test -p novex-mcp mcp_stdio --offline
cargo test -p backend mcp_stdio_process --offline
cargo test -p backend mcp_tool_execution --offline
cargo test -p backend mcp --offline
```

- [ ] **Step 3: Commit, merge, and clean**

Commit implementation, merge into `main`, run `cargo clean`, remove the temporary worktree/branch, and leave the old eval worktree untouched.
