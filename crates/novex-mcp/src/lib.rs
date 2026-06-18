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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpOAuthPkceMethod {
    S256,
}

impl McpOAuthPkceMethod {
    fn as_query_value(self) -> &'static str {
        match self {
            Self::S256 => "S256",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum McpOAuthClientAuth {
    None,
    ClientSecretRef(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthAuthorizationConfig {
    pub server_code: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub state: String,
    pub pkce_challenge: String,
    pub pkce_method: McpOAuthPkceMethod,
    pub client_auth: McpOAuthClientAuth,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthAuthorizationPlan {
    pub server_code: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub authorization_url: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub state: String,
    pub pkce_challenge: String,
    pub pkce_method: McpOAuthPkceMethod,
    pub client_auth: McpOAuthClientAuth,
}

impl McpOAuthAuthorizationPlan {
    pub fn new(config: McpOAuthAuthorizationConfig) -> Result<Self, McpOAuthAuthorizationError> {
        let server_code = required_oauth_field("server_code", &config.server_code)?;
        let authorization_endpoint =
            validate_oauth_https_url("authorization_endpoint", &config.authorization_endpoint)?;
        let token_endpoint = validate_oauth_https_url("token_endpoint", &config.token_endpoint)?;
        let client_id = required_oauth_field("client_id", &config.client_id)?;
        let redirect_uri = validate_oauth_redirect_uri(&config.redirect_uri)?;
        let scopes = normalize_oauth_scopes(config.scopes)?;
        let state = required_oauth_field("state", &config.state)?;
        let pkce_challenge = required_oauth_field("pkce_challenge", &config.pkce_challenge)?;
        let client_auth = normalize_oauth_client_auth(config.client_auth)?;
        let authorization_url = build_oauth_authorization_url(
            &authorization_endpoint,
            &client_id,
            &redirect_uri,
            &scopes,
            &state,
            &pkce_challenge,
            config.pkce_method,
        )?;

        Ok(Self {
            server_code,
            authorization_endpoint,
            token_endpoint,
            authorization_url,
            client_id,
            redirect_uri,
            scopes,
            state,
            pkce_challenge,
            pkce_method: config.pkce_method,
            client_auth,
        })
    }

    pub fn sanitized_evidence(&self) -> Value {
        let client_auth = match &self.client_auth {
            McpOAuthClientAuth::None => json!({
                "kind": "none",
            }),
            McpOAuthClientAuth::ClientSecretRef(client_secret_ref) => json!({
                "kind": "client_secret_ref",
                "clientSecretRef": client_secret_ref,
            }),
        };

        json!({
            "serverCode": self.server_code,
            "authorizationEndpoint": self.authorization_endpoint,
            "tokenEndpoint": self.token_endpoint,
            "authorizationUrl": self.authorization_url,
            "clientId": self.client_id,
            "redirectUri": self.redirect_uri,
            "scopes": self.scopes,
            "state": self.state,
            "pkce": {
                "method": self.pkce_method.as_query_value(),
                "challenge": self.pkce_challenge,
            },
            "clientAuth": client_auth,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthAuthorizationError {
    pub field: String,
    pub message: String,
}

impl McpOAuthAuthorizationError {
    fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

fn required_oauth_field(field: &str, value: &str) -> Result<String, McpOAuthAuthorizationError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(McpOAuthAuthorizationError::new(
            field,
            format!("MCP OAuth {field} is required"),
        ));
    }
    Ok(value.to_owned())
}

fn validate_oauth_https_url(
    field: &str,
    value: &str,
) -> Result<String, McpOAuthAuthorizationError> {
    let value = required_oauth_field(field, value)?;
    let url = Url::parse(&value).map_err(|_| {
        McpOAuthAuthorizationError::new(field, format!("MCP OAuth {field} is invalid"))
    })?;
    if url.scheme() != "https" {
        return Err(McpOAuthAuthorizationError::new(
            field,
            format!("MCP OAuth {field} must use https"),
        ));
    }
    if url.host_str().is_none() {
        return Err(McpOAuthAuthorizationError::new(
            field,
            format!("MCP OAuth {field} missing host"),
        ));
    }
    Ok(value)
}

fn validate_oauth_redirect_uri(value: &str) -> Result<String, McpOAuthAuthorizationError> {
    let value = required_oauth_field("redirect_uri", value)?;
    let url = Url::parse(&value).map_err(|_| {
        McpOAuthAuthorizationError::new("redirect_uri", "MCP OAuth redirect_uri is invalid")
    })?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(McpOAuthAuthorizationError::new(
            "redirect_uri",
            "MCP OAuth redirect_uri must be an absolute http/https URL",
        ));
    }
    Ok(value)
}

fn normalize_oauth_scopes(scopes: Vec<String>) -> Result<Vec<String>, McpOAuthAuthorizationError> {
    let scopes = scopes
        .into_iter()
        .map(|scope| scope.trim().to_owned())
        .filter(|scope| !scope.is_empty())
        .collect::<Vec<_>>();
    if scopes.is_empty() {
        return Err(McpOAuthAuthorizationError::new(
            "scopes",
            "MCP OAuth scopes are required",
        ));
    }
    Ok(scopes)
}

fn normalize_oauth_client_auth(
    client_auth: McpOAuthClientAuth,
) -> Result<McpOAuthClientAuth, McpOAuthAuthorizationError> {
    match client_auth {
        McpOAuthClientAuth::None => Ok(McpOAuthClientAuth::None),
        McpOAuthClientAuth::ClientSecretRef(client_secret_ref) => {
            let client_secret_ref = client_secret_ref.trim();
            if client_secret_ref.is_empty() || !client_secret_ref.starts_with("env:") {
                return Err(McpOAuthAuthorizationError::new(
                    "client_auth.client_secret_ref",
                    "MCP OAuth clientSecretRef must use env: prefix",
                ));
            }
            Ok(McpOAuthClientAuth::ClientSecretRef(
                client_secret_ref.to_owned(),
            ))
        }
    }
}

fn build_oauth_authorization_url(
    authorization_endpoint: &str,
    client_id: &str,
    redirect_uri: &str,
    scopes: &[String],
    state: &str,
    pkce_challenge: &str,
    pkce_method: McpOAuthPkceMethod,
) -> Result<String, McpOAuthAuthorizationError> {
    let mut url = Url::parse(authorization_endpoint).map_err(|_| {
        McpOAuthAuthorizationError::new(
            "authorization_endpoint",
            "MCP OAuth authorization_endpoint is invalid",
        )
    })?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", &scopes.join(" "))
        .append_pair("state", state)
        .append_pair("code_challenge", pkce_challenge)
        .append_pair("code_challenge_method", pkce_method.as_query_value());
    Ok(url.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpOAuthGrantType {
    AuthorizationCode,
}

impl McpOAuthGrantType {
    fn as_form_value(self) -> &'static str {
        match self {
            Self::AuthorizationCode => "authorization_code",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthTokenExchangeConfig {
    pub server_code: String,
    pub token_endpoint: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub authorization_code: String,
    pub code_verifier_secret_ref: String,
    pub client_auth: McpOAuthClientAuth,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthTokenExchangePlan {
    pub server_code: String,
    pub token_endpoint: String,
    pub http_method: String,
    pub headers: BTreeMap<String, String>,
    pub form: BTreeMap<String, String>,
    pub grant_type: McpOAuthGrantType,
    pub code_verifier_secret_ref: String,
    pub client_auth: McpOAuthClientAuth,
}

impl McpOAuthTokenExchangePlan {
    pub fn authorization_code(
        config: McpOAuthTokenExchangeConfig,
    ) -> Result<Self, McpOAuthSessionError> {
        let server_code = required_oauth_field("server_code", &config.server_code)?;
        let token_endpoint = validate_oauth_https_url("token_endpoint", &config.token_endpoint)?;
        let client_id = required_oauth_field("client_id", &config.client_id)?;
        let redirect_uri = validate_oauth_redirect_uri(&config.redirect_uri)?;
        let authorization_code =
            required_oauth_field("authorization_code", &config.authorization_code)?;
        let code_verifier_secret_ref = validate_oauth_runtime_secret_ref(
            "code_verifier_secret_ref",
            &config.code_verifier_secret_ref,
        )?;
        let client_auth = normalize_oauth_client_auth(config.client_auth)?;

        let mut headers = BTreeMap::new();
        headers.insert("Accept".to_owned(), "application/json".to_owned());
        headers.insert(
            "Content-Type".to_owned(),
            "application/x-www-form-urlencoded".to_owned(),
        );

        let grant_type = McpOAuthGrantType::AuthorizationCode;
        let mut form = BTreeMap::new();
        form.insert(
            "grant_type".to_owned(),
            grant_type.as_form_value().to_owned(),
        );
        form.insert("code".to_owned(), authorization_code);
        form.insert("client_id".to_owned(), client_id.clone());
        form.insert("redirect_uri".to_owned(), redirect_uri.clone());

        Ok(Self {
            server_code,
            token_endpoint,
            http_method: "POST".to_owned(),
            headers,
            form,
            grant_type,
            code_verifier_secret_ref,
            client_auth,
        })
    }

    pub fn sanitized_evidence(&self) -> Value {
        json!({
            "serverCode": self.server_code,
            "tokenEndpoint": self.token_endpoint,
            "httpMethod": self.http_method,
            "headers": self.headers,
            "form": {
                "grantType": self.grant_type.as_form_value(),
                "clientId": self.form.get("client_id"),
                "redirectUri": self.form.get("redirect_uri"),
                "authorizationCodePresent": self.form.contains_key("code"),
            },
            "codeVerifierSecretRef": self.code_verifier_secret_ref,
            "clientAuth": oauth_client_auth_evidence(&self.client_auth),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpOAuthTokenResponse {
    #[serde(rename = "access_token")]
    pub access_token: String,
    #[serde(rename = "token_type")]
    pub token_type: String,
    #[serde(rename = "expires_in")]
    pub expires_in_seconds: Option<u64>,
    #[serde(rename = "refresh_token")]
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthSessionMaterial {
    pub server_code: String,
    pub access_token_secret_ref: String,
    pub refresh_token_secret_ref: Option<String>,
    pub token_type: String,
    pub scopes: Vec<String>,
    pub expires_at_epoch_seconds: Option<u64>,
}

impl McpOAuthSessionMaterial {
    pub fn refresh_needed(&self, now_epoch_seconds: u64, skew_seconds: u64) -> bool {
        self.expires_at_epoch_seconds
            .is_some_and(|expires_at| now_epoch_seconds.saturating_add(skew_seconds) >= expires_at)
    }

    pub fn sanitized_evidence(&self) -> Value {
        json!({
            "serverCode": self.server_code,
            "accessTokenSecretRef": self.access_token_secret_ref,
            "refreshTokenSecretRef": self.refresh_token_secret_ref,
            "tokenType": self.token_type,
            "scopes": self.scopes,
            "expiresAtEpochSeconds": self.expires_at_epoch_seconds,
        })
    }
}

pub fn mcp_oauth_session_from_token_response(
    server_code: impl AsRef<str>,
    response: &McpOAuthTokenResponse,
    received_at_epoch_seconds: u64,
    access_token_secret_ref: impl AsRef<str>,
    refresh_token_secret_ref: Option<&str>,
) -> Result<McpOAuthSessionMaterial, McpOAuthSessionError> {
    let server_code = required_oauth_field("server_code", server_code.as_ref())?;
    required_oauth_field("access_token", &response.access_token)?;
    let token_type = required_oauth_field("token_type", &response.token_type)?;
    if !token_type.eq_ignore_ascii_case("bearer") {
        return Err(McpOAuthSessionError::new(
            "token_type",
            "MCP OAuth token_type must be Bearer",
        ));
    }

    let access_token_secret_ref = validate_oauth_token_storage_secret_ref(
        "access_token_secret_ref",
        access_token_secret_ref.as_ref(),
    )?;
    let refresh_token_present = response
        .refresh_token
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let refresh_token_secret_ref = match (refresh_token_present, refresh_token_secret_ref) {
        (true, Some(secret_ref)) => Some(validate_oauth_token_storage_secret_ref(
            "refresh_token_secret_ref",
            secret_ref,
        )?),
        (true, None) => {
            return Err(McpOAuthSessionError::new(
                "refresh_token_secret_ref",
                "MCP OAuth refresh_token_secret_ref is required when refresh_token is present",
            ))
        }
        (false, Some(secret_ref)) => Some(validate_oauth_token_storage_secret_ref(
            "refresh_token_secret_ref",
            secret_ref,
        )?),
        (false, None) => None,
    };
    let scopes = response
        .scope
        .as_deref()
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let expires_at_epoch_seconds = response
        .expires_in_seconds
        .map(|expires_in| received_at_epoch_seconds.saturating_add(expires_in));

    Ok(McpOAuthSessionMaterial {
        server_code,
        access_token_secret_ref,
        refresh_token_secret_ref,
        token_type: "Bearer".to_owned(),
        scopes,
        expires_at_epoch_seconds,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthSessionError {
    pub field: String,
    pub message: String,
}

impl McpOAuthSessionError {
    fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl From<McpOAuthAuthorizationError> for McpOAuthSessionError {
    fn from(error: McpOAuthAuthorizationError) -> Self {
        Self {
            field: error.field,
            message: error.message,
        }
    }
}

fn validate_oauth_runtime_secret_ref(
    field: &str,
    value: &str,
) -> Result<String, McpOAuthSessionError> {
    let value = value.trim();
    if value.is_empty() || !value.starts_with("env:") {
        return Err(McpOAuthSessionError::new(
            field,
            format!("MCP OAuth {field} must use env: prefix"),
        ));
    }
    Ok(value.to_owned())
}

fn validate_oauth_token_storage_secret_ref(
    field: &str,
    value: &str,
) -> Result<String, McpOAuthSessionError> {
    let value = value.trim();
    if value.is_empty() || !(value.starts_with("env:") || value.starts_with("sys:")) {
        return Err(McpOAuthSessionError::new(
            field,
            format!("MCP OAuth {field} must use env: or sys: prefix"),
        ));
    }
    Ok(value.to_owned())
}

fn oauth_client_auth_evidence(client_auth: &McpOAuthClientAuth) -> Value {
    match client_auth {
        McpOAuthClientAuth::None => json!({
            "kind": "none",
        }),
        McpOAuthClientAuth::ClientSecretRef(client_secret_ref) => json!({
            "kind": "client_secret_ref",
            "clientSecretRef": client_secret_ref,
        }),
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
    fn mcp_oauth_authorization_plan_builds_pkce_authorize_url() {
        let plan = McpOAuthAuthorizationPlan::new(McpOAuthAuthorizationConfig {
            server_code: "docs".to_owned(),
            authorization_endpoint: "https://auth.example.com/oauth/authorize".to_owned(),
            token_endpoint: "https://auth.example.com/oauth/token".to_owned(),
            client_id: "novex-mcp-client".to_owned(),
            redirect_uri: "https://novex.example.com/mcp/oauth/callback".to_owned(),
            scopes: vec!["mcp:tools".to_owned(), "offline_access".to_owned()],
            state: "tenant-42-state".to_owned(),
            pkce_challenge: "s256-code-challenge".to_owned(),
            pkce_method: McpOAuthPkceMethod::S256,
            client_auth: McpOAuthClientAuth::ClientSecretRef(
                "env:MCP_OAUTH_CLIENT_SECRET".to_owned(),
            ),
        })
        .expect("valid OAuth config should build an authorization plan");

        let authorization_url =
            Url::parse(&plan.authorization_url).expect("authorization URL should be parseable");
        let query = authorization_url.query_pairs().collect::<BTreeMap<_, _>>();

        assert_eq!(plan.server_code, "docs");
        assert_eq!(plan.token_endpoint, "https://auth.example.com/oauth/token");
        assert_eq!(
            query.get("response_type").map(|value| value.as_ref()),
            Some("code")
        );
        assert_eq!(
            query.get("client_id").map(|value| value.as_ref()),
            Some("novex-mcp-client")
        );
        assert_eq!(
            query.get("redirect_uri").map(|value| value.as_ref()),
            Some("https://novex.example.com/mcp/oauth/callback")
        );
        assert_eq!(
            query.get("scope").map(|value| value.as_ref()),
            Some("mcp:tools offline_access")
        );
        assert_eq!(
            query.get("state").map(|value| value.as_ref()),
            Some("tenant-42-state")
        );
        assert_eq!(
            query.get("code_challenge").map(|value| value.as_ref()),
            Some("s256-code-challenge")
        );
        assert_eq!(
            query
                .get("code_challenge_method")
                .map(|value| value.as_ref()),
            Some("S256")
        );
    }

    #[test]
    fn mcp_oauth_authorization_plan_sanitizes_client_secret_ref() {
        let plan = McpOAuthAuthorizationPlan::new(McpOAuthAuthorizationConfig {
            server_code: "docs".to_owned(),
            authorization_endpoint: "https://auth.example.com/oauth/authorize".to_owned(),
            token_endpoint: "https://auth.example.com/oauth/token".to_owned(),
            client_id: "novex-mcp-client".to_owned(),
            redirect_uri: "https://novex.example.com/mcp/oauth/callback".to_owned(),
            scopes: vec!["mcp:tools".to_owned()],
            state: "tenant-42-state".to_owned(),
            pkce_challenge: "s256-code-challenge".to_owned(),
            pkce_method: McpOAuthPkceMethod::S256,
            client_auth: McpOAuthClientAuth::ClientSecretRef(
                "env:MCP_OAUTH_CLIENT_SECRET".to_owned(),
            ),
        })
        .expect("valid OAuth config should build an authorization plan");

        let evidence = plan.sanitized_evidence();

        assert_eq!(evidence["clientAuth"]["kind"], "client_secret_ref");
        assert_eq!(
            evidence["clientAuth"]["clientSecretRef"],
            "env:MCP_OAUTH_CLIENT_SECRET"
        );
        assert_eq!(evidence["pkce"]["method"], "S256");
        assert!(!evidence.to_string().contains("super-secret-value"));
    }

    #[test]
    fn mcp_oauth_authorization_plan_rejects_non_https_endpoint() {
        let err = McpOAuthAuthorizationPlan::new(McpOAuthAuthorizationConfig {
            server_code: "docs".to_owned(),
            authorization_endpoint: "http://auth.example.com/oauth/authorize".to_owned(),
            token_endpoint: "https://auth.example.com/oauth/token".to_owned(),
            client_id: "novex-mcp-client".to_owned(),
            redirect_uri: "https://novex.example.com/mcp/oauth/callback".to_owned(),
            scopes: vec!["mcp:tools".to_owned()],
            state: "tenant-42-state".to_owned(),
            pkce_challenge: "s256-code-challenge".to_owned(),
            pkce_method: McpOAuthPkceMethod::S256,
            client_auth: McpOAuthClientAuth::None,
        })
        .unwrap_err();

        assert_eq!(err.field, "authorization_endpoint");
    }

    #[test]
    fn mcp_oauth_authorization_plan_rejects_invalid_client_secret_ref() {
        let err = McpOAuthAuthorizationPlan::new(McpOAuthAuthorizationConfig {
            server_code: "docs".to_owned(),
            authorization_endpoint: "https://auth.example.com/oauth/authorize".to_owned(),
            token_endpoint: "https://auth.example.com/oauth/token".to_owned(),
            client_id: "novex-mcp-client".to_owned(),
            redirect_uri: "https://novex.example.com/mcp/oauth/callback".to_owned(),
            scopes: vec!["mcp:tools".to_owned()],
            state: "tenant-42-state".to_owned(),
            pkce_challenge: "s256-code-challenge".to_owned(),
            pkce_method: McpOAuthPkceMethod::S256,
            client_auth: McpOAuthClientAuth::ClientSecretRef("plain-secret".to_owned()),
        })
        .unwrap_err();

        assert_eq!(err.field, "client_auth.client_secret_ref");
    }

    #[test]
    fn mcp_oauth_authorization_plan_requires_scope_and_state() {
        let no_scope = McpOAuthAuthorizationPlan::new(McpOAuthAuthorizationConfig {
            server_code: "docs".to_owned(),
            authorization_endpoint: "https://auth.example.com/oauth/authorize".to_owned(),
            token_endpoint: "https://auth.example.com/oauth/token".to_owned(),
            client_id: "novex-mcp-client".to_owned(),
            redirect_uri: "https://novex.example.com/mcp/oauth/callback".to_owned(),
            scopes: vec![" ".to_owned()],
            state: "tenant-42-state".to_owned(),
            pkce_challenge: "s256-code-challenge".to_owned(),
            pkce_method: McpOAuthPkceMethod::S256,
            client_auth: McpOAuthClientAuth::None,
        })
        .unwrap_err();
        let no_state = McpOAuthAuthorizationPlan::new(McpOAuthAuthorizationConfig {
            server_code: "docs".to_owned(),
            authorization_endpoint: "https://auth.example.com/oauth/authorize".to_owned(),
            token_endpoint: "https://auth.example.com/oauth/token".to_owned(),
            client_id: "novex-mcp-client".to_owned(),
            redirect_uri: "https://novex.example.com/mcp/oauth/callback".to_owned(),
            scopes: vec!["mcp:tools".to_owned()],
            state: " ".to_owned(),
            pkce_challenge: "s256-code-challenge".to_owned(),
            pkce_method: McpOAuthPkceMethod::S256,
            client_auth: McpOAuthClientAuth::None,
        })
        .unwrap_err();

        assert_eq!(no_scope.field, "scopes");
        assert_eq!(no_state.field, "state");
    }

    #[test]
    fn mcp_oauth_session_token_exchange_plan_builds_authorization_code_form() {
        let plan = McpOAuthTokenExchangePlan::authorization_code(McpOAuthTokenExchangeConfig {
            server_code: "docs".to_owned(),
            token_endpoint: "https://auth.example.com/oauth/token".to_owned(),
            client_id: "novex-mcp-client".to_owned(),
            redirect_uri: "https://novex.example.com/mcp/oauth/callback".to_owned(),
            authorization_code: "authorization-code-value".to_owned(),
            code_verifier_secret_ref: "env:MCP_OAUTH_CODE_VERIFIER".to_owned(),
            client_auth: McpOAuthClientAuth::ClientSecretRef(
                "env:MCP_OAUTH_CLIENT_SECRET".to_owned(),
            ),
        })
        .expect("valid token exchange config should build a request plan");

        assert_eq!(plan.server_code, "docs");
        assert_eq!(plan.http_method, "POST");
        assert_eq!(
            plan.headers.get("Content-Type").map(String::as_str),
            Some("application/x-www-form-urlencoded")
        );
        assert_eq!(
            plan.headers.get("Accept").map(String::as_str),
            Some("application/json")
        );
        assert_eq!(
            plan.form.get("grant_type").map(String::as_str),
            Some("authorization_code")
        );
        assert_eq!(
            plan.form.get("code").map(String::as_str),
            Some("authorization-code-value")
        );
        assert_eq!(
            plan.form.get("client_id").map(String::as_str),
            Some("novex-mcp-client")
        );
        assert_eq!(
            plan.form.get("redirect_uri").map(String::as_str),
            Some("https://novex.example.com/mcp/oauth/callback")
        );
        assert_eq!(plan.code_verifier_secret_ref, "env:MCP_OAUTH_CODE_VERIFIER");

        let evidence = plan.sanitized_evidence();
        assert_eq!(evidence["form"]["grantType"], "authorization_code");
        assert_eq!(evidence["form"]["authorizationCodePresent"], true);
        assert_eq!(
            evidence["codeVerifierSecretRef"],
            "env:MCP_OAUTH_CODE_VERIFIER"
        );
        assert_eq!(
            evidence["clientAuth"]["clientSecretRef"],
            "env:MCP_OAUTH_CLIENT_SECRET"
        );
        assert!(!evidence.to_string().contains("authorization-code-value"));
        assert!(!evidence.to_string().contains("code-verifier-value"));
        assert!(!evidence.to_string().contains("client-secret-value"));
    }

    #[test]
    fn mcp_oauth_session_token_exchange_rejects_plain_code_verifier_secret_ref() {
        let err = McpOAuthTokenExchangePlan::authorization_code(McpOAuthTokenExchangeConfig {
            server_code: "docs".to_owned(),
            token_endpoint: "https://auth.example.com/oauth/token".to_owned(),
            client_id: "novex-mcp-client".to_owned(),
            redirect_uri: "https://novex.example.com/mcp/oauth/callback".to_owned(),
            authorization_code: "authorization-code-value".to_owned(),
            code_verifier_secret_ref: "plain-code-verifier".to_owned(),
            client_auth: McpOAuthClientAuth::None,
        })
        .unwrap_err();

        assert_eq!(err.field, "code_verifier_secret_ref");
    }

    #[test]
    fn mcp_oauth_session_parses_token_response_into_secret_backed_session() {
        let response = McpOAuthTokenResponse {
            access_token: "access-token-value".to_owned(),
            token_type: "Bearer".to_owned(),
            expires_in_seconds: Some(3600),
            refresh_token: Some("refresh-token-value".to_owned()),
            scope: Some("mcp:tools offline_access".to_owned()),
        };

        let session = mcp_oauth_session_from_token_response(
            "docs",
            &response,
            1_700_000_000,
            "env:DOCS_MCP_ACCESS_TOKEN",
            Some("env:DOCS_MCP_REFRESH_TOKEN"),
        )
        .expect("valid token response should create secret-backed session material");

        assert_eq!(session.server_code, "docs");
        assert_eq!(session.access_token_secret_ref, "env:DOCS_MCP_ACCESS_TOKEN");
        assert_eq!(
            session.refresh_token_secret_ref.as_deref(),
            Some("env:DOCS_MCP_REFRESH_TOKEN")
        );
        assert_eq!(session.token_type, "Bearer");
        assert_eq!(
            session.scopes,
            vec!["mcp:tools".to_owned(), "offline_access".to_owned()]
        );
        assert_eq!(session.expires_at_epoch_seconds, Some(1_700_003_600));

        let evidence = session.sanitized_evidence();
        assert_eq!(
            evidence["accessTokenSecretRef"],
            "env:DOCS_MCP_ACCESS_TOKEN"
        );
        assert_eq!(
            evidence["refreshTokenSecretRef"],
            "env:DOCS_MCP_REFRESH_TOKEN"
        );
        assert!(!evidence.to_string().contains("access-token-value"));
        assert!(!evidence.to_string().contains("refresh-token-value"));
    }

    #[test]
    fn mcp_oauth_session_accepts_system_secret_refs_for_token_storage() {
        let response = McpOAuthTokenResponse {
            access_token: "access-token-value".to_owned(),
            token_type: "Bearer".to_owned(),
            expires_in_seconds: Some(3600),
            refresh_token: Some("refresh-token-value".to_owned()),
            scope: Some("mcp:tools offline_access".to_owned()),
        };

        let session = mcp_oauth_session_from_token_response(
            "docs",
            &response,
            1_700_000_000,
            "sys:tenant:42:mcp.docs.access",
            Some("sys:tenant:42:mcp.docs.refresh"),
        )
        .expect("OAuth token storage should accept system secret manager refs");

        assert_eq!(
            session.access_token_secret_ref,
            "sys:tenant:42:mcp.docs.access"
        );
        assert_eq!(
            session.refresh_token_secret_ref.as_deref(),
            Some("sys:tenant:42:mcp.docs.refresh")
        );
        let evidence = session.sanitized_evidence().to_string();
        assert!(evidence.contains("sys:tenant:42:mcp.docs.access"));
        assert!(!evidence.contains("access-token-value"));
        assert!(!evidence.contains("refresh-token-value"));
    }

    #[test]
    fn mcp_oauth_session_token_response_deserializes_oauth_json() {
        let response: McpOAuthTokenResponse = serde_json::from_value(serde_json::json!({
            "access_token": "access-token-value",
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": "refresh-token-value",
            "scope": "mcp:tools offline_access"
        }))
        .expect("OAuth token response should use standard token endpoint field names");

        assert_eq!(response.access_token, "access-token-value");
        assert_eq!(response.token_type, "Bearer");
        assert_eq!(response.expires_in_seconds, Some(3600));
        assert_eq!(
            response.refresh_token.as_deref(),
            Some("refresh-token-value")
        );
        assert_eq!(response.scope.as_deref(), Some("mcp:tools offline_access"));
    }

    #[test]
    fn mcp_oauth_session_requires_bearer_token_type() {
        let response = McpOAuthTokenResponse {
            access_token: "access-token-value".to_owned(),
            token_type: "mac".to_owned(),
            expires_in_seconds: Some(3600),
            refresh_token: None,
            scope: Some("mcp:tools".to_owned()),
        };

        let err = mcp_oauth_session_from_token_response(
            "docs",
            &response,
            1_700_000_000,
            "env:DOCS_MCP_ACCESS_TOKEN",
            None,
        )
        .unwrap_err();

        assert_eq!(err.field, "token_type");
    }

    #[test]
    fn mcp_oauth_session_refresh_needed_uses_skew() {
        let session = McpOAuthSessionMaterial {
            server_code: "docs".to_owned(),
            access_token_secret_ref: "env:DOCS_MCP_ACCESS_TOKEN".to_owned(),
            refresh_token_secret_ref: Some("env:DOCS_MCP_REFRESH_TOKEN".to_owned()),
            token_type: "Bearer".to_owned(),
            scopes: vec!["mcp:tools".to_owned()],
            expires_at_epoch_seconds: Some(1100),
        };
        let no_expiry = McpOAuthSessionMaterial {
            expires_at_epoch_seconds: None,
            ..session.clone()
        };

        assert!(session.refresh_needed(1045, 60));
        assert!(!session.refresh_needed(1030, 60));
        assert!(!no_expiry.refresh_needed(2_000, 60));
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
