use novex_tools::*;

#[test]
fn web_search_executor_binding_is_builtin() {
    let registry = ToolExecutorRegistry::from_bindings(agent_model_loop_tool_executor_bindings())
        .expect("agent executor registry should build");

    let web = registry
        .executor_for("web.search")
        .expect("web.search should have an executor");

    assert_eq!(web.executor_code, "builtin.web.search");
    assert_eq!(web.kind, ToolExecutorKind::Builtin);
}

#[test]
fn tool_executor_registry_routes_known_agent_tools() {
    let registry = ToolExecutorRegistry::from_bindings(agent_model_loop_tool_executor_bindings())
        .expect("agent executor registry should build");

    let rag = registry
        .executor_for(" rag.search ")
        .expect("rag.search should have an executor");
    assert_eq!(rag.executor_code, "builtin.rag.search");
    assert_eq!(rag.kind, ToolExecutorKind::Builtin);

    let media = registry
        .executor_for("media.image.generate")
        .expect("media image should have an executor");
    assert_eq!(media.kind, ToolExecutorKind::Model);
    assert!(media.supports_background_tasks);
}

#[test]
fn tool_executor_dispatch_plan_derives_runtime_dependencies() {
    let connector = ToolExecutorDispatchPlan::from_binding(&ToolExecutorBinding::new(
        "github.repo.search",
        "connector.github.repo.search",
        ToolExecutorKind::Connector,
    ));
    assert_eq!(connector.tool_code, "github.repo.search");
    assert_eq!(connector.executor_code, "connector.github.repo.search");
    assert!(connector.requires_connector_credential);
    assert!(!connector.requires_mcp_tool);
    assert!(!connector.requires_model_runtime);

    let model = ToolExecutorDispatchPlan::from_binding(
        &ToolExecutorBinding::new(
            "media.image.generate",
            "model.media.image.generate",
            ToolExecutorKind::Model,
        )
        .with_background_tasks()
        .waits_for_runtime_cancellation(),
    );
    assert!(model.requires_model_runtime);
    assert!(model.supports_background_tasks);
    assert!(model.waits_for_runtime_cancellation);

    let mcp = ToolExecutorDispatchPlan::from_binding(&ToolExecutorBinding::new(
        "mcp.repo.lookup",
        "mcp.repo.lookup",
        ToolExecutorKind::Mcp,
    ));
    assert!(mcp.requires_mcp_tool);
}

#[test]
fn tool_executor_registry_rejects_duplicate_and_missing_bindings() {
    let duplicate = ToolExecutorRegistry::from_bindings(vec![
        ToolExecutorBinding::new(
            "rag.search",
            "builtin.rag.search",
            ToolExecutorKind::Builtin,
        ),
        ToolExecutorBinding::new(
            "rag.search",
            "builtin.rag.search.v2",
            ToolExecutorKind::Builtin,
        ),
    ])
    .unwrap_err();
    assert_eq!(
        duplicate.kind,
        ToolExecutorRegistryErrorKind::DuplicateToolCode
    );

    let missing = ToolExecutorRegistry::default()
        .executor_for("sandbox.exec")
        .unwrap_err();
    assert_eq!(missing.kind, ToolExecutorRegistryErrorKind::MissingExecutor);
    assert_eq!(missing.tool_code.as_deref(), Some("sandbox.exec"));
}

#[test]
fn agent_model_loop_executor_bindings_cover_agent_model_loop_tools() {
    let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
        .expect("agent model loop tools should build a router");
    let registry = ToolExecutorRegistry::from_bindings(agent_model_loop_tool_executor_bindings())
        .expect("agent executor registry should build");

    assert_eq!(registry.tool_codes(), router.tool_codes());
}
