use std::collections::BTreeMap;

use novex_ai_core::FoundationModule;
use novex_tools::{ApprovalPolicy, ToolConcurrencyPolicy, ToolDefinition, ToolRiskLevel};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use url::Url;

pub const CRATE_ID: &str = "novex-mcp";
pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
pub const MCP_STDIO_MIN_TIMEOUT_MS: u64 = 100;
pub const MCP_STDIO_MAX_TIMEOUT_MS: u64 = 60_000;
pub const MCP_STDIO_DEFAULT_STARTUP_TIMEOUT_MS: u64 = 10_000;
pub const MCP_STDIO_DEFAULT_SHUTDOWN_TIMEOUT_MS: u64 = 5_000;

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

pub fn mcp_tool_code(server_code: &str, tool_name: &str) -> String {
    format!(
        "mcp.{}.{}",
        normalize_mcp_code_segment(server_code),
        normalize_mcp_code_segment(tool_name)
    )
}

fn normalize_mcp_code_segment(value: &str) -> String {
    let normalized: String = value
        .trim()
        .chars()
        .map(|ch| match ch {
            ch if ch.is_ascii_alphanumeric() => ch.to_ascii_lowercase(),
            '.' | '_' => ch,
            '-' | '/' | ':' | ' ' => '_',
            _ => '_',
        })
        .collect();
    let collapsed = normalized
        .split('_')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if collapsed.is_empty() {
        "unknown".to_owned()
    } else {
        collapsed
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpJsonRpcRequest {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    pub params: Value,
}

impl McpJsonRpcRequest {
    pub fn tools_call(id: impl Into<String>, request: &McpToolInvocationRequest) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id: id.into(),
            method: "tools/call".to_owned(),
            params: json!({
                "name": request.tool_name,
                "arguments": request.arguments,
            }),
        }
    }

    pub fn into_value(self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| Value::Null)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStreamableHttpRequestPlan {
    pub endpoint_url: String,
    pub http_method: String,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
    pub secret_ref: Option<String>,
}

impl McpStreamableHttpRequestPlan {
    pub fn tools_call(
        endpoint_url: impl Into<String>,
        request_id: impl Into<String>,
        request: &McpToolInvocationRequest,
        secret_ref: Option<&str>,
    ) -> Self {
        let mut headers = BTreeMap::new();
        headers.insert(
            "Accept".to_owned(),
            "application/json, text/event-stream".to_owned(),
        );
        headers.insert("Content-Type".to_owned(), "application/json".to_owned());
        headers.insert(
            "MCP-Protocol-Version".to_owned(),
            MCP_PROTOCOL_VERSION.to_owned(),
        );

        Self {
            endpoint_url: endpoint_url.into(),
            http_method: "POST".to_owned(),
            headers,
            body: McpJsonRpcRequest::tools_call(request_id, request).into_value(),
            secret_ref: secret_ref.map(ToOwned::to_owned),
        }
    }

    pub fn header_value(&self, name: &str) -> Option<String> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.clone())
    }

    pub fn sanitized_evidence(&self) -> Value {
        json!({
            "endpointUrl": self.endpoint_url,
            "httpMethod": self.http_method,
            "headers": self.headers,
            "body": self.body,
            "secretRef": self.secret_ref,
        })
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStreamableHttpResponse {
    pub http_status: u16,
    pub content_type: String,
    pub body: String,
}

impl McpStreamableHttpResponse {
    pub fn new(http_status: u16, content_type: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            http_status,
            content_type: content_type.into(),
            body: body.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpClientErrorKind {
    HttpStatus,
    UnsupportedContentType,
    JsonRpcError,
    MalformedJson,
    MissingResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClientError {
    pub kind: McpClientErrorKind,
    pub message: String,
    pub http_status: Option<u16>,
    pub rpc_code: Option<i64>,
}

impl McpClientError {
    fn new(kind: McpClientErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            http_status: None,
            rpc_code: None,
        }
    }

    fn with_http_status(mut self, http_status: u16) -> Self {
        self.http_status = Some(http_status);
        self
    }

    fn with_rpc_code(mut self, rpc_code: i64) -> Self {
        self.rpc_code = Some(rpc_code);
        self
    }
}

pub fn parse_mcp_tool_call_response(
    tool_code: impl Into<String>,
    response: &McpStreamableHttpResponse,
) -> Result<McpToolInvocationResult, McpClientError> {
    let tool_code = tool_code.into();
    if response.http_status >= 400 {
        return Err(McpClientError::new(
            McpClientErrorKind::HttpStatus,
            format!("MCP server returned HTTP {}", response.http_status),
        )
        .with_http_status(response.http_status));
    }

    let content_type = response
        .content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let payload = match content_type.as_str() {
        "application/json" => parse_mcp_json_payload(&response.body)?,
        "text/event-stream" => parse_mcp_sse_payload(&response.body)?,
        _ => {
            return Err(McpClientError::new(
                McpClientErrorKind::UnsupportedContentType,
                format!("Unsupported MCP content type `{}`", response.content_type),
            ));
        }
    };

    mcp_tool_result_from_json_rpc(tool_code, payload)
}

fn parse_mcp_json_payload(body: &str) -> Result<Value, McpClientError> {
    serde_json::from_str(body).map_err(|error| {
        McpClientError::new(
            McpClientErrorKind::MalformedJson,
            format!("MCP JSON response is invalid: {error}"),
        )
    })
}

fn parse_mcp_sse_payload(body: &str) -> Result<Value, McpClientError> {
    for event in body.split("\n\n") {
        let data = event
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim)
            .filter(|line| !line.is_empty() && *line != "[DONE]")
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() {
            continue;
        }
        return parse_mcp_json_payload(&data);
    }
    Err(McpClientError::new(
        McpClientErrorKind::MissingResult,
        "MCP event stream did not include a JSON-RPC data message",
    ))
}

fn mcp_tool_result_from_json_rpc(
    tool_code: String,
    payload: Value,
) -> Result<McpToolInvocationResult, McpClientError> {
    if let Some(error) = payload.get("error") {
        let code = error
            .get("code")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("MCP JSON-RPC error");
        return Err(
            McpClientError::new(McpClientErrorKind::JsonRpcError, message.to_owned())
                .with_rpc_code(code),
        );
    }

    let result = payload.get("result").cloned().ok_or_else(|| {
        McpClientError::new(
            McpClientErrorKind::MissingResult,
            "MCP JSON-RPC response missing result",
        )
    })?;
    let is_error = result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(McpToolInvocationResult {
        tool_code,
        status: if is_error { "failed" } else { "succeeded" }.to_owned(),
        output: result,
        dry_run: false,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRegistrationPolicy {
    pub server_code: String,
    pub endpoint_url: Option<String>,
    pub transport_kind: McpTransportKind,
    pub auth_scope: McpAuthScope,
    pub auth_type: McpAuthType,
    pub secret_ref: Option<String>,
    pub network_allowlist: Vec<String>,
    pub tool_allowlist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpDiscoveryPlan {
    pub server_code: String,
    pub endpoint_url: Option<String>,
    pub transport_kind: McpTransportKind,
    pub auth_scope: McpAuthScope,
    pub allowed_tools: Vec<String>,
    pub status: McpServerStatus,
    pub requires_secret: bool,
    pub audit_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRegistrationError {
    pub field: String,
    pub message: String,
}

impl McpRegistrationError {
    fn new(field: &str, message: impl Into<String>) -> Self {
        Self {
            field: field.to_owned(),
            message: message.into(),
        }
    }
}

pub fn validate_mcp_registration_policy(
    policy: &McpRegistrationPolicy,
) -> Result<McpDiscoveryPlan, McpRegistrationError> {
    let server_code = policy.server_code.trim();
    if server_code.is_empty() {
        return Err(McpRegistrationError::new(
            "server_code",
            "MCP server code is required",
        ));
    }
    if policy.auth_type.requires_secret() {
        let secret_ref = policy
            .secret_ref
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                McpRegistrationError::new("secret_ref", "MCP server secretRef is required")
            })?;
        if !secret_ref.starts_with("env:") {
            return Err(McpRegistrationError::new(
                "secret_ref",
                "MCP server secretRef must use env: prefix",
            ));
        }
    }
    if matches!(
        policy.transport_kind,
        McpTransportKind::Sse | McpTransportKind::StreamableHttp
    ) {
        let endpoint_url = policy
            .endpoint_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                McpRegistrationError::new("endpoint_url", "MCP server endpointUrl is required")
            })?;
        ensure_endpoint_allowed(endpoint_url, &policy.network_allowlist)?;
    }

    Ok(McpDiscoveryPlan {
        server_code: server_code.to_owned(),
        endpoint_url: policy
            .endpoint_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        transport_kind: policy.transport_kind,
        auth_scope: policy.auth_scope,
        allowed_tools: policy
            .tool_allowlist
            .iter()
            .map(|tool| tool.trim())
            .filter(|tool| !tool.is_empty())
            .map(str::to_owned)
            .collect(),
        status: McpServerStatus::Discovering,
        requires_secret: policy.auth_type.requires_secret(),
        audit_required: true,
    })
}

fn ensure_endpoint_allowed(
    endpoint_url: &str,
    network_allowlist: &[String],
) -> Result<(), McpRegistrationError> {
    let endpoint = Url::parse(endpoint_url).map_err(|_| {
        McpRegistrationError::new("endpoint_url", "MCP server endpointUrl is invalid")
    })?;
    if !matches!(endpoint.scheme(), "http" | "https") {
        return Err(McpRegistrationError::new(
            "endpoint_url",
            "MCP server endpointUrl only allows http/https",
        ));
    }
    let host = endpoint
        .host_str()
        .ok_or_else(|| {
            McpRegistrationError::new("endpoint_url", "MCP server endpointUrl missing host")
        })?
        .to_ascii_lowercase();
    let host_with_port = endpoint
        .port()
        .map(|port| format!("{host}:{port}"))
        .unwrap_or_else(|| host.clone());

    let allowed = network_allowlist
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .any(|entry| {
            entry == host
                || entry == host_with_port
                || entry
                    .strip_prefix("*.")
                    .is_some_and(|suffix| host.ends_with(&format!(".{suffix}")))
                || Url::parse(&entry)
                    .ok()
                    .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
                    .is_some_and(|entry_host| entry_host == host)
        });
    if !allowed {
        return Err(McpRegistrationError::new(
            "network_allowlist",
            "MCP server endpoint host must be included in network allow-list",
        ));
    }
    Ok(())
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "MCP Gateway",
        "ai-foundation",
        "MCP server registration, tool discovery, tenant authorization, secret, and audit boundaries.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::FoundationStatus;
    use novex_tools::ToolRiskLevel;

    #[test]
    fn module_describes_mcp_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-mcp");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }

    #[test]
    fn registration_policy_builds_tenant_scoped_discovery_plan() {
        let plan = validate_mcp_registration_policy(&McpRegistrationPolicy {
            server_code: "docs.search".to_owned(),
            endpoint_url: Some("https://mcp.example.com/sse".to_owned()),
            transport_kind: McpTransportKind::StreamableHttp,
            auth_scope: McpAuthScope::Tenant,
            auth_type: McpAuthType::BearerEnv,
            secret_ref: Some("env:DOCS_MCP_TOKEN".to_owned()),
            network_allowlist: vec!["mcp.example.com".to_owned()],
            tool_allowlist: vec!["docs.search".to_owned()],
        })
        .expect("valid MCP registration should build a discovery plan");

        assert_eq!(plan.server_code, "docs.search");
        assert_eq!(plan.status, McpServerStatus::Discovering);
        assert_eq!(plan.allowed_tools, vec!["docs.search"]);
        assert!(plan.requires_secret);
        assert!(plan.audit_required);
    }

    #[test]
    fn registration_policy_rejects_endpoint_outside_network_allowlist() {
        let err = validate_mcp_registration_policy(&McpRegistrationPolicy {
            server_code: "docs.search".to_owned(),
            endpoint_url: Some("https://mcp.example.com/sse".to_owned()),
            transport_kind: McpTransportKind::StreamableHttp,
            auth_scope: McpAuthScope::Tenant,
            auth_type: McpAuthType::BearerEnv,
            secret_ref: Some("env:DOCS_MCP_TOKEN".to_owned()),
            network_allowlist: vec!["api.example.com".to_owned()],
            tool_allowlist: vec!["docs.search".to_owned()],
        })
        .unwrap_err();

        assert_eq!(err.field, "network_allowlist");
    }

    #[test]
    fn mcp_discovered_tool_converts_to_tenant_tool_definition() {
        let tool = McpDiscoveredTool {
            server_code: "docs".to_owned(),
            tool_name: "search".to_owned(),
            description: "Search docs".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string"
                    }
                }
            }),
            output_schema: Some(serde_json::json!({
                "type": "object"
            })),
            risk_level: ToolRiskLevel::Low,
        };

        let definition = tool.to_tool_definition("ai:mcp:docs:search");

        assert_eq!(definition.code, "mcp.docs.search");
        assert_eq!(
            definition.input_schema["properties"]["query"]["type"],
            "string"
        );
        assert_eq!(
            definition.permission_code.as_deref(),
            Some("ai:mcp:docs:search")
        );
    }

    #[test]
    fn mcp_streamable_http_request_plan_builds_sanitized_tools_call() {
        let request = McpToolInvocationRequest {
            server_code: "docs".to_owned(),
            tool_name: "search".to_owned(),
            arguments: serde_json::json!({"query": "codex"}),
        };

        let plan = McpStreamableHttpRequestPlan::tools_call(
            "https://mcp.example.com/mcp",
            "tool-call-1",
            &request,
            Some("env:DOCS_MCP_TOKEN"),
        );

        assert_eq!(plan.http_method, "POST");
        assert_eq!(
            plan.header_value("Accept").as_deref(),
            Some("application/json, text/event-stream")
        );
        assert_eq!(
            plan.header_value("Content-Type").as_deref(),
            Some("application/json")
        );
        assert_eq!(
            plan.header_value("MCP-Protocol-Version").as_deref(),
            Some(MCP_PROTOCOL_VERSION)
        );
        assert_eq!(plan.body["jsonrpc"], "2.0");
        assert_eq!(plan.body["method"], "tools/call");
        assert_eq!(plan.body["params"]["name"], "search");
        assert_eq!(plan.body["params"]["arguments"]["query"], "codex");
        let evidence = plan.sanitized_evidence();
        assert_eq!(evidence["secretRef"], "env:DOCS_MCP_TOKEN");
        assert!(!evidence.to_string().contains("test-token"));
    }

    #[test]
    fn mcp_streamable_http_json_response_maps_tool_result() {
        let raw = McpStreamableHttpResponse::new(
            200,
            "application/json",
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "tool-call-1",
                "result": {
                    "content": [{"type": "text", "text": "Found policy"}],
                    "structuredContent": {"hits": 1},
                    "isError": false
                }
            })
            .to_string(),
        );

        let result = parse_mcp_tool_call_response("mcp.docs.search", &raw).unwrap();

        assert_eq!(result.tool_code, "mcp.docs.search");
        assert_eq!(result.status, "succeeded");
        assert!(!result.dry_run);
        assert_eq!(result.output["structuredContent"]["hits"], 1);
        assert_eq!(result.output["content"][0]["text"], "Found policy");
    }

    #[test]
    fn mcp_streamable_http_sse_response_maps_tool_result() {
        let raw = McpStreamableHttpResponse::new(
            200,
            "text/event-stream",
            concat!(
                "event: message\n",
                "data: {\"jsonrpc\":\"2.0\",\"id\":\"tool-call-1\",\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"streamed\"}],\"isError\":false}}\n\n"
            )
            .to_owned(),
        );

        let result = parse_mcp_tool_call_response("mcp.docs.search", &raw).unwrap();

        assert_eq!(result.status, "succeeded");
        assert_eq!(result.output["content"][0]["text"], "streamed");
    }

    #[test]
    fn mcp_streamable_http_json_rpc_error_is_structured() {
        let raw = McpStreamableHttpResponse::new(
            200,
            "application/json",
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "tool-call-1",
                "error": {
                    "code": -32602,
                    "message": "Invalid arguments"
                }
            })
            .to_string(),
        );

        let err = parse_mcp_tool_call_response("mcp.docs.search", &raw).unwrap_err();

        assert_eq!(err.kind, McpClientErrorKind::JsonRpcError);
        assert_eq!(err.rpc_code, Some(-32602));
        assert!(err.message.contains("Invalid arguments"));
    }

    #[test]
    fn mcp_stdio_launch_plan_sanitizes_env_secret_refs() {
        let mut env = BTreeMap::new();
        env.insert(
            "DOCS_MCP_TOKEN".to_owned(),
            McpStdioEnvValue::SecretRef("env:DOCS_MCP_TOKEN".to_owned()),
        );
        env.insert(
            "LOG_LEVEL".to_owned(),
            McpStdioEnvValue::Literal("debug-secret-literal".to_owned()),
        );

        let plan = McpStdioLaunchPlan::new(McpStdioLaunchConfig {
            command: "npx".to_owned(),
            args: vec![
                "-y".to_owned(),
                "@modelcontextprotocol/server-filesystem".to_owned(),
            ],
            env,
            working_dir: Some("/srv/docs".to_owned()),
            lifecycle_policy: McpStdioLifecyclePolicy::new(10_000, 5_000)
                .expect("timeouts should be in range"),
        })
        .expect("stdio launch plan should be valid");

        let evidence = plan.sanitized_evidence();

        assert_eq!(evidence["command"], "npx");
        assert_eq!(evidence["args"][0], "-y");
        assert_eq!(evidence["workingDir"], "/srv/docs");
        assert_eq!(evidence["env"]["DOCS_MCP_TOKEN"]["kind"], "secret_ref");
        assert_eq!(
            evidence["env"]["DOCS_MCP_TOKEN"]["secretRef"],
            "env:DOCS_MCP_TOKEN"
        );
        assert_eq!(evidence["env"]["LOG_LEVEL"]["kind"], "literal");
        assert_eq!(evidence["lifecyclePolicy"]["startupTimeoutMs"], 10_000);
        assert_eq!(evidence["lifecyclePolicy"]["shutdownTimeoutMs"], 5_000);
        assert!(!evidence.to_string().contains("debug-secret-literal"));
    }

    #[test]
    fn mcp_stdio_launch_plan_rejects_empty_command() {
        let err = McpStdioLaunchPlan::new(McpStdioLaunchConfig {
            command: "   ".to_owned(),
            args: vec![],
            env: BTreeMap::new(),
            working_dir: None,
            lifecycle_policy: McpStdioLifecyclePolicy::new(1_000, 1_000)
                .expect("timeouts should be in range"),
        })
        .unwrap_err();

        assert_eq!(err.field, "command");
    }

    #[test]
    fn mcp_stdio_launch_plan_rejects_invalid_env_secret_ref() {
        let mut env = BTreeMap::new();
        env.insert(
            "DOCS_MCP_TOKEN".to_owned(),
            McpStdioEnvValue::SecretRef("DOCS_MCP_TOKEN".to_owned()),
        );

        let err = McpStdioLaunchPlan::new(McpStdioLaunchConfig {
            command: "node".to_owned(),
            args: vec!["server.js".to_owned()],
            env,
            working_dir: None,
            lifecycle_policy: McpStdioLifecyclePolicy::new(1_000, 1_000)
                .expect("timeouts should be in range"),
        })
        .unwrap_err();

        assert_eq!(err.field, "env.DOCS_MCP_TOKEN.secret_ref");
    }

    #[test]
    fn mcp_stdio_lifecycle_policy_rejects_out_of_bounds_timeouts() {
        let startup_err =
            McpStdioLifecyclePolicy::new(MCP_STDIO_MIN_TIMEOUT_MS - 1, 1_000).unwrap_err();
        let shutdown_err =
            McpStdioLifecyclePolicy::new(1_000, MCP_STDIO_MAX_TIMEOUT_MS + 1).unwrap_err();

        assert_eq!(startup_err.field, "startup_timeout_ms");
        assert_eq!(shutdown_err.field, "shutdown_timeout_ms");
    }

    #[test]
    fn mcp_stdio_lifecycle_plan_lists_expected_phases() {
        let plan = McpStdioLaunchPlan::new(McpStdioLaunchConfig {
            command: "node".to_owned(),
            args: vec!["server.js".to_owned()],
            env: BTreeMap::new(),
            working_dir: None,
            lifecycle_policy: McpStdioLifecyclePolicy::new(2_000, 1_000)
                .expect("timeouts should be in range"),
        })
        .expect("stdio launch plan should be valid");

        assert_eq!(
            plan.lifecycle_phases(),
            vec![
                McpStdioLifecyclePhase::Spawn,
                McpStdioLifecyclePhase::Initialize,
                McpStdioLifecyclePhase::ListTools,
                McpStdioLifecyclePhase::CallTools,
                McpStdioLifecyclePhase::Shutdown,
            ]
        );
        assert_eq!(
            plan.sanitized_evidence()["lifecyclePhases"],
            serde_json::json!([
                "spawn",
                "initialize",
                "list_tools",
                "call_tools",
                "shutdown"
            ])
        );
    }
}
