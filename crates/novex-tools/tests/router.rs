use novex_tools::*;

#[test]
fn tool_definition_converts_to_model_visible_spec() {
    let tool = ToolDefinition {
        code: "rag.search".to_owned(),
        name: "Search knowledge".to_owned(),
        description: "Search tenant-scoped knowledge base.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": { "type": "string" }
            }
        }),
        output_schema: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "hits": { "type": "array" }
            }
        })),
        risk_level: ToolRiskLevel::Low,
        approval_policy: ApprovalPolicy::OnRisk,
        permission_code: Some("ai:knowledge:ask".to_owned()),
        concurrency: ToolConcurrencyPolicy::shared(),
    };

    let spec = tool.to_model_tool_spec();

    assert_eq!(spec.name, "rag.search");
    assert_eq!(spec.parameters["required"][0], "query");
    assert_eq!(spec.metadata["riskLevel"], "low");
}

#[test]
fn tool_router_exposes_sorted_model_visible_specs() {
    let router = ToolRouter::from_definitions(vec![
        test_tool_definition("media.image.generate"),
        test_tool_definition("rag.search"),
    ])
    .unwrap();

    assert_eq!(
        router.tool_codes(),
        vec!["media.image.generate".to_owned(), "rag.search".to_owned()]
    );
    assert_eq!(router.model_tool_specs()[0].name, "media.image.generate");
}

#[test]
fn tool_router_rejects_duplicate_tool_codes() {
    let err = ToolRouter::from_definitions(vec![
        test_tool_definition("rag.search"),
        test_tool_definition("rag.search"),
    ])
    .unwrap_err();

    assert_eq!(err.kind, ToolRouteErrorKind::DuplicateToolCode);
    assert_eq!(err.tool_code.as_deref(), Some("rag.search"));
}

#[test]
fn tool_router_rejects_unknown_model_tool_call() {
    let router = ToolRouter::from_definitions(vec![test_tool_definition("rag.search")])
        .expect("router should build from one definition");

    let err = router
        .route_tool_call("call-1", "sandbox.exec", serde_json::json!({}))
        .unwrap_err();

    assert_eq!(err.kind, ToolRouteErrorKind::UnknownTool);
    assert_eq!(err.tool_code.as_deref(), Some("sandbox.exec"));
}

fn test_tool_definition(code: &str) -> ToolDefinition {
    ToolDefinition {
        code: code.to_owned(),
        name: code.to_owned(),
        description: format!("Tool {code}"),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        output_schema: None,
        risk_level: ToolRiskLevel::Low,
        approval_policy: ApprovalPolicy::OnRisk,
        permission_code: Some("ai:tool:dryRun".to_owned()),
        concurrency: ToolConcurrencyPolicy::shared(),
    }
}
