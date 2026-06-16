use novex_ai_core::FoundationModule;
use novex_tools::{ApprovalPolicy, ToolConcurrencyPolicy, ToolDefinition, ToolRiskLevel};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

pub const CRATE_ID: &str = "novex-mcp";

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
}
