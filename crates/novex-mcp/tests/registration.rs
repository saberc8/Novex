use novex_mcp::*;

#[test]
fn registration_policy_builds_tenant_scoped_discovery_plan() {
    let plan = validate_mcp_registration_policy(&McpRegistrationPolicy {
        server_code: "docs.search".to_owned(),
        endpoint_url: Some("https://mcp.example.com/sse".to_owned()),
        transport_kind: McpTransportKind::StreamableHttp,
        auth_scope: McpAuthScope::Tenant,
        auth_type: McpAuthType::BearerEnv,
        secret_ref: Some("env:DOCS_MCP_TOKEN".to_owned()),
        network_allowlist: vec!["mcp.example.com".to_owned()],
        tool_allowlist: vec!["docs.search".to_owned()],
    })
    .expect("valid MCP registration should build a discovery plan");

    assert_eq!(plan.server_code, "docs.search");
    assert_eq!(plan.status, McpServerStatus::Discovering);
    assert_eq!(plan.allowed_tools, vec!["docs.search"]);
    assert!(plan.requires_secret);
    assert!(plan.audit_required);
}

#[test]
fn registration_policy_rejects_endpoint_outside_network_allowlist() {
    let err = validate_mcp_registration_policy(&McpRegistrationPolicy {
        server_code: "docs.search".to_owned(),
        endpoint_url: Some("https://mcp.example.com/sse".to_owned()),
        transport_kind: McpTransportKind::StreamableHttp,
        auth_scope: McpAuthScope::Tenant,
        auth_type: McpAuthType::BearerEnv,
        secret_ref: Some("env:DOCS_MCP_TOKEN".to_owned()),
        network_allowlist: vec!["api.example.com".to_owned()],
        tool_allowlist: vec!["docs.search".to_owned()],
    })
    .unwrap_err();

    assert_eq!(err.field, "network_allowlist");
}
