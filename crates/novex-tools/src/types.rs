use crate::concurrency::ToolConcurrencyPolicy;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Function,
    Http,
    Connector,
    Mcp,
    Sandbox,
    Model,
    Media,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    Never,
    OnRisk,
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionPolicyInput {
    pub tool_code: String,
    pub risk_level: ToolRiskLevel,
    pub approval_policy: ApprovalPolicy,
    pub permission_code: Option<String>,
    pub auto_approved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionPolicyDecision {
    pub tool_code: String,
    pub risk_level: ToolRiskLevel,
    pub permission_code: Option<String>,
    pub requires_approval: bool,
    pub can_execute: bool,
    pub pause_reason: Option<String>,
    pub policy_reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolExecution {
    pub response_payload: Value,
    pub status: String,
    pub dry_run: bool,
    pub error_message: Option<String>,
    pub final_output: String,
}

impl AgentToolExecution {
    pub fn succeeded(response_payload: Value, dry_run: bool, final_output: String) -> Self {
        Self {
            response_payload,
            status: "succeeded".to_owned(),
            dry_run,
            error_message: None,
            final_output,
        }
    }

    pub fn failed(response_payload: Value, error_message: String, final_output: String) -> Self {
        Self {
            response_payload,
            status: "failed".to_owned(),
            dry_run: false,
            error_message: Some(error_message),
            final_output,
        }
    }

    pub fn cancelled(response_payload: Value, final_output: String) -> Self {
        Self {
            response_payload,
            status: "cancelled".to_owned(),
            dry_run: false,
            error_message: Some(final_output.clone()),
            final_output,
        }
    }

    pub fn succeeded_status(&self) -> bool {
        self.status == "succeeded"
    }

    pub fn cancelled_status(&self) -> bool {
        self.status == "cancelled"
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub code: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub risk_level: ToolRiskLevel,
    pub approval_policy: ApprovalPolicy,
    pub permission_code: Option<String>,
    #[serde(default)]
    pub concurrency: ToolConcurrencyPolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub output_schema: Option<Value>,
    pub metadata: Value,
}
