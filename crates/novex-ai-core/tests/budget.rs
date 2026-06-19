use novex_ai_core::{normalize_task_budget, TaskBudget};

#[test]
fn run_graph_task_budget_normalizes_and_rejects_poc_limit_overrides() {
    let budget = normalize_task_budget(TaskBudget {
        max_steps: Some(3),
        max_tool_calls: Some(1),
        max_seconds: None,
        max_cost_cents: None,
    })
    .unwrap();

    assert_eq!(budget.max_steps, Some(3));
    assert_eq!(budget.max_tool_calls, Some(1));

    let err = normalize_task_budget(TaskBudget {
        max_steps: Some(101),
        max_tool_calls: Some(1),
        max_seconds: None,
        max_cost_cents: None,
    })
    .unwrap_err();

    assert_eq!(err.field, "max_steps");
}
