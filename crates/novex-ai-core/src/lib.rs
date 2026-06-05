use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-ai-core";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundationStatus {
    Skeleton,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundationModule {
    pub id: &'static str,
    pub name: &'static str,
    pub layer: &'static str,
    pub status: FoundationStatus,
    pub description: &'static str,
}

impl FoundationModule {
    pub const fn skeleton(
        id: &'static str,
        name: &'static str,
        layer: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            id,
            name,
            layer,
            status: FoundationStatus::Skeleton,
            description,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TenantContext {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub role_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRef {
    pub resource_type: String,
    pub resource_id: String,
    pub tenant_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    WaitingApproval,
    Paused,
    Resuming,
    Cancelling,
    Cancelled,
    Failed,
    Succeeded,
}

impl RunStatus {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Cancelled | Self::Failed | Self::Succeeded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunTransitionError {
    pub from: RunStatus,
    pub to: RunStatus,
}

pub fn can_transition_run_status(from: RunStatus, to: RunStatus) -> bool {
    use RunStatus::*;

    matches!(
        (from, to),
        (Queued, Running)
            | (Queued, Cancelling)
            | (Queued, Cancelled)
            | (Running, WaitingApproval)
            | (Running, Paused)
            | (Running, Cancelling)
            | (Running, Failed)
            | (Running, Succeeded)
            | (WaitingApproval, Resuming)
            | (WaitingApproval, Cancelling)
            | (WaitingApproval, Failed)
            | (Paused, Resuming)
            | (Paused, Cancelling)
            | (Paused, Failed)
            | (Resuming, Running)
            | (Resuming, Cancelling)
            | (Resuming, Failed)
            | (Cancelling, Cancelled)
    )
}

pub fn validate_run_transition(from: RunStatus, to: RunStatus) -> Result<(), RunTransitionError> {
    if can_transition_run_status(from, to) {
        Ok(())
    } else {
        Err(RunTransitionError { from, to })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStepType {
    ModelCall,
    Retrieval,
    Rerank,
    ToolCall,
    Approval,
    HumanInput,
    ConnectorSync,
    MediaJob,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PauseReason {
    Approval,
    HumanInput,
    ExternalCallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunEventKind {
    InputReceived,
    StatusChanged,
    IntentRouted,
    Thought,
    ActionSelected,
    ApprovalRequested,
    Paused,
    Resumed,
    ToolCalled,
    Observation,
    FinalOutput,
    CancelRequested,
    Cancelled,
    Error,
}

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

pub fn crate_module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "AI Core",
        "foundation",
        "Shared tenant, resource, run graph, trace, and policy contracts.",
    )
}

pub fn foundation_modules() -> Vec<FoundationModule> {
    vec![
        FoundationModule::skeleton(
            "tenant-context",
            "Tenant Context",
            "novex-ai-core",
            "Tenant and caller context passed through AI foundation modules.",
        ),
        FoundationModule::skeleton(
            "resource-ref",
            "Resource Reference",
            "novex-ai-core",
            "Stable references for tenant-scoped AI assets and run artifacts.",
        ),
        FoundationModule::skeleton(
            "run-graph",
            "Run Graph",
            "novex-ai-core",
            "Shared run, step, status, pause, cancel, replay, and event boundaries.",
        ),
        FoundationModule::skeleton(
            "trace",
            "Trace",
            "novex-ai-core",
            "Trace, cost, usage, latency, and replay metadata boundary.",
        ),
        FoundationModule::skeleton(
            "policy",
            "Policy",
            "novex-ai-core",
            "Permission, approval, network zone, and execution policy boundary.",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foundation_modules_describe_m0_skeleton_boundaries() {
        let modules = foundation_modules();

        assert!(modules.iter().any(|module| module.id == "run-graph"));
        assert!(modules.iter().any(|module| module.id == "policy"));
        assert!(modules
            .iter()
            .all(|module| module.status == FoundationStatus::Skeleton));
    }

    #[test]
    fn run_graph_status_transition_allows_approval_resume_success_path() {
        let path = [
            RunStatus::Queued,
            RunStatus::Running,
            RunStatus::WaitingApproval,
            RunStatus::Resuming,
            RunStatus::Running,
            RunStatus::Succeeded,
        ];

        for window in path.windows(2) {
            validate_run_transition(window[0], window[1]).unwrap();
        }
    }

    #[test]
    fn run_graph_status_transition_allows_cancel_from_active_states() {
        for status in [
            RunStatus::Queued,
            RunStatus::Running,
            RunStatus::WaitingApproval,
            RunStatus::Paused,
            RunStatus::Resuming,
        ] {
            validate_run_transition(status, RunStatus::Cancelling).unwrap();
            validate_run_transition(RunStatus::Cancelling, RunStatus::Cancelled).unwrap();
        }
    }

    #[test]
    fn run_graph_status_transition_rejects_terminal_restart() {
        let err = validate_run_transition(RunStatus::Succeeded, RunStatus::Running).unwrap_err();

        assert_eq!(err.from, RunStatus::Succeeded);
        assert_eq!(err.to, RunStatus::Running);
        assert!(RunStatus::Succeeded.is_terminal());
    }

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
}
