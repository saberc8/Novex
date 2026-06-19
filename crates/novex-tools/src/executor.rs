use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutorKind {
    Builtin,
    Connector,
    Mcp,
    Model,
    Http,
    Sandbox,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutorBinding {
    pub tool_code: String,
    pub executor_code: String,
    pub kind: ToolExecutorKind,
    pub supports_background_tasks: bool,
    pub waits_for_runtime_cancellation: bool,
}

impl ToolExecutorBinding {
    pub fn new(
        tool_code: impl Into<String>,
        executor_code: impl Into<String>,
        kind: ToolExecutorKind,
    ) -> Self {
        Self {
            tool_code: tool_code.into().trim().to_owned(),
            executor_code: executor_code.into().trim().to_owned(),
            kind,
            supports_background_tasks: false,
            waits_for_runtime_cancellation: false,
        }
    }

    pub fn with_background_tasks(mut self) -> Self {
        self.supports_background_tasks = true;
        self
    }

    pub fn waits_for_runtime_cancellation(mut self) -> Self {
        self.waits_for_runtime_cancellation = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutorDispatchPlan {
    pub tool_code: String,
    pub executor_code: String,
    pub kind: ToolExecutorKind,
    pub requires_connector_credential: bool,
    pub requires_mcp_tool: bool,
    pub requires_model_runtime: bool,
    pub supports_background_tasks: bool,
    pub waits_for_runtime_cancellation: bool,
}

impl ToolExecutorDispatchPlan {
    pub fn from_binding(binding: &ToolExecutorBinding) -> Self {
        Self {
            tool_code: binding.tool_code.trim().to_owned(),
            executor_code: binding.executor_code.trim().to_owned(),
            kind: binding.kind,
            requires_connector_credential: matches!(binding.kind, ToolExecutorKind::Connector),
            requires_mcp_tool: matches!(binding.kind, ToolExecutorKind::Mcp),
            requires_model_runtime: matches!(binding.kind, ToolExecutorKind::Model),
            supports_background_tasks: binding.supports_background_tasks,
            waits_for_runtime_cancellation: binding.waits_for_runtime_cancellation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutorRegistryErrorKind {
    EmptyToolCode,
    EmptyExecutorCode,
    DuplicateToolCode,
    MissingExecutor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutorRegistryError {
    pub kind: ToolExecutorRegistryErrorKind,
    pub tool_code: Option<String>,
    pub executor_code: Option<String>,
    pub message: String,
}

impl ToolExecutorRegistryError {
    fn empty_tool_code() -> Self {
        Self {
            kind: ToolExecutorRegistryErrorKind::EmptyToolCode,
            tool_code: None,
            executor_code: None,
            message: "tool code is empty".to_owned(),
        }
    }

    fn empty_executor_code(tool_code: impl Into<String>) -> Self {
        let tool_code = tool_code.into();
        Self {
            kind: ToolExecutorRegistryErrorKind::EmptyExecutorCode,
            tool_code: Some(tool_code.clone()),
            executor_code: None,
            message: format!("executor code is empty for tool `{tool_code}`"),
        }
    }

    fn duplicate_tool_code(tool_code: impl Into<String>) -> Self {
        let tool_code = tool_code.into();
        Self {
            kind: ToolExecutorRegistryErrorKind::DuplicateToolCode,
            tool_code: Some(tool_code.clone()),
            executor_code: None,
            message: format!("duplicate executor binding for tool `{tool_code}`"),
        }
    }

    fn missing_executor(tool_code: impl Into<String>) -> Self {
        let tool_code = tool_code.into();
        Self {
            kind: ToolExecutorRegistryErrorKind::MissingExecutor,
            tool_code: Some(tool_code.clone()),
            executor_code: None,
            message: format!("tool `{tool_code}` has no registered executor"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutorRegistry {
    bindings: BTreeMap<String, ToolExecutorBinding>,
}

impl ToolExecutorRegistry {
    pub fn from_bindings(
        bindings: impl IntoIterator<Item = ToolExecutorBinding>,
    ) -> Result<Self, ToolExecutorRegistryError> {
        let mut registry = BTreeMap::new();
        for mut binding in bindings {
            binding.tool_code = binding.tool_code.trim().to_owned();
            binding.executor_code = binding.executor_code.trim().to_owned();
            if binding.tool_code.is_empty() {
                return Err(ToolExecutorRegistryError::empty_tool_code());
            }
            if binding.executor_code.is_empty() {
                return Err(ToolExecutorRegistryError::empty_executor_code(
                    binding.tool_code,
                ));
            }
            if registry.contains_key(&binding.tool_code) {
                return Err(ToolExecutorRegistryError::duplicate_tool_code(
                    binding.tool_code,
                ));
            }
            registry.insert(binding.tool_code.clone(), binding);
        }
        Ok(Self { bindings: registry })
    }

    pub fn tool_codes(&self) -> Vec<String> {
        self.bindings.keys().cloned().collect()
    }

    pub fn executor_for(
        &self,
        tool_code: impl AsRef<str>,
    ) -> Result<&ToolExecutorBinding, ToolExecutorRegistryError> {
        let tool_code = tool_code.as_ref().trim();
        if tool_code.is_empty() {
            return Err(ToolExecutorRegistryError::empty_tool_code());
        }
        self.bindings
            .get(tool_code)
            .ok_or_else(|| ToolExecutorRegistryError::missing_executor(tool_code))
    }
}
