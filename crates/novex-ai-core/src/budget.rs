use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TaskBudget {
    pub max_steps: Option<u32>,
    pub max_tool_calls: Option<u32>,
    pub max_seconds: Option<u32>,
    pub max_cost_cents: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetValidationError {
    pub field: String,
    pub max: u32,
}

pub const DEFAULT_MAX_STEPS: u32 = 12;
pub const DEFAULT_MAX_TOOL_CALLS: u32 = 4;
pub const DEFAULT_MAX_SECONDS: u32 = 120;
pub const DEFAULT_MAX_COST_CENTS: u32 = 0;

pub const POC_MAX_STEPS: u32 = 100;
pub const POC_MAX_TOOL_CALLS: u32 = 20;
pub const POC_MAX_SECONDS: u32 = 600;
pub const POC_MAX_COST_CENTS: u32 = 1_000;

pub fn normalize_task_budget(budget: TaskBudget) -> Result<TaskBudget, BudgetValidationError> {
    let normalized = TaskBudget {
        max_steps: Some(budget.max_steps.unwrap_or(DEFAULT_MAX_STEPS)),
        max_tool_calls: Some(budget.max_tool_calls.unwrap_or(DEFAULT_MAX_TOOL_CALLS)),
        max_seconds: Some(budget.max_seconds.unwrap_or(DEFAULT_MAX_SECONDS)),
        max_cost_cents: Some(budget.max_cost_cents.unwrap_or(DEFAULT_MAX_COST_CENTS)),
    };

    validate_budget_field("max_steps", normalized.max_steps, POC_MAX_STEPS)?;
    validate_budget_field(
        "max_tool_calls",
        normalized.max_tool_calls,
        POC_MAX_TOOL_CALLS,
    )?;
    validate_budget_field("max_seconds", normalized.max_seconds, POC_MAX_SECONDS)?;
    validate_budget_field(
        "max_cost_cents",
        normalized.max_cost_cents,
        POC_MAX_COST_CENTS,
    )?;

    Ok(normalized)
}

fn validate_budget_field(
    field: &'static str,
    value: Option<u32>,
    max: u32,
) -> Result<(), BudgetValidationError> {
    if value.unwrap_or_default() > max {
        return Err(BudgetValidationError {
            field: field.to_owned(),
            max,
        });
    }
    Ok(())
}
