use std::fs;
use std::path::Path;

use novex_mcp::{
    mcp_oauth_session_from_token_response, mcp_tool_code, parse_mcp_tool_call_response,
    validate_mcp_registration_policy, McpAuthScope, McpAuthType, McpDiscoveredTool,
    McpJsonRpcRequest, McpOAuthAuthorizationConfig, McpOAuthAuthorizationPlan, McpOAuthClientAuth,
    McpOAuthPkceMethod, McpOAuthTokenResponse, McpRegistrationPolicy, McpServerStatus,
    McpStdioEnvValue, McpStdioLaunchConfig, McpStdioLaunchPlan, McpStdioLifecyclePolicy,
    McpStreamableHttpRequestPlan, McpStreamableHttpResponse, McpToolInvocationRequest,
    McpTransportKind, MCP_PROTOCOL_VERSION,
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
    assert_eq!(
        mcp_tool_code("Docs Server", "Search/File"),
        "mcp.docs_server.search_file"
    );

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
    assert_eq!(
        definition.permission_code.as_deref(),
        Some("ai:mcp:docs:search")
    );
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
    assert!(oauth
        .authorization_url
        .contains("code_challenge_method=S256"));

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
