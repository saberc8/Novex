use novex_agent::{plan_react_run, plan_react_run_with_memory, AgentIntent, AgentLoopKind};
use novex_ai_core::TaskBudget;
use novex_memory::{
    build_memory_context, MemoryAccessContext, MemoryScope, MemoryScopeRef, MemorySnippet,
    MemoryWritePolicy,
};

#[test]
fn agent_runtime_plan_contains_react_steps_and_budget() {
    let plan = plan_react_run(
        "send a Feishu reminder",
        TaskBudget {
            max_steps: Some(6),
            max_tool_calls: Some(2),
            max_seconds: Some(30),
            max_cost_cents: Some(0),
        },
    )
    .unwrap();

    assert_eq!(plan.intent, AgentIntent::ToolTask);
    assert_eq!(plan.loop_kind, AgentLoopKind::ReAct);
    assert_eq!(
        plan.selected_tool.as_ref().unwrap().code,
        "feishu.message.send"
    );
    assert!(plan.requires_approval);
    assert_eq!(plan.budget.max_steps, Some(6));
    assert!(plan.steps.iter().any(|step| step == "thought"));
    assert!(plan.steps.iter().any(|step| step == "action"));
    assert!(plan.steps.iter().any(|step| step == "observation"));
}

#[test]
fn agent_runtime_plan_carries_filtered_memory_context() {
    let memory_context = build_memory_context(
        vec![MemorySnippet {
            tenant_id: "tenant-a".to_owned(),
            scope: MemoryScope::User,
            scope_id: "user-1".to_owned(),
            key: "profile.locale".to_owned(),
            content: "Prefers Chinese answers".to_owned(),
            write_policy: MemoryWritePolicy::UserApproved,
        }],
        &MemoryAccessContext {
            tenant_id: "tenant-a".to_owned(),
            subject_id: "user-1".to_owned(),
            allowed_scopes: vec![MemoryScopeRef {
                scope: MemoryScope::User,
                scope_id: "user-1".to_owned(),
            }],
            max_snippets: 4,
        },
    );

    let plan = plan_react_run_with_memory(
        "answer in my preferred language",
        TaskBudget {
            max_steps: Some(4),
            max_tool_calls: Some(1),
            max_seconds: Some(20),
            max_cost_cents: Some(0),
        },
        memory_context,
    )
    .unwrap();

    assert_eq!(plan.memory_context.snippets.len(), 1);
    assert_eq!(plan.memory_context.snippets[0].key, "profile.locale");
}

#[test]
fn agent_runtime_plan_rejects_budget_above_poc_limits() {
    let err = plan_react_run(
        "search the handbook",
        TaskBudget {
            max_steps: Some(101),
            max_tool_calls: Some(2),
            max_seconds: Some(30),
            max_cost_cents: Some(0),
        },
    )
    .unwrap_err();

    assert_eq!(err.field(), Some("max_steps"));
}
