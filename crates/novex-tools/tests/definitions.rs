use novex_tools::*;

#[test]
fn customer_service_tools_have_risk_and_schema_contracts() {
    let tools = customer_service_tool_definitions();

    assert!(tools.iter().any(|tool| tool.code == "faq.search"));
    assert!(tools.iter().any(|tool| tool.code == "customer.lookup"));
    assert!(tools.iter().any(|tool| tool.code == "ticket.create"));
    assert!(tools.iter().any(|tool| tool.code == "handoff.request"));

    let ticket = tools
        .iter()
        .find(|tool| tool.code == "ticket.create")
        .expect("ticket.create tool should exist");
    assert_eq!(ticket.risk_level, ToolRiskLevel::High);
    assert_eq!(ticket.approval_policy, ApprovalPolicy::Always);
    assert_eq!(
        ticket.permission_code.as_deref(),
        Some("ai:customer-service:ticket")
    );
    assert_eq!(ticket.input_schema["required"][0], "customerId");
}

#[test]
fn agent_model_loop_tool_definitions_cover_builtin_agent_tools() {
    let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
        .expect("agent model loop tools should build a router");
    let codes = router.tool_codes();

    assert!(codes.contains(&"rag.search".to_owned()));
    assert!(codes.contains(&"web.search".to_owned()));
    assert!(codes.contains(&"github.repo.search".to_owned()));
    assert!(codes.contains(&"github.repo.read".to_owned()));
    assert!(codes.contains(&"media.image.generate".to_owned()));
    assert!(codes.contains(&"feishu.message.send".to_owned()));
}
