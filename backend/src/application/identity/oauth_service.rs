use anyhow::anyhow;
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use url::Url;
use uuid::Uuid;

use crate::{
    infrastructure::persistence::identity_repository::{
        IdentityProviderRecord, IdentityRepository, OAuthStateSaveRecord,
    },
    shared::error::AppError,
};

const GITHUB_PROVIDER_TYPE: &str = "github";
const DEFAULT_GITHUB_AUTHORIZE_URL: &str = "https://github.com/login/oauth/authorize";
const DEFAULT_GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const DEFAULT_GITHUB_USER_INFO_URL: &str = "https://api.github.com/user";
const GITHUB_DEFAULT_SCOPES: &[&str] = &["read:user", "user:email"];
const DEFAULT_TENANT_ID: i64 = 1;
const PUBLIC_CREATE_USER_ID: i64 = 1;
const OAUTH_STATE_TTL_MINUTES: i64 = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthAuthorizePreview {
    pub provider_code: String,
    pub provider_type: String,
    pub authorization_url: String,
    pub state: String,
    pub requested_scopes: Vec<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalIdentityProfile {
    pub external_subject: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct OAuthAuthorizeCommand {
    pub tenant_id: i64,
    pub provider_code: String,
    pub redirect_uri: String,
    pub requested_scopes: Vec<String>,
    pub create_user: i64,
}

#[derive(Debug, Clone)]
pub struct OAuthCallbackCommand {
    pub tenant_id: i64,
    pub provider_code: String,
    pub code: String,
    pub state: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalIdentityLogin {
    pub user_id: i64,
    pub provider_code: String,
    pub external_subject: String,
}

#[derive(Debug, Clone)]
pub struct IdentityOAuthService {
    repository: IdentityRepository,
    http: reqwest::Client,
}

impl IdentityOAuthService {
    pub fn new(db: PgPool) -> Self {
        Self {
            repository: IdentityRepository::new(db),
            http: reqwest::Client::new(),
        }
    }

    pub async fn start_authorization(
        &self,
        command: OAuthAuthorizeCommand,
    ) -> Result<OAuthAuthorizePreview, AppError> {
        let provider = self
            .repository
            .find_provider_by_code(command.tenant_id, command.provider_code.trim())
            .await?
            .ok_or(AppError::NotFound)?;
        let provider = provider_with_runtime_client_id(provider);
        let state = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::minutes(OAUTH_STATE_TTL_MINUTES);
        let preview = build_oauth_authorize_preview(
            &provider,
            &command.redirect_uri,
            &command.requested_scopes,
            &state,
            expires_at,
        )?;
        self.repository
            .save_oauth_state(&OAuthStateSaveRecord {
                tenant_id: provider.tenant_id,
                provider_id: provider.id,
                state_hash: oauth_state_hash(&state),
                redirect_uri: command.redirect_uri.trim().to_owned(),
                requested_scopes: json!(preview.requested_scopes),
                code_verifier_hash: None,
                expires_at: preview.expires_at.naive_utc(),
                create_user: command.create_user,
            })
            .await?;

        Ok(preview)
    }

    pub async fn complete_github_callback(
        &self,
        command: OAuthCallbackCommand,
    ) -> Result<ExternalIdentityLogin, AppError> {
        let provider = self
            .repository
            .find_provider_by_code(command.tenant_id, command.provider_code.trim())
            .await?
            .ok_or(AppError::NotFound)?;
        let provider = provider_with_runtime_client_id(provider);
        let redirect_uri = command.redirect_uri.trim();
        let code = command.code.trim();
        let state = command.state.trim();
        if redirect_uri.is_empty() {
            return Err(AppError::bad_request("redirectUri不能为空"));
        }
        if code.is_empty() {
            return Err(AppError::bad_request("code不能为空"));
        }
        if state.is_empty() {
            return Err(AppError::bad_request("state不能为空"));
        }

        let consumed_state = self
            .repository
            .consume_oauth_state(
                provider.tenant_id,
                provider.id,
                &oauth_state_hash(state),
                redirect_uri,
            )
            .await?
            .ok_or_else(|| AppError::bad_request("OAuth state已失效或已使用"))?;
        let access_token = self
            .exchange_github_access_token(&provider, code, &consumed_state.redirect_uri)
            .await?;
        let profile = self.fetch_github_profile(&provider, &access_token).await?;
        let account = self
            .repository
            .find_external_account_by_subject(
                provider.tenant_id,
                provider.id,
                &profile.external_subject,
            )
            .await?
            .ok_or_else(|| AppError::bad_request("GitHub账号未绑定"))?;
        self.repository
            .touch_external_account_login(
                account.id,
                profile.display_name.as_deref(),
                profile.email.as_deref(),
                &profile.metadata,
            )
            .await?;

        Ok(ExternalIdentityLogin {
            user_id: account.user_id,
            provider_code: provider.code,
            external_subject: profile.external_subject,
        })
    }

    async fn exchange_github_access_token(
        &self,
        provider: &IdentityProviderRecord,
        code: &str,
        redirect_uri: &str,
    ) -> Result<String, AppError> {
        let client_id = provider_client_id(provider)
            .ok_or_else(|| AppError::bad_request("GitHub OAuth clientId未配置"))?;
        let client_secret = github_client_secret(provider)
            .ok_or_else(|| AppError::bad_request("GitHub OAuth clientSecret未配置"))?;
        let response = self
            .http
            .post(provider_endpoint(
                provider,
                "tokenUrl",
                DEFAULT_GITHUB_TOKEN_URL,
            ))
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id),
                ("client_secret", client_secret.as_str()),
                ("code", code),
                ("redirect_uri", redirect_uri),
            ])
            .send()
            .await
            .map_err(|err| {
                AppError::Anyhow(anyhow!("GitHub OAuth token exchange failed: {err}"))
            })?;
        let status = response.status();
        let payload = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
        if !status.is_success() {
            return Err(AppError::bad_request(format!(
                "GitHub OAuth token exchange failed: HTTP {}",
                status.as_u16()
            )));
        }
        let token = payload
            .get("access_token")
            .and_then(Value::as_str)
            .and_then(non_empty_str)
            .ok_or_else(|| AppError::bad_request("GitHub OAuth access_token缺失"))?;

        Ok(token.to_owned())
    }

    async fn fetch_github_profile(
        &self,
        provider: &IdentityProviderRecord,
        access_token: &str,
    ) -> Result<ExternalIdentityProfile, AppError> {
        let response = self
            .http
            .get(provider_endpoint(
                provider,
                "userInfoUrl",
                DEFAULT_GITHUB_USER_INFO_URL,
            ))
            .bearer_auth(access_token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "novex-github-identity-poc")
            .send()
            .await
            .map_err(|err| AppError::Anyhow(anyhow!("GitHub profile fetch failed: {err}")))?;
        let status = response.status();
        let payload = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
        if !status.is_success() {
            return Err(AppError::bad_request(format!(
                "GitHub profile fetch failed: HTTP {}",
                status.as_u16()
            )));
        }

        parse_github_user_profile(&payload)
    }
}

impl Default for OAuthAuthorizeCommand {
    fn default() -> Self {
        Self {
            tenant_id: DEFAULT_TENANT_ID,
            provider_code: String::new(),
            redirect_uri: String::new(),
            requested_scopes: Vec::new(),
            create_user: PUBLIC_CREATE_USER_ID,
        }
    }
}

pub fn build_oauth_authorize_preview(
    provider: &IdentityProviderRecord,
    redirect_uri: &str,
    requested_scopes: &[String],
    state: &str,
    expires_at: DateTime<Utc>,
) -> Result<OAuthAuthorizePreview, AppError> {
    if provider.status != 1 {
        return Err(AppError::bad_request("身份提供商已禁用"));
    }
    if !provider
        .provider_type
        .eq_ignore_ascii_case(GITHUB_PROVIDER_TYPE)
    {
        return Err(AppError::bad_request("暂不支持该身份提供商"));
    }

    let redirect_uri = redirect_uri.trim();
    if redirect_uri.is_empty() {
        return Err(AppError::bad_request("redirectUri不能为空"));
    }
    let state = state.trim();
    if state.is_empty() {
        return Err(AppError::bad_request("state不能为空"));
    }

    let Some(client_id) = provider_client_id(provider) else {
        return Err(AppError::bad_request("GitHub OAuth clientId未配置"));
    };

    let scopes = normalize_oauth_scopes(
        requested_scopes,
        provider.tenant_policy.get("defaultScopes"),
        GITHUB_DEFAULT_SCOPES,
    );
    let authorize_url = provider
        .tenant_policy
        .get("authorizationUrl")
        .and_then(Value::as_str)
        .and_then(non_empty_str)
        .unwrap_or(DEFAULT_GITHUB_AUTHORIZE_URL);
    let mut url = Url::parse(authorize_url)
        .map_err(|_| AppError::bad_request("GitHub OAuth authorizationUrl配置无效"))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", &scopes.join(" "))
        .append_pair("state", state)
        .append_pair("response_type", "code");

    Ok(OAuthAuthorizePreview {
        provider_code: provider.code.clone(),
        provider_type: provider.provider_type.clone(),
        authorization_url: url.to_string(),
        state: state.to_owned(),
        requested_scopes: scopes,
        expires_at,
    })
}

pub fn oauth_state_hash(state: &str) -> String {
    let digest = Sha256::digest(state.as_bytes());
    hex_lower(&digest)
}

pub fn parse_github_user_profile(value: &Value) -> Result<ExternalIdentityProfile, AppError> {
    let external_subject = value
        .get("id")
        .and_then(|value| {
            value
                .as_i64()
                .map(|id| id.to_string())
                .or_else(|| value.as_str().and_then(non_empty_str).map(str::to_owned))
        })
        .ok_or_else(|| AppError::bad_request("GitHub profile id is required"))?;
    let login = value
        .get("login")
        .and_then(Value::as_str)
        .and_then(non_empty_str);
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .and_then(non_empty_str);
    let email = value
        .get("email")
        .and_then(Value::as_str)
        .and_then(non_empty_str)
        .map(str::to_owned);
    let avatar_url = value
        .get("avatar_url")
        .and_then(Value::as_str)
        .and_then(non_empty_str);

    Ok(ExternalIdentityProfile {
        external_subject,
        display_name: name.or(login).map(str::to_owned),
        email,
        metadata: json!({
            "provider": GITHUB_PROVIDER_TYPE,
            "login": login.unwrap_or_default(),
            "avatarUrl": avatar_url.unwrap_or_default()
        }),
    })
}

fn normalize_oauth_scopes(
    requested_scopes: &[String],
    policy_scopes: Option<&Value>,
    default_scopes: &[&str],
) -> Vec<String> {
    let mut scopes = Vec::new();
    let requested = requested_scopes
        .iter()
        .flat_map(|scope| scope.split(','))
        .filter_map(non_empty_str);
    for scope in requested {
        push_unique_scope(&mut scopes, scope);
    }

    if scopes.is_empty() {
        if let Some(Value::Array(policy_scopes)) = policy_scopes {
            for scope in policy_scopes
                .iter()
                .filter_map(Value::as_str)
                .filter_map(non_empty_str)
            {
                push_unique_scope(&mut scopes, scope);
            }
        }
    }

    if scopes.is_empty() {
        for scope in default_scopes {
            push_unique_scope(&mut scopes, scope);
        }
    }

    scopes
}

fn provider_with_runtime_client_id(mut provider: IdentityProviderRecord) -> IdentityProviderRecord {
    if provider
        .client_id
        .as_deref()
        .and_then(non_empty_str)
        .is_none()
    {
        provider.client_id = std::env::var("GITHUB_OAUTH_CLIENT_ID")
            .ok()
            .or_else(|| std::env::var("NOVEX_GITHUB_OAUTH_CLIENT_ID").ok())
            .and_then(|value| non_empty_str(&value).map(str::to_owned));
    }
    provider
}

fn provider_client_id(provider: &IdentityProviderRecord) -> Option<&str> {
    provider.client_id.as_deref().and_then(non_empty_str)
}

fn github_client_secret(provider: &IdentityProviderRecord) -> Option<String> {
    provider
        .secret_ref
        .as_deref()
        .and_then(resolve_secret_ref)
        .or_else(|| std::env::var("GITHUB_OAUTH_CLIENT_SECRET").ok())
        .or_else(|| std::env::var("NOVEX_GITHUB_OAUTH_CLIENT_SECRET").ok())
        .and_then(|value| non_empty_str(&value).map(str::to_owned))
}

fn resolve_secret_ref(secret_ref: &str) -> Option<String> {
    let secret_ref = secret_ref.trim();
    let env_name = secret_ref.strip_prefix("env:")?.trim();
    if env_name.is_empty() {
        return None;
    }
    std::env::var(env_name).ok()
}

fn provider_endpoint<'a>(
    provider: &'a IdentityProviderRecord,
    key: &str,
    default_endpoint: &'a str,
) -> &'a str {
    provider
        .tenant_policy
        .get(key)
        .and_then(Value::as_str)
        .and_then(non_empty_str)
        .unwrap_or(default_endpoint)
}

fn push_unique_scope(scopes: &mut Vec<String>, scope: &str) {
    if !scopes.iter().any(|item| item == scope) {
        scopes.push(scope.to_owned());
    }
}

fn non_empty_str(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use serde_json::json;

    use super::*;
    use crate::infrastructure::persistence::identity_repository::IdentityProviderRecord;

    fn github_provider() -> IdentityProviderRecord {
        IdentityProviderRecord {
            id: 42,
            tenant_id: 1,
            provider_type: "github".to_owned(),
            code: "github.login".to_owned(),
            name: "GitHub Login".to_owned(),
            client_id: Some("client-123".to_owned()),
            secret_ref: Some("env:GITHUB_OAUTH_CLIENT_SECRET".to_owned()),
            allowed_domains: json!([]),
            tenant_policy: json!({
                "authorizationUrl": "https://github.com/login/oauth/authorize",
                "tokenUrl": "https://github.com/login/oauth/access_token",
                "userInfoUrl": "https://api.github.com/user",
                "defaultScopes": ["read:user", "user:email"]
            }),
            status: 1,
        }
    }

    #[test]
    fn github_authorize_url_uses_default_scopes_and_state_hash_contract() {
        let state = "state-for-test";
        let result = build_oauth_authorize_preview(
            &github_provider(),
            "https://novex.example/auth/github/callback",
            &[],
            state,
            Utc::now() + Duration::minutes(10),
        )
        .expect("authorize preview should build");

        assert_eq!(result.provider_code, "github.login");
        assert_eq!(result.provider_type, "github");
        assert_eq!(result.requested_scopes, vec!["read:user", "user:email"]);
        assert!(result
            .authorization_url
            .starts_with("https://github.com/login/oauth/authorize?"));
        assert!(result.authorization_url.contains("client_id=client-123"));
        assert!(result
            .authorization_url
            .contains("redirect_uri=https%3A%2F%2Fnovex.example%2Fauth%2Fgithub%2Fcallback"));
        assert!(result
            .authorization_url
            .contains("scope=read%3Auser+user%3Aemail"));
        assert!(result.authorization_url.contains("state=state-for-test"));
        assert_ne!(oauth_state_hash(state), state);
        assert_eq!(oauth_state_hash(state).len(), 64);
    }

    #[test]
    fn github_profile_parser_requires_subject_and_keeps_login_metadata() {
        let profile = parse_github_user_profile(&json!({
            "id": 123456,
            "login": "octocat",
            "name": "The Octocat",
            "email": "octocat@example.com",
            "avatar_url": "https://avatars.githubusercontent.com/u/123456"
        }))
        .expect("profile should parse");

        assert_eq!(profile.external_subject, "123456");
        assert_eq!(profile.display_name.as_deref(), Some("The Octocat"));
        assert_eq!(profile.email.as_deref(), Some("octocat@example.com"));
        assert_eq!(profile.metadata["login"], "octocat");
        assert_eq!(profile.metadata["provider"], "github");

        let err = parse_github_user_profile(&json!({ "login": "missing-id" })).unwrap_err();
        assert!(err.to_string().contains("GitHub profile id is required"));
    }
}
