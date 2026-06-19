use novex_mcp::*;

#[test]
fn mcp_streamable_http_request_plan_builds_sanitized_tools_call() {
    let request = McpToolInvocationRequest {
        server_code: "docs".to_owned(),
        tool_name: "search".to_owned(),
        arguments: serde_json::json!({"query": "codex"}),
    };

    let plan = McpStreamableHttpRequestPlan::tools_call(
        "https://mcp.example.com/mcp",
        "tool-call-1",
        &request,
        Some("env:DOCS_MCP_TOKEN"),
    );

    assert_eq!(plan.http_method, "POST");
    assert_eq!(
        plan.header_value("Accept").as_deref(),
        Some("application/json, text/event-stream")
    );
    assert_eq!(
        plan.header_value("Content-Type").as_deref(),
        Some("application/json")
    );
    assert_eq!(
        plan.header_value("MCP-Protocol-Version").as_deref(),
        Some(MCP_PROTOCOL_VERSION)
    );
    assert_eq!(plan.body["jsonrpc"], "2.0");
    assert_eq!(plan.body["method"], "tools/call");
    assert_eq!(plan.body["params"]["name"], "search");
    assert_eq!(plan.body["params"]["arguments"]["query"], "codex");
    let evidence = plan.sanitized_evidence();
    assert_eq!(evidence["secretRef"], "env:DOCS_MCP_TOKEN");
    assert!(!evidence.to_string().contains("test-token"));
}

#[test]
fn mcp_streamable_http_json_response_maps_tool_result() {
    let raw = McpStreamableHttpResponse::new(
        200,
        "application/json",
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "tool-call-1",
            "result": {
                "content": [{"type": "text", "text": "Found policy"}],
                "structuredContent": {"hits": 1},
                "isError": false
            }
        })
        .to_string(),
    );

    let result = parse_mcp_tool_call_response("mcp.docs.search", &raw).unwrap();

    assert_eq!(result.tool_code, "mcp.docs.search");
    assert_eq!(result.status, "succeeded");
    assert!(!result.dry_run);
    assert_eq!(result.output["structuredContent"]["hits"], 1);
    assert_eq!(result.output["content"][0]["text"], "Found policy");
}

#[test]
fn mcp_streamable_http_sse_response_maps_tool_result() {
    let raw = McpStreamableHttpResponse::new(
        200,
        "text/event-stream",
        concat!(
            "event: message\n",
            "data: {\"jsonrpc\":\"2.0\",\"id\":\"tool-call-1\",\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"streamed\"}],\"isError\":false}}\n\n"
        )
        .to_owned(),
    );

    let result = parse_mcp_tool_call_response("mcp.docs.search", &raw).unwrap();

    assert_eq!(result.status, "succeeded");
    assert_eq!(result.output["content"][0]["text"], "streamed");
}

#[test]
fn mcp_streamable_http_json_rpc_error_is_structured() {
    let raw = McpStreamableHttpResponse::new(
        200,
        "application/json",
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "tool-call-1",
            "error": {
                "code": -32602,
                "message": "Invalid arguments"
            }
        })
        .to_string(),
    );

    let err = parse_mcp_tool_call_response("mcp.docs.search", &raw).unwrap_err();

    assert_eq!(err.kind, McpClientErrorKind::JsonRpcError);
    assert_eq!(err.rpc_code, Some(-32602));
    assert!(err.message.contains("Invalid arguments"));
}
