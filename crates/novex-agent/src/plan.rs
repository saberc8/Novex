use novex_ai_core::{normalize_task_budget, BudgetValidationError, TaskBudget};
use novex_memory::MemoryContext;
use serde::{Deserialize, Serialize};

use crate::intent::{route_intent, AgentIntent};
use crate::tool_selection::{select_tool, SelectedTool};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentLoopKind {
    ReAct,
    Planner,
    SupervisorWorker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunPlan {
    pub intent: AgentIntent,
    pub loop_kind: AgentLoopKind,
    pub selected_tool: Option<SelectedTool>,
    pub requires_approval: bool,
    pub memory_context: MemoryContext,
    pub budget: TaskBudget,
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentPlanError {
    Budget(BudgetValidationError),
}

impl AgentPlanError {
    pub fn field(&self) -> Option<&str> {
        match self {
            Self::Budget(err) => Some(err.field.as_str()),
        }
    }
}

pub fn plan_react_run(input: &str, budget: TaskBudget) -> Result<AgentRunPlan, AgentPlanError> {
    plan_react_run_with_memory(input, budget, MemoryContext::empty())
}

pub fn plan_react_run_with_memory(
    input: &str,
    budget: TaskBudget,
    memory_context: MemoryContext,
) -> Result<AgentRunPlan, AgentPlanError> {
    let budget = normalize_task_budget(budget).map_err(AgentPlanError::Budget)?;
    let intent = route_intent(input);
    let selected_tool = select_tool(input);
    let requires_approval = selected_tool
        .as_ref()
        .is_some_and(|tool| tool.requires_approval);
    let steps = if selected_tool.is_some() {
        vec!["input", "thought", "action", "observation", "final"]
    } else {
        vec!["input", "thought", "final"]
    }
    .into_iter()
    .map(str::to_owned)
    .collect();

    Ok(AgentRunPlan {
        intent,
        loop_kind: AgentLoopKind::ReAct,
        selected_tool,
        requires_approval,
        memory_context,
        budget,
        steps,
    })
}
