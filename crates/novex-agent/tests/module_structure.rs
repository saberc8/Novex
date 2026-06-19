use std::fs;
use std::path::Path;

use novex_agent::{
    module, plan_react_run, route_intent, select_tool, AgentIntent, AgentLoopKind, AgentRunPlan,
    SelectedTool,
};
use novex_ai_core::{FoundationStatus, TaskBudget};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_agent_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["intent", "module", "plan", "text", "tool_selection"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum AgentIntent",
        "pub enum AgentLoopKind",
        "pub struct SelectedTool",
        "pub struct AgentRunPlan",
        "pub fn route_intent",
        "pub fn select_tool",
        "pub fn plan_react_run",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn agent_domain_modules_exist() {
    for module in [
        "src/intent.rs",
        "src/module.rs",
        "src/plan.rs",
        "src/text.rs",
        "src/tool_selection.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_agent_contracts() {
    let module = module();
    assert_eq!(module.id, "novex-agent");
    assert_eq!(module.status, FoundationStatus::Skeleton);

    assert_eq!(
        route_intent("send a Feishu reminder"),
        AgentIntent::ToolTask
    );
    let tool: SelectedTool = select_tool("read GitHub file src/lib.rs").unwrap();
    assert_eq!(tool.code, "github.repo.read");

    let plan: AgentRunPlan = plan_react_run(
        "send a Feishu reminder",
        TaskBudget {
            max_steps: Some(6),
            max_tool_calls: Some(2),
            max_seconds: Some(30),
            max_cost_cents: Some(0),
        },
    )
    .unwrap();
    assert_eq!(plan.loop_kind, AgentLoopKind::ReAct);
    assert_eq!(plan.intent, AgentIntent::ToolTask);
    assert!(plan.requires_approval);
    assert!(plan.steps.iter().any(|step| step == "action"));
}
