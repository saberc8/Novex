use std::collections::BTreeMap;

use crate::concurrency::ToolConcurrencyPolicy;
use crate::policy::{approval_policy_code, tool_risk_code};
use crate::types::{ModelToolSpec, ToolDefinition};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRouteErrorKind {
    EmptyToolCode,
    DuplicateToolCode,
    UnknownTool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRouteError {
    pub kind: ToolRouteErrorKind,
    pub tool_code: Option<String>,
    pub message: String,
}

impl ToolRouteError {
    fn empty_tool_code() -> Self {
        Self {
            kind: ToolRouteErrorKind::EmptyToolCode,
            tool_code: None,
            message: "tool code is empty".to_owned(),
        }
    }

    fn duplicate_tool_code(code: impl Into<String>) -> Self {
        let code = code.into();
        Self {
            kind: ToolRouteErrorKind::DuplicateToolCode,
            tool_code: Some(code.clone()),
            message: format!("duplicate tool code `{code}`"),
        }
    }

    fn unknown_tool(code: impl Into<String>) -> Self {
        let code = code.into();
        Self {
            kind: ToolRouteErrorKind::UnknownTool,
            tool_code: Some(code.clone()),
            message: format!("model requested unregistered tool `{code}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutedToolCall {
    pub call_id: String,
    pub tool: ToolDefinition,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRouter {
    tools: BTreeMap<String, ToolDefinition>,
}

impl ToolRouter {
    pub fn from_definitions(
        definitions: impl IntoIterator<Item = ToolDefinition>,
    ) -> Result<Self, ToolRouteError> {
        let mut tools = BTreeMap::new();
        for mut definition in definitions {
            let code = definition.code.trim().to_owned();
            if code.is_empty() {
                return Err(ToolRouteError::empty_tool_code());
            }
            if tools.contains_key(&code) {
                return Err(ToolRouteError::duplicate_tool_code(code));
            }
            definition.code = code.clone();
            tools.insert(code, definition);
        }
        Ok(Self { tools })
    }

    pub fn tool_codes(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub fn model_tool_specs(&self) -> Vec<ModelToolSpec> {
        self.tools
            .values()
            .map(ToolDefinition::to_model_tool_spec)
            .collect()
    }

    pub fn tool_concurrency_policy(&self, tool_code: &str) -> Option<&ToolConcurrencyPolicy> {
        self.tools
            .get(tool_code.trim())
            .map(|tool| &tool.concurrency)
    }

    pub fn route_tool_call(
        &self,
        call_id: impl Into<String>,
        tool_code: impl AsRef<str>,
        arguments: Value,
    ) -> Result<RoutedToolCall, ToolRouteError> {
        let tool_code = tool_code.as_ref().trim();
        if tool_code.is_empty() {
            return Err(ToolRouteError::empty_tool_code());
        }
        let Some(tool) = self.tools.get(tool_code) else {
            return Err(ToolRouteError::unknown_tool(tool_code));
        };
        Ok(RoutedToolCall {
            call_id: call_id.into(),
            tool: tool.clone(),
            arguments,
        })
    }
}

impl ToolDefinition {
    pub fn to_model_tool_spec(&self) -> ModelToolSpec {
        ModelToolSpec {
            name: self.code.clone(),
            description: self.description.clone(),
            parameters: self.input_schema.clone(),
            output_schema: self.output_schema.clone(),
            metadata: serde_json::json!({
                "displayName": self.name,
                "riskLevel": tool_risk_code(self.risk_level),
                "approvalPolicy": approval_policy_code(self.approval_policy),
                "permissionCode": self.permission_code,
                "concurrencyPolicy": self.concurrency,
            }),
        }
    }
}
