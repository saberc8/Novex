use std::fs;
use std::path::Path;

use novex_tools::{
    agent_model_loop_tool_definitions, agent_model_loop_tool_executor_bindings,
    approval_policy_code, customer_service_tool_definitions, evaluate_tool_execution_policy,
    feishu_message_text_from_tool_input, github_read_request_from_tool_input,
    github_search_request_from_tool_input, media_image_request_from_tool_input,
    parse_media_image_generation_response, tool_risk_code, AgentToolExecution, ApprovalPolicy,
    MediaImageGenerationRequest, ToolBatchExecutionMode, ToolBatchPlan, ToolConcurrencyPolicy,
    ToolExecutionLock, ToolExecutionPolicyInput, ToolExecutorBinding, ToolExecutorDispatchPlan,
    ToolExecutorKind, ToolExecutorRegistry, ToolKind, ToolRiskLevel, ToolRouteErrorKind,
    ToolRouter,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_tool_modules() {
    let lib = crate_file("src/lib.rs");

    for module in [
        "adapters",
        "concurrency",
        "definitions",
        "executor",
        "media",
        "policy",
        "router",
        "types",
    ] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub struct ToolDefinition",
        "pub struct ToolRouter",
        "pub struct ToolExecutorRegistry",
        "pub fn agent_model_loop_tool_definitions",
        "pub fn evaluate_tool_execution_policy",
        "pub fn parse_media_image_generation_response",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn tool_domain_modules_exist() {
    for module in [
        "src/adapters.rs",
        "src/concurrency.rs",
        "src/definitions.rs",
        "src/executor.rs",
        "src/media.rs",
        "src/policy.rs",
        "src/router.rs",
        "src/types.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_core_policy_router_and_executor_contracts() {
    assert_eq!(tool_risk_code(ToolRiskLevel::High), "high");
    assert_eq!(approval_policy_code(ApprovalPolicy::Always), "always");
    assert_eq!(ToolKind::Mcp as u8, ToolKind::Mcp as u8);

    let decision = evaluate_tool_execution_policy(ToolExecutionPolicyInput {
        tool_code: "ticket.create".to_owned(),
        risk_level: ToolRiskLevel::High,
        approval_policy: ApprovalPolicy::OnRisk,
        permission_code: Some("ai:customer-service:ticket".to_owned()),
        auto_approved: true,
    });
    assert!(decision.requires_approval);
    assert_eq!(decision.pause_reason.as_deref(), Some("approval"));

    let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions()).unwrap();
    let call = router
        .route_tool_call(
            "call-1",
            "rag.search",
            serde_json::json!({"query": "policy"}),
        )
        .unwrap();
    assert_eq!(call.tool.code, "rag.search");
    assert_eq!(
        router
            .route_tool_call("call-2", "missing.tool", serde_json::json!({}))
            .unwrap_err()
            .kind,
        ToolRouteErrorKind::UnknownTool
    );

    let registry = ToolExecutorRegistry::from_bindings(agent_model_loop_tool_executor_bindings())
        .expect("agent executor registry should build");
    let web = registry.executor_for("web.search").unwrap();
    assert_eq!(web.kind, ToolExecutorKind::Builtin);

    let dispatch = ToolExecutorDispatchPlan::from_binding(&ToolExecutorBinding::new(
        "media.image.generate",
        "model.media.image.generate",
        ToolExecutorKind::Model,
    ));
    assert!(dispatch.requires_model_runtime);
}

#[test]
fn root_facade_preserves_concurrency_and_definition_contracts() {
    let read_only = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
        .unwrap()
        .route_tool_call(
            "call-1",
            "github.repo.read",
            serde_json::json!({"repository":"org/repo","path":"README.md"}),
        )
        .unwrap();
    let rag = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
        .unwrap()
        .route_tool_call(
            "call-2",
            "rag.search",
            serde_json::json!({"query":"policy"}),
        )
        .unwrap();
    let plan = ToolBatchPlan::from_routed_calls(vec![read_only, rag]);

    assert_eq!(plan.mode, ToolBatchExecutionMode::Parallel);
    assert_eq!(
        ToolConcurrencyPolicy::shared().lock,
        ToolExecutionLock::Shared
    );
    assert!(customer_service_tool_definitions()
        .iter()
        .any(|tool| tool.code == "ticket.create" && tool.risk_level == ToolRiskLevel::High));
}

#[test]
fn root_facade_preserves_adapter_media_and_execution_contracts() {
    let feishu = feishu_message_text_from_tool_input(&serde_json::json!({
        "message": "Complete training today",
        "input": "ignored"
    }));
    assert_eq!(feishu, "Complete training today");

    let media_request = media_image_request_from_tool_input(&serde_json::json!({
        "prompt": "Create poster",
        "size": "1024x1024",
        "count": 2
    }));
    assert_eq!(media_request.prompt, "Create poster");
    assert_eq!(media_request.count, 2);

    let provider_payload = MediaImageGenerationRequest::new("Create poster")
        .with_size("1024x1024")
        .with_count(2)
        .to_provider_payload();
    assert_eq!(provider_payload["n"], 2);

    let media_result = parse_media_image_generation_response(&serde_json::json!({
        "id": "img-1",
        "data": [{"url": "https://cdn.example.com/img.png"}]
    }))
    .unwrap();
    assert_eq!(media_result.provider_asset_id.as_deref(), Some("img-1"));

    let github_search = github_search_request_from_tool_input(&serde_json::json!({
        "input": "search GitHub repo acme/app for parser worker under src"
    }))
    .unwrap();
    assert_eq!(github_search.repository, "acme/app");
    assert_eq!(github_search.query, "parser worker");

    let github_read = github_read_request_from_tool_input(&serde_json::json!({
        "input": "read GitHub file acme/app src/lib.rs ref main"
    }))
    .unwrap();
    assert_eq!(github_read.path, "src/lib.rs");

    let execution = AgentToolExecution::succeeded(
        serde_json::json!({"status": "succeeded"}),
        true,
        "ok".to_owned(),
    );
    assert!(execution.succeeded_status());
}
