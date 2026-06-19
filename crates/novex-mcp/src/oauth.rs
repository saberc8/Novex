use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use url::Url;

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
    RefreshToken,
}

impl McpOAuthGrantType {
    fn as_form_value(self) -> &'static str {
        match self {
            Self::AuthorizationCode => "authorization_code",
            Self::RefreshToken => "refresh_token",
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
pub struct McpOAuthTokenRefreshConfig {
    pub server_code: String,
    pub token_endpoint: String,
    pub client_id: String,
    pub refresh_token: String,
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

    pub fn refresh_token(config: McpOAuthTokenRefreshConfig) -> Result<Self, McpOAuthSessionError> {
        let server_code = required_oauth_field("server_code", &config.server_code)?;
        let token_endpoint = validate_oauth_https_url("token_endpoint", &config.token_endpoint)?;
        let client_id = required_oauth_field("client_id", &config.client_id)?;
        let refresh_token = required_oauth_field("refresh_token", &config.refresh_token)?;
        let client_auth = normalize_oauth_client_auth(config.client_auth)?;

        let mut headers = BTreeMap::new();
        headers.insert("Accept".to_owned(), "application/json".to_owned());
        headers.insert(
            "Content-Type".to_owned(),
            "application/x-www-form-urlencoded".to_owned(),
        );

        let grant_type = McpOAuthGrantType::RefreshToken;
        let mut form = BTreeMap::new();
        form.insert(
            "grant_type".to_owned(),
            grant_type.as_form_value().to_owned(),
        );
        form.insert("refresh_token".to_owned(), refresh_token);
        form.insert("client_id".to_owned(), client_id.clone());

        Ok(Self {
            server_code,
            token_endpoint,
            http_method: "POST".to_owned(),
            headers,
            form,
            grant_type,
            code_verifier_secret_ref: String::new(),
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
                "refreshTokenPresent": self.form.contains_key("refresh_token"),
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
