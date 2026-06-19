use std::collections::BTreeMap;

use novex_mcp::*;

#[test]
fn mcp_stdio_launch_plan_sanitizes_env_secret_refs() {
    let mut env = BTreeMap::new();
    env.insert(
        "DOCS_MCP_TOKEN".to_owned(),
        McpStdioEnvValue::SecretRef("env:DOCS_MCP_TOKEN".to_owned()),
    );
    env.insert(
        "LOG_LEVEL".to_owned(),
        McpStdioEnvValue::Literal("debug-secret-literal".to_owned()),
    );

    let plan = McpStdioLaunchPlan::new(McpStdioLaunchConfig {
        command: "npx".to_owned(),
        args: vec![
            "-y".to_owned(),
            "@modelcontextprotocol/server-filesystem".to_owned(),
        ],
        env,
        working_dir: Some("/srv/docs".to_owned()),
        lifecycle_policy: McpStdioLifecyclePolicy::new(10_000, 5_000)
            .expect("timeouts should be in range"),
    })
    .expect("stdio launch plan should be valid");

    let evidence = plan.sanitized_evidence();

    assert_eq!(evidence["command"], "npx");
    assert_eq!(evidence["args"][0], "-y");
    assert_eq!(evidence["workingDir"], "/srv/docs");
    assert_eq!(evidence["env"]["DOCS_MCP_TOKEN"]["kind"], "secret_ref");
    assert_eq!(
        evidence["env"]["DOCS_MCP_TOKEN"]["secretRef"],
        "env:DOCS_MCP_TOKEN"
    );
    assert_eq!(evidence["env"]["LOG_LEVEL"]["kind"], "literal");
    assert_eq!(evidence["lifecyclePolicy"]["startupTimeoutMs"], 10_000);
    assert_eq!(evidence["lifecyclePolicy"]["shutdownTimeoutMs"], 5_000);
    assert!(!evidence.to_string().contains("debug-secret-literal"));
}

#[test]
fn mcp_stdio_launch_plan_rejects_empty_command() {
    let err = McpStdioLaunchPlan::new(McpStdioLaunchConfig {
        command: "   ".to_owned(),
        args: vec![],
        env: BTreeMap::new(),
        working_dir: None,
        lifecycle_policy: McpStdioLifecyclePolicy::new(1_000, 1_000)
            .expect("timeouts should be in range"),
    })
    .unwrap_err();

    assert_eq!(err.field, "command");
}

#[test]
fn mcp_stdio_launch_plan_rejects_invalid_env_secret_ref() {
    let mut env = BTreeMap::new();
    env.insert(
        "DOCS_MCP_TOKEN".to_owned(),
        McpStdioEnvValue::SecretRef("DOCS_MCP_TOKEN".to_owned()),
    );

    let err = McpStdioLaunchPlan::new(McpStdioLaunchConfig {
        command: "node".to_owned(),
        args: vec!["server.js".to_owned()],
        env,
        working_dir: None,
        lifecycle_policy: McpStdioLifecyclePolicy::new(1_000, 1_000)
            .expect("timeouts should be in range"),
    })
    .unwrap_err();

    assert_eq!(err.field, "env.DOCS_MCP_TOKEN.secret_ref");
}

#[test]
fn mcp_stdio_lifecycle_policy_rejects_out_of_bounds_timeouts() {
    let startup_err =
        McpStdioLifecyclePolicy::new(MCP_STDIO_MIN_TIMEOUT_MS - 1, 1_000).unwrap_err();
    let shutdown_err =
        McpStdioLifecyclePolicy::new(1_000, MCP_STDIO_MAX_TIMEOUT_MS + 1).unwrap_err();

    assert_eq!(startup_err.field, "startup_timeout_ms");
    assert_eq!(shutdown_err.field, "shutdown_timeout_ms");
}

#[test]
fn mcp_stdio_lifecycle_plan_lists_expected_phases() {
    let plan = McpStdioLaunchPlan::new(McpStdioLaunchConfig {
        command: "node".to_owned(),
        args: vec!["server.js".to_owned()],
        env: BTreeMap::new(),
        working_dir: None,
        lifecycle_policy: McpStdioLifecyclePolicy::new(2_000, 1_000)
            .expect("timeouts should be in range"),
    })
    .expect("stdio launch plan should be valid");

    assert_eq!(
        plan.lifecycle_phases(),
        vec![
            McpStdioLifecyclePhase::Spawn,
            McpStdioLifecyclePhase::Initialize,
            McpStdioLifecyclePhase::ListTools,
            McpStdioLifecyclePhase::CallTools,
            McpStdioLifecyclePhase::Shutdown,
        ]
    );
    assert_eq!(
        plan.sanitized_evidence()["lifecyclePhases"],
        serde_json::json!([
            "spawn",
            "initialize",
            "list_tools",
            "call_tools",
            "shutdown"
        ])
    );
}

#[test]
fn mcp_stdio_tool_call_plan_builds_initialize_and_call_messages() {
    let launch = McpStdioLaunchPlan::new(McpStdioLaunchConfig {
        command: "node".to_owned(),
        args: vec!["server.js".to_owned()],
        env: BTreeMap::new(),
        working_dir: None,
        lifecycle_policy: McpStdioLifecyclePolicy::new(2_000, 1_000)
            .expect("timeouts should be in range"),
    })
    .expect("stdio launch plan should be valid");
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
        lifecycle_policy: McpStdioLifecyclePolicy::new(2_000, 1_000)
            .expect("timeouts should be in range"),
    })
    .expect("stdio launch plan should be valid");
    let request = McpToolInvocationRequest {
        server_code: "docs".to_owned(),
        tool_name: "search".to_owned(),
        arguments: serde_json::json!({"query": "codex"}),
    };

    let evidence = McpStdioToolCallPlan::new(launch, "tool-call-1", &request).sanitized_evidence();

    assert_eq!(evidence["transportKind"], "stdio");
    assert_eq!(evidence["request"]["method"], "tools/call");
    assert_eq!(evidence["request"]["params"]["arguments"]["query"], "codex");
    assert!(!evidence.to_string().contains("plain-secret"));
}
