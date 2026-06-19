use novex_agent::select_tool;

#[test]
fn agent_runtime_selects_seeded_poc_tools() {
    assert_eq!(
        select_tool("search the handbook").unwrap().code,
        "rag.search"
    );
    assert_eq!(
        select_tool("generate an image for the course")
            .unwrap()
            .code,
        "media.image.generate"
    );
    assert_eq!(
        select_tool("send a Feishu notification").unwrap().code,
        "feishu.message.send"
    );
    assert_eq!(
        select_tool("search GitHub repo for parser worker")
            .unwrap()
            .code,
        "github.repo.search"
    );
    assert_eq!(
        select_tool("read GitHub file src/lib.rs").unwrap().code,
        "github.repo.read"
    );
}

#[test]
fn agent_runtime_selected_tool_carries_shared_policy_decision() {
    let tool = select_tool("send a Feishu notification").unwrap();

    assert_eq!(tool.code, "feishu.message.send");
    assert_eq!(tool.policy_decision.tool_code, tool.code);
    assert!(tool.policy_decision.requires_approval);
    assert_eq!(
        tool.policy_decision.pause_reason.as_deref(),
        Some("approval")
    );
    assert_eq!(
        tool.requires_approval,
        tool.policy_decision.requires_approval
    );
}
