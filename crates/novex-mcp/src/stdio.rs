use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::json_rpc::{McpJsonRpcNotification, McpJsonRpcRequest};
use crate::types::McpToolInvocationRequest;
use crate::{
    MCP_STDIO_DEFAULT_SHUTDOWN_TIMEOUT_MS, MCP_STDIO_DEFAULT_STARTUP_TIMEOUT_MS,
    MCP_STDIO_MAX_TIMEOUT_MS, MCP_STDIO_MIN_TIMEOUT_MS,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum McpStdioEnvValue {
    Literal(String),
    SecretRef(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStdioLifecyclePolicy {
    pub startup_timeout_ms: u64,
    pub shutdown_timeout_ms: u64,
}

impl McpStdioLifecyclePolicy {
    pub fn new(
        startup_timeout_ms: u64,
        shutdown_timeout_ms: u64,
    ) -> Result<Self, McpStdioLaunchError> {
        validate_stdio_timeout("startup_timeout_ms", startup_timeout_ms)?;
        validate_stdio_timeout("shutdown_timeout_ms", shutdown_timeout_ms)?;
        Ok(Self {
            startup_timeout_ms,
            shutdown_timeout_ms,
        })
    }
}

impl Default for McpStdioLifecyclePolicy {
    fn default() -> Self {
        Self {
            startup_timeout_ms: MCP_STDIO_DEFAULT_STARTUP_TIMEOUT_MS,
            shutdown_timeout_ms: MCP_STDIO_DEFAULT_SHUTDOWN_TIMEOUT_MS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpStdioLifecyclePhase {
    Spawn,
    Initialize,
    ListTools,
    CallTools,
    Shutdown,
}

impl McpStdioLifecyclePhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Spawn => "spawn",
            Self::Initialize => "initialize",
            Self::ListTools => "list_tools",
            Self::CallTools => "call_tools",
            Self::Shutdown => "shutdown",
        }
    }
}

const MCP_STDIO_LIFECYCLE_PHASES: [McpStdioLifecyclePhase; 5] = [
    McpStdioLifecyclePhase::Spawn,
    McpStdioLifecyclePhase::Initialize,
    McpStdioLifecyclePhase::ListTools,
    McpStdioLifecyclePhase::CallTools,
    McpStdioLifecyclePhase::Shutdown,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStdioLaunchConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, McpStdioEnvValue>,
    pub working_dir: Option<String>,
    pub lifecycle_policy: McpStdioLifecyclePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStdioLaunchPlan {
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, McpStdioEnvValue>,
    pub working_dir: Option<String>,
    pub lifecycle_policy: McpStdioLifecyclePolicy,
}

impl McpStdioLaunchPlan {
    pub fn new(config: McpStdioLaunchConfig) -> Result<Self, McpStdioLaunchError> {
        let command = config.command.trim();
        if command.is_empty() {
            return Err(McpStdioLaunchError::new(
                "command",
                "MCP stdio command is required",
            ));
        }

        let mut env = BTreeMap::new();
        for (name, value) in config.env {
            let name = normalize_stdio_env_name(name)?;
            let value = normalize_stdio_env_value(&name, value)?;
            env.insert(name, value);
        }

        Ok(Self {
            command: command.to_owned(),
            args: config.args,
            env,
            working_dir: config
                .working_dir
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            lifecycle_policy: config.lifecycle_policy,
        })
    }

    pub fn lifecycle_phases(&self) -> Vec<McpStdioLifecyclePhase> {
        MCP_STDIO_LIFECYCLE_PHASES.to_vec()
    }

    pub fn sanitized_evidence(&self) -> Value {
        let mut env = serde_json::Map::new();
        for (name, value) in &self.env {
            let evidence = match value {
                McpStdioEnvValue::Literal(_) => json!({
                    "kind": "literal",
                }),
                McpStdioEnvValue::SecretRef(secret_ref) => json!({
                    "kind": "secret_ref",
                    "secretRef": secret_ref,
                }),
            };
            env.insert(name.clone(), evidence);
        }
        let lifecycle_phases = self
            .lifecycle_phases()
            .iter()
            .map(|phase| phase.as_str())
            .collect::<Vec<_>>();

        json!({
            "command": self.command,
            "args": self.args,
            "env": Value::Object(env),
            "workingDir": self.working_dir,
            "lifecyclePolicy": self.lifecycle_policy,
            "lifecyclePhases": lifecycle_phases,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStdioToolCallPlan {
    pub launch: McpStdioLaunchPlan,
    pub initialize: Value,
    pub initialized: Value,
    pub tools_call: Value,
}

impl McpStdioToolCallPlan {
    pub fn new(
        launch: McpStdioLaunchPlan,
        request_id: impl Into<String>,
        request: &McpToolInvocationRequest,
    ) -> Self {
        let request_id = request_id.into();
        Self {
            launch,
            initialize: McpJsonRpcRequest::initialize(format!("{request_id}-initialize"))
                .into_value(),
            initialized: McpJsonRpcNotification::initialized().into_value(),
            tools_call: McpJsonRpcRequest::tools_call(request_id, request).into_value(),
        }
    }

    pub fn sanitized_evidence(&self) -> Value {
        json!({
            "transportKind": "stdio",
            "launch": self.launch.sanitized_evidence(),
            "initialize": self.initialize,
            "initialized": self.initialized,
            "request": self.tools_call,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStdioLaunchError {
    pub field: String,
    pub message: String,
}

impl McpStdioLaunchError {
    fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

fn validate_stdio_timeout(field: &str, timeout_ms: u64) -> Result<(), McpStdioLaunchError> {
    if !(MCP_STDIO_MIN_TIMEOUT_MS..=MCP_STDIO_MAX_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(McpStdioLaunchError::new(
            field,
            format!(
                "MCP stdio timeout must be between {MCP_STDIO_MIN_TIMEOUT_MS} and {MCP_STDIO_MAX_TIMEOUT_MS} ms"
            ),
        ));
    }
    Ok(())
}

fn normalize_stdio_env_name(name: String) -> Result<String, McpStdioLaunchError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(McpStdioLaunchError::new(
            "env",
            "MCP stdio env name is required",
        ));
    }
    let mut chars = name.chars();
    let starts_valid = chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic());
    let rest_valid = chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric());
    if !starts_valid || !rest_valid {
        return Err(McpStdioLaunchError::new(
            format!("env.{name}"),
            "MCP stdio env name must contain only ASCII letters, digits, and underscores, and must not start with a digit",
        ));
    }
    Ok(name.to_owned())
}

fn normalize_stdio_env_value(
    name: &str,
    value: McpStdioEnvValue,
) -> Result<McpStdioEnvValue, McpStdioLaunchError> {
    match value {
        McpStdioEnvValue::Literal(value) => Ok(McpStdioEnvValue::Literal(value)),
        McpStdioEnvValue::SecretRef(secret_ref) => {
            let secret_ref = secret_ref.trim();
            if secret_ref.is_empty() || !secret_ref.starts_with("env:") {
                return Err(McpStdioLaunchError::new(
                    format!("env.{name}.secret_ref"),
                    "MCP stdio env secretRef must use env: prefix",
                ));
            }
            Ok(McpStdioEnvValue::SecretRef(secret_ref.to_owned()))
        }
    }
}
