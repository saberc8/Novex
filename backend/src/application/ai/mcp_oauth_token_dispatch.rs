#![allow(dead_code)]

use std::{collections::BTreeMap, future::Future, time::Duration};

use novex_mcp::{
    mcp_oauth_session_from_token_response, McpOAuthClientAuth, McpOAuthSessionMaterial,
    McpOAuthTokenExchangePlan, McpOAuthTokenResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const MCP_OAUTH_TOKEN_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpOAuthTokenDispatchResolvedSecrets {
    pub code_verifier: String,
    pub client_secret: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpOAuthTokenDispatchHttpResponse {
    pub http_status: u16,
    pub content_type: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpOAuthTokenDispatchOutcome {
    pub request_evidence: Value,
    pub response_meta: Value,
    pub session: McpOAuthSessionMaterial,
}

impl McpOAuthTokenDispatchOutcome {
    pub(crate) fn sanitized_evidence(&self) -> Value {
        json!({
            "request": self.request_evidence,
            "responseMeta": self.response_meta,
            "session": self.session.sanitized_evidence(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpOAuthTokenDispatchError {
    pub field: String,
    pub message: String,
    pub request_evidence: Value,
    pub response_meta: Value,
    pub sanitized_evidence: Value,
}

impl McpOAuthTokenDispatchError {
    fn new(
        field: impl Into<String>,
        message: impl Into<String>,
        request_evidence: Value,
        response_meta: Value,
    ) -> Self {
        let field = field.into();
        let message = message.into();
        let sanitized_evidence = json!({
            "request": request_evidence,
            "responseMeta": response_meta,
            "error": {
                "field": field,
                "message": message,
            },
        });

        Self {
            field,
            message,
            request_evidence,
            response_meta,
            sanitized_evidence,
        }
    }
}

pub(crate) async fn exchange_mcp_oauth_token_with_dispatch<EnvGet, Dispatch, DispatchFuture>(
    plan: McpOAuthTokenExchangePlan,
    received_at_epoch_seconds: u64,
    access_token_secret_ref: &str,
    refresh_token_secret_ref: Option<&str>,
    mut env_get: EnvGet,
    dispatch: Dispatch,
) -> Result<McpOAuthTokenDispatchOutcome, McpOAuthTokenDispatchError>
where
    EnvGet: FnMut(&str) -> Option<String>,
    Dispatch:
        FnOnce(McpOAuthTokenExchangePlan, McpOAuthTokenDispatchResolvedSecrets) -> DispatchFuture,
    DispatchFuture: Future<Output = Result<McpOAuthTokenDispatchHttpResponse, String>>,
{
    let server_code = plan.server_code.clone();
    let request_evidence = plan.sanitized_evidence();
    let code_verifier = resolve_env_secret_ref(
        "code_verifier_secret_ref",
        &plan.code_verifier_secret_ref,
        &mut env_get,
        &request_evidence,
    )?;
    let client_secret = match &plan.client_auth {
        McpOAuthClientAuth::None => None,
        McpOAuthClientAuth::ClientSecretRef(client_secret_ref) => Some(resolve_env_secret_ref(
            "client_auth.client_secret_ref",
            client_secret_ref,
            &mut env_get,
            &request_evidence,
        )?),
    };
    let secrets = McpOAuthTokenDispatchResolvedSecrets {
        code_verifier,
        client_secret,
    };

    let response = dispatch(plan, secrets).await.map_err(|error| {
        McpOAuthTokenDispatchError::new(
            "token_endpoint",
            format!("MCP OAuth token dispatch failed: {error}"),
            request_evidence.clone(),
            Value::Null,
        )
    })?;
    let response_meta = json!({
        "httpStatus": response.http_status,
        "contentType": response.content_type,
    });
    if response.http_status >= 400 {
        return Err(McpOAuthTokenDispatchError::new(
            "token_endpoint",
            format!(
                "MCP OAuth token endpoint returned HTTP {}",
                response.http_status
            ),
            request_evidence,
            response_meta,
        ));
    }

    let token_response =
        serde_json::from_str::<McpOAuthTokenResponse>(&response.body).map_err(|_| {
            McpOAuthTokenDispatchError::new(
                "token_response",
                "MCP OAuth token response JSON is invalid",
                request_evidence.clone(),
                response_meta.clone(),
            )
        })?;
    let session = mcp_oauth_session_from_token_response(
        server_code,
        &token_response,
        received_at_epoch_seconds,
        access_token_secret_ref,
        refresh_token_secret_ref,
    )
    .map_err(|error| {
        McpOAuthTokenDispatchError::new(
            error.field,
            error.message,
            request_evidence.clone(),
            response_meta.clone(),
        )
    })?;

    Ok(McpOAuthTokenDispatchOutcome {
        request_evidence,
        response_meta,
        session,
    })
}

pub(crate) async fn dispatch_mcp_oauth_token_request(
    plan: McpOAuthTokenExchangePlan,
    secrets: McpOAuthTokenDispatchResolvedSecrets,
) -> Result<McpOAuthTokenDispatchHttpResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(MCP_OAUTH_TOKEN_TIMEOUT)
        .user_agent("novex-mcp-oauth-token")
        .build()
        .map_err(|err| format!("MCP OAuth token client init failed: {err}"))?;
    let mut form = plan.form.clone();
    form.insert("code_verifier".to_owned(), secrets.code_verifier);
    if let Some(client_secret) = secrets
        .client_secret
        .filter(|client_secret| !client_secret.trim().is_empty())
    {
        form.insert("client_secret".to_owned(), client_secret);
    }
    let body = form_urlencoded_body(&form);

    let mut request = client.post(&plan.token_endpoint);
    for (header, value) in &plan.headers {
        request = request.header(header.as_str(), value.as_str());
    }
    let response = request
        .body(body)
        .send()
        .await
        .map_err(|err| format!("MCP OAuth token dispatch failed: {err}"))?;
    let http_status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/json")
        .to_owned();
    let body = response
        .text()
        .await
        .map_err(|err| format!("MCP OAuth token response read failed: {err}"))?;

    Ok(McpOAuthTokenDispatchHttpResponse {
        http_status,
        content_type,
        body,
    })
}

fn resolve_env_secret_ref<F>(
    field: &str,
    secret_ref: &str,
    env_get: &mut F,
    request_evidence: &Value,
) -> Result<String, McpOAuthTokenDispatchError>
where
    F: FnMut(&str) -> Option<String>,
{
    let env_key = secret_ref.trim().strip_prefix("env:").ok_or_else(|| {
        McpOAuthTokenDispatchError::new(
            field,
            format!("MCP OAuth {field} must use env: prefix"),
            request_evidence.clone(),
            Value::Null,
        )
    })?;
    if env_key.trim().is_empty() {
        return Err(McpOAuthTokenDispatchError::new(
            field,
            format!("MCP OAuth {field} env key is required"),
            request_evidence.clone(),
            Value::Null,
        ));
    }

    env_get(env_key)
        .map(|secret| secret.trim().to_owned())
        .filter(|secret| !secret.is_empty())
        .ok_or_else(|| {
            McpOAuthTokenDispatchError::new(
                field,
                format!("MCP OAuth {field} is not resolved"),
                request_evidence.clone(),
                Value::Null,
            )
        })
}

fn form_urlencoded_body(form: &BTreeMap<String, String>) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in form {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, net::SocketAddr, sync::Arc};

    use axum::{
        body::Bytes,
        extract::State,
        http::{HeaderMap, Method},
        routing::post,
        Router,
    };
    use novex_mcp::{
        McpOAuthClientAuth, McpOAuthGrantType, McpOAuthTokenExchangeConfig,
        McpOAuthTokenExchangePlan,
    };
    use serde_json::json;
    use tokio::sync::{oneshot, Mutex};

    use super::*;

    #[derive(Debug, Clone, Default)]
    struct LocalTokenCapture {
        method: String,
        headers: BTreeMap<String, String>,
        body: String,
    }

    fn token_exchange_plan(endpoint: &str) -> McpOAuthTokenExchangePlan {
        if endpoint.starts_with("http://") {
            let mut headers = BTreeMap::new();
            headers.insert("Accept".to_owned(), "application/json".to_owned());
            headers.insert(
                "Content-Type".to_owned(),
                "application/x-www-form-urlencoded".to_owned(),
            );
            let mut form = BTreeMap::new();
            form.insert("grant_type".to_owned(), "authorization_code".to_owned());
            form.insert("code".to_owned(), "authorization-code-value".to_owned());
            form.insert("client_id".to_owned(), "novex-mcp-client".to_owned());
            form.insert(
                "redirect_uri".to_owned(),
                "https://novex.example.com/mcp/oauth/callback".to_owned(),
            );

            return McpOAuthTokenExchangePlan {
                server_code: "docs".to_owned(),
                token_endpoint: endpoint.to_owned(),
                http_method: "POST".to_owned(),
                headers,
                form,
                grant_type: McpOAuthGrantType::AuthorizationCode,
                code_verifier_secret_ref: "env:MCP_OAUTH_CODE_VERIFIER".to_owned(),
                client_auth: McpOAuthClientAuth::ClientSecretRef(
                    "env:MCP_OAUTH_CLIENT_SECRET".to_owned(),
                ),
            };
        }

        McpOAuthTokenExchangePlan::authorization_code(McpOAuthTokenExchangeConfig {
            server_code: "docs".to_owned(),
            token_endpoint: endpoint.to_owned(),
            client_id: "novex-mcp-client".to_owned(),
            redirect_uri: "https://novex.example.com/mcp/oauth/callback".to_owned(),
            authorization_code: "authorization-code-value".to_owned(),
            code_verifier_secret_ref: "env:MCP_OAUTH_CODE_VERIFIER".to_owned(),
            client_auth: McpOAuthClientAuth::ClientSecretRef(
                "env:MCP_OAUTH_CLIENT_SECRET".to_owned(),
            ),
        })
        .expect("valid token exchange plan")
    }

    #[tokio::test]
    async fn mcp_oauth_token_dispatch_resolves_env_secrets_and_returns_session_evidence() {
        let plan = token_exchange_plan("https://auth.example.com/oauth/token");

        let outcome = exchange_mcp_oauth_token_with_dispatch(
            plan,
            1_700_000_000,
            "env:DOCS_MCP_ACCESS_TOKEN",
            Some("env:DOCS_MCP_REFRESH_TOKEN"),
            |key| match key {
                "MCP_OAUTH_CODE_VERIFIER" => Some("code-verifier-value".to_owned()),
                "MCP_OAUTH_CLIENT_SECRET" => Some("client-secret-value".to_owned()),
                _ => None,
            },
            |plan, secrets| async move {
                assert_eq!(plan.token_endpoint, "https://auth.example.com/oauth/token");
                assert_eq!(secrets.code_verifier, "code-verifier-value");
                assert_eq!(
                    secrets.client_secret.as_deref(),
                    Some("client-secret-value")
                );
                Ok(McpOAuthTokenDispatchHttpResponse {
                    http_status: 200,
                    content_type: "application/json".to_owned(),
                    body: json!({
                        "access_token": "access-token-value",
                        "token_type": "Bearer",
                        "expires_in": 3600,
                        "refresh_token": "refresh-token-value",
                        "scope": "mcp:tools offline_access"
                    })
                    .to_string(),
                })
            },
        )
        .await
        .expect("valid token dispatch should return secret-backed session");

        assert_eq!(outcome.session.server_code, "docs");
        assert_eq!(
            outcome.session.access_token_secret_ref,
            "env:DOCS_MCP_ACCESS_TOKEN"
        );
        assert_eq!(
            outcome.session.refresh_token_secret_ref.as_deref(),
            Some("env:DOCS_MCP_REFRESH_TOKEN")
        );
        assert_eq!(
            outcome.session.expires_at_epoch_seconds,
            Some(1_700_003_600)
        );
        assert_eq!(outcome.response_meta["httpStatus"], 200);
        assert_eq!(
            outcome.request_evidence["codeVerifierSecretRef"],
            "env:MCP_OAUTH_CODE_VERIFIER"
        );
        let evidence = serde_json::to_string(&outcome.sanitized_evidence()).unwrap();
        for secret in [
            "authorization-code-value",
            "code-verifier-value",
            "client-secret-value",
            "access-token-value",
            "refresh-token-value",
        ] {
            assert!(!evidence.contains(secret), "leaked secret: {secret}");
        }
    }

    #[tokio::test]
    async fn mcp_oauth_token_dispatch_rejects_missing_code_verifier_secret_without_dispatch() {
        let plan = token_exchange_plan("https://auth.example.com/oauth/token");

        let err = exchange_mcp_oauth_token_with_dispatch(
            plan,
            1_700_000_000,
            "env:DOCS_MCP_ACCESS_TOKEN",
            None,
            |_key| None,
            |_plan, _secrets| async move {
                panic!("token dispatch must not run when code verifier is unresolved")
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.field, "code_verifier_secret_ref");
        assert!(!err
            .sanitized_evidence
            .to_string()
            .contains("code-verifier-value"));
    }

    #[tokio::test]
    async fn mcp_oauth_token_dispatch_rejects_missing_client_secret_without_dispatch() {
        let plan = token_exchange_plan("https://auth.example.com/oauth/token");

        let err = exchange_mcp_oauth_token_with_dispatch(
            plan,
            1_700_000_000,
            "env:DOCS_MCP_ACCESS_TOKEN",
            None,
            |key| (key == "MCP_OAUTH_CODE_VERIFIER").then(|| "code-verifier-value".to_owned()),
            |_plan, _secrets| async move {
                panic!("token dispatch must not run when client secret is unresolved")
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.field, "client_auth.client_secret_ref");
        assert!(!err
            .sanitized_evidence
            .to_string()
            .contains("client-secret-value"));
        assert!(!err
            .sanitized_evidence
            .to_string()
            .contains("code-verifier-value"));
    }

    #[tokio::test]
    async fn mcp_oauth_token_dispatch_rejects_http_status_without_secret_leakage() {
        let plan = token_exchange_plan("https://auth.example.com/oauth/token");

        let err = exchange_mcp_oauth_token_with_dispatch(
            plan,
            1_700_000_000,
            "env:DOCS_MCP_ACCESS_TOKEN",
            None,
            |key| match key {
                "MCP_OAUTH_CODE_VERIFIER" => Some("code-verifier-value".to_owned()),
                "MCP_OAUTH_CLIENT_SECRET" => Some("client-secret-value".to_owned()),
                _ => None,
            },
            |_plan, _secrets| async move {
                Ok(McpOAuthTokenDispatchHttpResponse {
                    http_status: 401,
                    content_type: "application/json".to_owned(),
                    body: json!({
                        "error": "invalid_client",
                        "error_description": "client-secret-value is wrong"
                    })
                    .to_string(),
                })
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.field, "token_endpoint");
        assert_eq!(err.response_meta["httpStatus"], 401);
        assert!(!err
            .sanitized_evidence
            .to_string()
            .contains("client-secret-value"));
        assert!(!err
            .sanitized_evidence
            .to_string()
            .contains("code-verifier-value"));
    }

    #[tokio::test]
    async fn mcp_oauth_token_dispatch_reaches_local_token_endpoint() {
        let (endpoint, capture_rx) = run_one_shot_token_server().await;
        let plan = token_exchange_plan(&endpoint);

        let outcome = exchange_mcp_oauth_token_with_dispatch(
            plan,
            1_700_000_000,
            "env:DOCS_MCP_ACCESS_TOKEN",
            Some("env:DOCS_MCP_REFRESH_TOKEN"),
            |key| match key {
                "MCP_OAUTH_CODE_VERIFIER" => Some("local-code-verifier".to_owned()),
                "MCP_OAUTH_CLIENT_SECRET" => Some("local-client-secret".to_owned()),
                _ => None,
            },
            |plan, secrets| async move { dispatch_mcp_oauth_token_request(plan, secrets).await },
        )
        .await
        .expect("local token endpoint should return session material");
        let captured = capture_rx
            .await
            .expect("local token endpoint should capture one request");

        assert_eq!(captured.method, "POST");
        assert_eq!(
            captured.headers["content-type"],
            "application/x-www-form-urlencoded"
        );
        assert_eq!(captured.headers["accept"], "application/json");
        assert!(captured.body.contains("grant_type=authorization_code"));
        assert!(captured.body.contains("code=authorization-code-value"));
        assert!(captured.body.contains("client_id=novex-mcp-client"));
        assert!(captured.body.contains("code_verifier=local-code-verifier"));
        assert!(captured.body.contains("client_secret=local-client-secret"));
        assert_eq!(
            outcome.session.access_token_secret_ref,
            "env:DOCS_MCP_ACCESS_TOKEN"
        );
        assert!(!outcome
            .sanitized_evidence()
            .to_string()
            .contains("local-client-secret"));
    }

    async fn run_one_shot_token_server() -> (String, oneshot::Receiver<LocalTokenCapture>) {
        let (capture_tx, capture_rx) = oneshot::channel();
        let capture_tx = Arc::new(Mutex::new(Some(capture_tx)));
        let app = Router::new()
            .route("/oauth/token", post(local_token_handler))
            .with_state(capture_tx);
        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .expect("local token listener should bind");
        let addr = listener.local_addr().expect("local token addr");

        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("local token server should serve one request");
        });

        (format!("http://{addr}/oauth/token"), capture_rx)
    }

    async fn local_token_handler(
        State(capture_tx): State<Arc<Mutex<Option<oneshot::Sender<LocalTokenCapture>>>>>,
        method: Method,
        headers: HeaderMap,
        body: Bytes,
    ) -> String {
        let headers = headers
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_owned(), value.to_owned()))
            })
            .collect::<BTreeMap<_, _>>();
        let body = String::from_utf8(body.to_vec()).expect("form body should be utf8");
        let capture = LocalTokenCapture {
            method: method.as_str().to_owned(),
            headers,
            body,
        };
        if let Some(sender) = capture_tx.lock().await.take() {
            let _ = sender.send(capture);
        }

        json!({
            "access_token": "local-access-token",
            "token_type": "Bearer",
            "expires_in": 60,
            "refresh_token": "local-refresh-token",
            "scope": "mcp:tools"
        })
        .to_string()
    }
}
