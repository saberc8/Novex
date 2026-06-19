use novex_tools::{ApprovalPolicy, ToolConcurrencyPolicy, ToolDefinition, ToolRiskLevel};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tool_code::mcp_tool_code;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpServerStatus {
    Registered,
    Discovering,
    Connected,
    Degraded,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransportKind {
    Builtin,
    Stdio,
    Sse,
    StreamableHttp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpAuthScope {
    Tenant,
    User,
    App,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpAuthType {
    None,
    BearerEnv,
    OAuth,
    Headers,
}

impl McpAuthType {
    pub fn requires_secret(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDescriptor {
    pub server_id: String,
    pub tool_name: String,
    pub permission_code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpDiscoveredTool {
    pub server_code: String,
    pub tool_name: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub risk_level: ToolRiskLevel,
}

impl McpDiscoveredTool {
    pub fn to_tool_definition(&self, permission_code: impl Into<String>) -> ToolDefinition {
        ToolDefinition {
            code: mcp_tool_code(&self.server_code, &self.tool_name),
            name: format!("{}.{}", self.server_code.trim(), self.tool_name.trim()),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            output_schema: self.output_schema.clone(),
            risk_level: self.risk_level,
            approval_policy: ApprovalPolicy::OnRisk,
            permission_code: Some(permission_code.into()),
            concurrency: match self.risk_level {
                ToolRiskLevel::Low => ToolConcurrencyPolicy::shared(),
                ToolRiskLevel::Medium | ToolRiskLevel::High => {
                    ToolConcurrencyPolicy::exclusive(format!("mcp:{}", self.server_code.trim()))
                }
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolInvocationRequest {
    pub server_code: String,
    pub tool_name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolInvocationResult {
    pub tool_code: String,
    pub status: String,
    pub output: Value,
    pub dry_run: bool,
}
