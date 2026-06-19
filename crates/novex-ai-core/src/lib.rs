mod budget;
mod context;
mod integration_usage;
mod module;
mod run_graph;

pub use budget::{
    normalize_task_budget, BudgetValidationError, TaskBudget, DEFAULT_MAX_COST_CENTS,
    DEFAULT_MAX_SECONDS, DEFAULT_MAX_STEPS, DEFAULT_MAX_TOOL_CALLS, POC_MAX_COST_CENTS,
    POC_MAX_SECONDS, POC_MAX_STEPS, POC_MAX_TOOL_CALLS,
};
pub use context::{ResourceRef, TenantContext};
pub use integration_usage::{
    build_integration_usage_subject, enforce_integration_usage_limits, integration_usage_windows,
    IntegrationPrincipalType, IntegrationUsageLimitError, IntegrationUsageSubject,
    IntegrationUsageWindow, INTEGRATION_QPS_RESOURCE, INTEGRATION_QUOTA_RESOURCE,
    INTEGRATION_USAGE_UNIT,
};
pub use module::{crate_module, foundation_modules, FoundationModule, FoundationStatus};
pub use run_graph::{
    can_transition_run_status, validate_run_transition, PauseReason, RunEventKind, RunStatus,
    RunStepType, RunTransitionError,
};

pub const CRATE_ID: &str = "novex-ai-core";
