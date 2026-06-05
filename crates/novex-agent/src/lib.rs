use novex_ai_core::{normalize_task_budget, BudgetValidationError, FoundationModule, TaskBudget};
use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-agent";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentIntent {
    Chat,
    RagQuestion,
    ToolTask,
    CodeSearch,
    TrainingQuiz,
    HumanHandoff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentLoopKind {
    ReAct,
    Planner,
    SupervisorWorker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedTool {
    pub code: String,
    pub risk_level: u8,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunPlan {
    pub intent: AgentIntent,
    pub loop_kind: AgentLoopKind,
    pub selected_tool: Option<SelectedTool>,
    pub requires_approval: bool,
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

pub fn route_intent(input: &str) -> AgentIntent {
    let normalized = input.to_lowercase();
    if contains_any(
        &normalized,
        &["human", "handoff", "人工", "真人", "转人工", "review"],
    ) {
        AgentIntent::HumanHandoff
    } else if contains_any(
        &normalized,
        &["quiz", "exam", "test", "测验", "考试", "出题"],
    ) {
        AgentIntent::TrainingQuiz
    } else if select_tool(input).is_some() {
        AgentIntent::ToolTask
    } else if contains_any(
        &normalized,
        &["code", "repo", "github", "pull request", "issue", "代码"],
    ) {
        AgentIntent::CodeSearch
    } else if contains_any(
        &normalized,
        &[
            "?",
            "？",
            "search",
            "find",
            "knowledge",
            "handbook",
            "资料",
            "知识库",
            "什么时候",
        ],
    ) {
        AgentIntent::RagQuestion
    } else {
        AgentIntent::Chat
    }
}

pub fn select_tool(input: &str) -> Option<SelectedTool> {
    let normalized = input.to_lowercase();
    if contains_any(
        &normalized,
        &[
            "feishu",
            "飞书",
            "message",
            "notify",
            "notification",
            "reminder",
            "发送",
            "通知",
        ],
    ) {
        return Some(SelectedTool {
            code: "feishu.message.send".to_owned(),
            risk_level: 2,
            requires_approval: true,
        });
    }
    if contains_any(
        &normalized,
        &[
            "image",
            "picture",
            "poster",
            "media",
            "generate image",
            "图片",
            "海报",
        ],
    ) {
        return Some(SelectedTool {
            code: "media.image.generate".to_owned(),
            risk_level: 2,
            requires_approval: true,
        });
    }
    if contains_any(
        &normalized,
        &[
            "search",
            "find",
            "knowledge",
            "handbook",
            "rag",
            "检索",
            "搜索",
            "知识库",
        ],
    ) {
        return Some(SelectedTool {
            code: "rag.search".to_owned(),
            risk_level: 1,
            requires_approval: false,
        });
    }
    None
}

pub fn plan_react_run(input: &str, budget: TaskBudget) -> Result<AgentRunPlan, AgentPlanError> {
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
        budget,
        steps,
    })
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Agent Runtime",
        "ai-foundation",
        "Intent routing, planning, ReAct loop, tool loop, and run graph orchestration boundaries.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::{FoundationStatus, TaskBudget};

    #[test]
    fn module_describes_agent_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-agent");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }

    #[test]
    fn agent_runtime_routes_training_knowledge_tool_and_handoff_intents() {
        assert_eq!(
            route_intent("员工培训资料什么时候开始?"),
            AgentIntent::RagQuestion
        );
        assert_eq!(
            route_intent("generate a training quiz"),
            AgentIntent::TrainingQuiz
        );
        assert_eq!(
            route_intent("send a Feishu reminder"),
            AgentIntent::ToolTask
        );
        assert_eq!(
            route_intent("I need a human to review this"),
            AgentIntent::HumanHandoff
        );
    }

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
    }

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
}
