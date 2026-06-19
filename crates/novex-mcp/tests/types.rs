use novex_mcp::*;
use novex_tools::ToolRiskLevel;

#[test]
fn mcp_discovered_tool_converts_to_tenant_tool_definition() {
    let tool = McpDiscoveredTool {
        server_code: "docs".to_owned(),
        tool_name: "search".to_owned(),
        description: "Search docs".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string"
                }
            }
        }),
        output_schema: Some(serde_json::json!({
            "type": "object"
        })),
        risk_level: ToolRiskLevel::Low,
    };

    let definition = tool.to_tool_definition("ai:mcp:docs:search");

    assert_eq!(definition.code, "mcp.docs.search");
    assert_eq!(
        definition.input_schema["properties"]["query"]["type"],
        "string"
    );
    assert_eq!(
        definition.permission_code.as_deref(),
        Some("ai:mcp:docs:search")
    );
}
