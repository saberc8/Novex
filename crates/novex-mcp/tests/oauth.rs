use std::collections::BTreeMap;

use novex_mcp::*;
use url::Url;

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
        client_auth: McpOAuthClientAuth::ClientSecretRef("env:MCP_OAUTH_CLIENT_SECRET".to_owned()),
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
        client_auth: McpOAuthClientAuth::ClientSecretRef("env:MCP_OAUTH_CLIENT_SECRET".to_owned()),
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
        client_auth: McpOAuthClientAuth::ClientSecretRef("env:MCP_OAUTH_CLIENT_SECRET".to_owned()),
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
fn mcp_oauth_session_token_exchange_plan_builds_refresh_token_form_without_leakage() {
    let plan = McpOAuthTokenExchangePlan::refresh_token(McpOAuthTokenRefreshConfig {
        server_code: "docs".to_owned(),
        token_endpoint: "https://auth.example.com/oauth/token".to_owned(),
        client_id: "novex-mcp-client".to_owned(),
        refresh_token: "refresh-token-secret-value".to_owned(),
        client_auth: McpOAuthClientAuth::ClientSecretRef("env:MCP_OAUTH_CLIENT_SECRET".to_owned()),
    })
    .expect("valid refresh-token config should build a request plan");

    assert_eq!(plan.server_code, "docs");
    assert_eq!(plan.grant_type, McpOAuthGrantType::RefreshToken);
    assert_eq!(
        plan.form.get("grant_type").map(String::as_str),
        Some("refresh_token")
    );
    assert_eq!(
        plan.form.get("refresh_token").map(String::as_str),
        Some("refresh-token-secret-value")
    );
    assert_eq!(
        plan.form.get("client_id").map(String::as_str),
        Some("novex-mcp-client")
    );

    let evidence = plan.sanitized_evidence();
    assert_eq!(evidence["form"]["grantType"], "refresh_token");
    assert_eq!(evidence["form"]["refreshTokenPresent"], true);
    assert_eq!(
        evidence["clientAuth"]["clientSecretRef"],
        "env:MCP_OAUTH_CLIENT_SECRET"
    );
    assert!(!evidence.to_string().contains("refresh-token-secret-value"));
    assert!(!evidence.to_string().contains("client-secret-value"));
}

#[test]
fn mcp_oauth_session_token_exchange_rejects_missing_refresh_token() {
    let err = McpOAuthTokenExchangePlan::refresh_token(McpOAuthTokenRefreshConfig {
        server_code: "docs".to_owned(),
        token_endpoint: "https://auth.example.com/oauth/token".to_owned(),
        client_id: "novex-mcp-client".to_owned(),
        refresh_token: " ".to_owned(),
        client_auth: McpOAuthClientAuth::None,
    })
    .unwrap_err();

    assert_eq!(err.field, "refresh_token");
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
