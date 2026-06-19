use serde::{Deserialize, Serialize};
use url::Url;

use crate::types::{McpAuthScope, McpAuthType, McpServerStatus, McpTransportKind};

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
