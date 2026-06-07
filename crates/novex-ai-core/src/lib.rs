use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};
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

pub const INTEGRATION_QPS_RESOURCE: &str = "external_integration.qps";
pub const INTEGRATION_QUOTA_RESOURCE: &str = "external_integration.quota";
pub const INTEGRATION_USAGE_UNIT: &str = "request";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationPrincipalType {
    ApiKey,
    PublicLink,
}

impl IntegrationPrincipalType {
    pub const fn scope_type(self) -> &'static str {
        match self {
            Self::ApiKey => "api_key",
            Self::PublicLink => "public_link",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationUsageSubject {
    pub tenant_id: i64,
    pub scope_type: String,
    pub scope_id: String,
    pub qps_limit: i32,
    pub quota_limit: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationUsageWindow {
    pub resource_type: String,
    pub usage_unit: String,
    pub window_start: NaiveDateTime,
    pub window_end: NaiveDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationUsageLimitError {
    InvalidLimit,
    QpsExceeded,
    QuotaExceeded,
}

pub fn build_integration_usage_subject(
    principal_type: IntegrationPrincipalType,
    tenant_id: i64,
    credential_id: impl Into<String>,
    qps_limit: i32,
    quota_limit: i64,
) -> Result<IntegrationUsageSubject, IntegrationUsageLimitError> {
    if qps_limit <= 0 || quota_limit <= 0 {
        return Err(IntegrationUsageLimitError::InvalidLimit);
    }

    Ok(IntegrationUsageSubject {
        tenant_id,
        scope_type: principal_type.scope_type().to_owned(),
        scope_id: credential_id.into(),
        qps_limit,
        quota_limit,
    })
}

pub fn integration_usage_windows(now: NaiveDateTime) -> Vec<IntegrationUsageWindow> {
    let second_start = now
        .with_nanosecond(0)
        .expect("zero nanosecond is valid for NaiveDateTime");
    let month_start = NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
        .expect("current year and month form a valid date")
        .and_hms_opt(0, 0, 0)
        .expect("midnight is a valid time");
    let (next_year, next_month) = if now.month() == 12 {
        (now.year() + 1, 1)
    } else {
        (now.year(), now.month() + 1)
    };
    let month_end = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .expect("next year and month form a valid date")
        .and_hms_opt(0, 0, 0)
        .expect("midnight is a valid time");

    vec![
        IntegrationUsageWindow {
            resource_type: INTEGRATION_QPS_RESOURCE.to_owned(),
            usage_unit: INTEGRATION_USAGE_UNIT.to_owned(),
            window_start: second_start,
            window_end: second_start + Duration::seconds(1),
        },
        IntegrationUsageWindow {
            resource_type: INTEGRATION_QUOTA_RESOURCE.to_owned(),
            usage_unit: INTEGRATION_USAGE_UNIT.to_owned(),
            window_start: month_start,
            window_end: month_end,
        },
    ]
}

pub fn enforce_integration_usage_limits(
    subject: &IntegrationUsageSubject,
    qps_usage: i64,
    quota_usage: i64,
) -> Result<(), IntegrationUsageLimitError> {
    if subject.qps_limit <= 0 || subject.quota_limit <= 0 {
        return Err(IntegrationUsageLimitError::InvalidLimit);
    }
    if qps_usage > i64::from(subject.qps_limit) {
        return Err(IntegrationUsageLimitError::QpsExceeded);
    }
    if quota_usage > subject.quota_limit {
        return Err(IntegrationUsageLimitError::QuotaExceeded);
    }
    Ok(())
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
    Retrieval,
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

    #[test]
    fn integration_usage_subject_binds_principal_to_meter_scope() {
        assert_eq!(
            build_integration_usage_subject(IntegrationPrincipalType::ApiKey, 11, "42", 2, 5)
                .unwrap(),
            IntegrationUsageSubject {
                tenant_id: 11,
                scope_type: "api_key".to_owned(),
                scope_id: "42".to_owned(),
                qps_limit: 2,
                quota_limit: 5,
            }
        );
        assert_eq!(
            build_integration_usage_subject(IntegrationPrincipalType::PublicLink, 11, "43", 3, 8)
                .unwrap()
                .scope_type,
            "public_link"
        );
    }

    #[test]
    fn integration_usage_windows_cover_second_qps_and_monthly_quota() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-06-06T08:09:10Z")
            .unwrap()
            .naive_utc();
        let windows = integration_usage_windows(now);

        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].resource_type, INTEGRATION_QPS_RESOURCE);
        assert_eq!(windows[0].usage_unit, INTEGRATION_USAGE_UNIT);
        assert_eq!(windows[0].window_start, now);
        assert_eq!(
            windows[0].window_end,
            chrono::DateTime::parse_from_rfc3339("2026-06-06T08:09:11Z")
                .unwrap()
                .naive_utc()
        );
        assert_eq!(windows[1].resource_type, INTEGRATION_QUOTA_RESOURCE);
        assert_eq!(
            windows[1].window_start,
            chrono::DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z")
                .unwrap()
                .naive_utc()
        );
        assert_eq!(
            windows[1].window_end,
            chrono::DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
                .unwrap()
                .naive_utc()
        );
    }

    #[test]
    fn integration_usage_limits_allow_boundary_and_reject_excess() {
        let subject = IntegrationUsageSubject {
            tenant_id: 11,
            scope_type: "api_key".to_owned(),
            scope_id: "42".to_owned(),
            qps_limit: 2,
            quota_limit: 5,
        };

        assert!(enforce_integration_usage_limits(&subject, 2, 5).is_ok());
        assert_eq!(
            enforce_integration_usage_limits(&subject, 3, 5).unwrap_err(),
            IntegrationUsageLimitError::QpsExceeded
        );
        assert_eq!(
            enforce_integration_usage_limits(&subject, 2, 6).unwrap_err(),
            IntegrationUsageLimitError::QuotaExceeded
        );
    }
}
