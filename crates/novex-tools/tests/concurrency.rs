use novex_tools::*;

#[test]
fn tool_router_reports_parallel_policy_for_read_only_tools() {
    let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
        .expect("agent model loop tools should build a router");

    let rag = router.tool_concurrency_policy("rag.search").unwrap();
    assert_eq!(rag.lock, ToolExecutionLock::Shared);
    assert!(rag.supports_parallel_calls);

    let media = router
        .tool_concurrency_policy("media.image.generate")
        .unwrap();
    assert_eq!(media.lock, ToolExecutionLock::Exclusive);
    assert!(!media.supports_parallel_calls);
}

#[test]
fn tool_batch_plan_allows_parallel_read_only_calls() {
    let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
        .expect("agent model loop tools should build a router");
    let calls = vec![
        router
            .route_tool_call(
                "call-1",
                "rag.search",
                serde_json::json!({"query":"policy"}),
            )
            .unwrap(),
        router
            .route_tool_call(
                "call-2",
                "github.repo.read",
                serde_json::json!({"repository":"org/repo","path":"README.md"}),
            )
            .unwrap(),
    ];

    let plan = ToolBatchPlan::from_routed_calls(calls);

    assert_eq!(plan.mode, ToolBatchExecutionMode::Parallel);
    assert_eq!(plan.serial_reason, None);
}

#[test]
fn tool_batch_plan_serializes_non_parallel_calls() {
    let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
        .expect("agent model loop tools should build a router");
    let calls = vec![
        router
            .route_tool_call(
                "call-1",
                "rag.search",
                serde_json::json!({"query":"policy"}),
            )
            .unwrap(),
        router
            .route_tool_call(
                "call-2",
                "media.image.generate",
                serde_json::json!({"prompt":"poster"}),
            )
            .unwrap(),
    ];

    let plan = ToolBatchPlan::from_routed_calls(calls);

    assert_eq!(plan.mode, ToolBatchExecutionMode::Serial);
    assert_eq!(
        plan.serial_reason.as_deref(),
        Some("exclusive_tool:media.image.generate")
    );
}

#[test]
fn tool_batch_plan_serializes_duplicate_exclusive_groups() {
    let mut first = test_tool_definition("connector.write.one");
    first.concurrency = ToolConcurrencyPolicy::exclusive("connector:crm");
    let mut second = test_tool_definition("connector.write.two");
    second.concurrency = ToolConcurrencyPolicy::exclusive("connector:crm");
    let router = ToolRouter::from_definitions(vec![first, second])
        .expect("router should build from exclusive test tools");
    let calls = vec![
        router
            .route_tool_call("call-1", "connector.write.one", serde_json::json!({}))
            .unwrap(),
        router
            .route_tool_call("call-2", "connector.write.two", serde_json::json!({}))
            .unwrap(),
    ];

    let plan = ToolBatchPlan::from_routed_calls(calls);

    assert_eq!(plan.mode, ToolBatchExecutionMode::Serial);
    assert_eq!(
        plan.serial_reason.as_deref(),
        Some("exclusive_group:connector:crm")
    );
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
