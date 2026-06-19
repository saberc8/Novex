mod adapters;
mod concurrency;
mod definitions;
mod executor;
mod media;
mod policy;
mod router;
mod types;

use novex_ai_core::FoundationModule;

pub use adapters::{
    feishu_message_text_from_tool_input, github_read_request_from_tool_input,
    github_search_request_from_tool_input, media_image_request_from_tool_input,
};
pub use concurrency::{
    ToolBatchExecutionMode, ToolBatchPlan, ToolConcurrencyPolicy, ToolExecutionLock,
};
pub use definitions::{
    agent_model_loop_tool_definitions, agent_model_loop_tool_executor_bindings,
    customer_service_tool_definitions,
};
pub use executor::{
    ToolExecutorBinding, ToolExecutorDispatchPlan, ToolExecutorKind, ToolExecutorRegistry,
    ToolExecutorRegistryError, ToolExecutorRegistryErrorKind,
};
pub use media::{
    parse_media_image_generation_response, MediaImageGenerationRequest, MediaImageGenerationResult,
};
pub use policy::{approval_policy_code, evaluate_tool_execution_policy, tool_risk_code};
pub use router::{RoutedToolCall, ToolRouteError, ToolRouteErrorKind, ToolRouter};
pub use types::{
    AgentToolExecution, ApprovalPolicy, ModelToolSpec, ToolDefinition, ToolExecutionPolicyDecision,
    ToolExecutionPolicyInput, ToolKind, ToolRiskLevel,
};

pub const CRATE_ID: &str = "novex-tools";

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Tool Registry",
        "ai-foundation",
        "Tool schema, risk, permissions, approval, executor, audit, and replay boundaries.",
    )
}
