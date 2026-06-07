use novex_ai_core::{normalize_task_budget, BudgetValidationError, FoundationModule, TaskBudget};
use novex_memory::MemoryContext;
use novex_tools::{
    evaluate_tool_execution_policy, ApprovalPolicy, ToolExecutionPolicyDecision,
    ToolExecutionPolicyInput, ToolRiskLevel,
};
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
    pub policy_decision: ToolExecutionPolicyDecision,
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
            "read github",
            "github file",
            "read repo file",
            "read file",
            "读取 github",
            "读取仓库文件",
        ],
    ) {
        return Some(selected_tool(
            "github.repo.read",
            ToolRiskLevel::Low,
            ApprovalPolicy::OnRisk,
            "ai:tool:dryRun",
        ));
    }
    if contains_any(
        &normalized,
        &[
            "github",
            "repo search",
            "search repo",
            "repository search",
            "代码仓库",
            "仓库搜索",
        ],
    ) {
        return Some(selected_tool(
            "github.repo.search",
            ToolRiskLevel::Low,
            ApprovalPolicy::OnRisk,
            "ai:tool:dryRun",
        ));
    }
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
        return Some(selected_tool(
            "feishu.message.send",
            ToolRiskLevel::Medium,
            ApprovalPolicy::OnRisk,
            "ai:agent:run",
        ));
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
        return Some(selected_tool(
            "media.image.generate",
            ToolRiskLevel::Medium,
            ApprovalPolicy::OnRisk,
            "ai:tool:dryRun",
        ));
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
        return Some(selected_tool(
            "rag.search",
            ToolRiskLevel::Low,
            ApprovalPolicy::OnRisk,
            "ai:knowledge:ask",
        ));
    }
    None
}

fn selected_tool(
    code: &str,
    risk_level: ToolRiskLevel,
    approval_policy: ApprovalPolicy,
    permission_code: &str,
) -> SelectedTool {
    let decision = evaluate_tool_execution_policy(ToolExecutionPolicyInput {
        tool_code: code.to_owned(),
        risk_level,
        approval_policy,
        permission_code: Some(permission_code.to_owned()),
        auto_approved: false,
    });
    SelectedTool {
        code: code.to_owned(),
        risk_level: risk_level_value(risk_level),
        requires_approval: decision.requires_approval,
        policy_decision: decision,
    }
}

fn risk_level_value(risk_level: ToolRiskLevel) -> u8 {
    match risk_level {
        ToolRiskLevel::Low => 1,
        ToolRiskLevel::Medium => 2,
        ToolRiskLevel::High => 3,
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
    use novex_memory::{
        build_memory_context, MemoryAccessContext, MemoryScope, MemoryScopeRef, MemorySnippet,
        MemoryWritePolicy,
    };

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
}
