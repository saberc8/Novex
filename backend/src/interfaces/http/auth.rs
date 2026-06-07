use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    application::auth::service::{
        AuthService, CurrentUserDetails, LoginCommand, LoginMeta, LoginResult,
    },
    application::identity::oauth_service::{
        IdentityOAuthService, OAuthAuthorizeCommand, OAuthAuthorizePreview, OAuthCallbackCommand,
    },
    application::monitor::online_service::OnlineService,
    application::rbac::service::RbacService,
    domain::auth::model::{CurrentUser, RoleContext},
    domain::rbac::model::RouteItem,
    interfaces::http::middleware::access_log::{
        bearer_token_value, browser_name, client_ip, os_name,
    },
    shared::{error::AppError, response::ApiResponse},
};

use super::{captcha, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/oauth/:provider/authorize", get(oauth_authorize))
        .route("/auth/oauth/:provider/callback", post(oauth_callback))
        .route("/auth/logout", post(logout))
        .route("/auth/user/info", get(user_info))
        .route("/auth/user/route", get(user_route))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginReq {
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    auth_type: Option<String>,
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    captcha: Option<String>,
    #[serde(default)]
    captcha_key: Option<String>,
    #[serde(default)]
    uuid: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OAuthAuthorizeQuery {
    #[serde(default)]
    redirect_uri: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    scopes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OAuthCallbackReq {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    redirect_uri: Option<String>,
    #[serde(default)]
    client_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginResp {
    token: String,
    expire: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OAuthAuthorizeResp {
    provider_code: String,
    provider_type: String,
    authorization_url: String,
    state: String,
    requested_scopes: Vec<String>,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserInfoResp {
    id: i64,
    username: String,
    nickname: String,
    gender: i16,
    email: String,
    phone: String,
    avatar: String,
    description: String,
    pwd_reset_time: String,
    pwd_expired: bool,
    registration_date: String,
    dept_name: String,
    roles: Vec<String>,
    permissions: Vec<String>,
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LoginReq>,
) -> Result<Json<ApiResponse<LoginResp>>, AppError> {
    captcha::ensure_login_captcha(
        &state,
        req.uuid.as_deref().or(req.captcha_key.as_deref()),
        req.captcha.as_deref(),
    )
    .await?;

    let service = AuthService::new(state.db, state.jwt);
    let meta = LoginMeta {
        ip: client_ip(&headers),
        browser: browser_name(&headers),
        os: os_name(&headers),
    };
    let result = service.login(req.into(), meta).await?;

    Ok(Json(ApiResponse::ok(LoginResp::from(result))))
}

async fn oauth_authorize(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(query): Query<OAuthAuthorizeQuery>,
) -> Result<Json<ApiResponse<OAuthAuthorizeResp>>, AppError> {
    let redirect_uri = require_non_empty(query.redirect_uri.as_deref(), "redirectUri不能为空")?;
    let service = IdentityOAuthService::new(state.db);
    let result = service
        .start_authorization(OAuthAuthorizeCommand {
            tenant_id: 1,
            provider_code: provider,
            redirect_uri: redirect_uri.to_owned(),
            requested_scopes: oauth_scopes_from_query(&query),
            create_user: 1,
        })
        .await?;

    Ok(Json(ApiResponse::ok(OAuthAuthorizeResp::from(result))))
}

async fn oauth_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(provider): Path<String>,
    Json(req): Json<OAuthCallbackReq>,
) -> Result<Json<ApiResponse<LoginResp>>, AppError> {
    let code = require_non_empty(req.code.as_deref(), "code不能为空")?;
    let state_value = require_non_empty(req.state.as_deref(), "state不能为空")?;
    let redirect_uri = require_non_empty(req.redirect_uri.as_deref(), "redirectUri不能为空")?;
    let identity_service = IdentityOAuthService::new(state.db.clone());
    let identity_login = identity_service
        .complete_github_callback(OAuthCallbackCommand {
            tenant_id: 1,
            provider_code: provider,
            code: code.to_owned(),
            state: state_value.to_owned(),
            redirect_uri: redirect_uri.to_owned(),
        })
        .await?;
    let auth_service = AuthService::new(state.db, state.jwt);
    let meta = LoginMeta {
        ip: client_ip(&headers),
        browser: browser_name(&headers),
        os: os_name(&headers),
    };
    let result = auth_service
        .login_external_account(
            identity_login.user_id,
            &identity_login.provider_code,
            req.client_id,
            meta,
        )
        .await?;

    Ok(Json(ApiResponse::ok(LoginResp::from(result))))
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    _current_user: CurrentUser,
) -> Result<Json<ApiResponse<()>>, AppError> {
    if let Some(token) = bearer_token_value(&headers) {
        OnlineService::new(state.db).kickout(token).await?;
    }

    Ok(Json(ApiResponse::ok(())))
}

async fn user_info(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<UserInfoResp>>, AppError> {
    let service = AuthService::new(state.db, state.jwt);
    let details = service.current_user_details(current_user.id).await?;

    Ok(Json(ApiResponse::ok(UserInfoResp::from(details))))
}

async fn user_route(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<RouteItem>>>, AppError> {
    let service = RbacService::new(state.db);
    let routes = service.route_tree(&current_user).await?;

    Ok(Json(ApiResponse::ok(routes)))
}

impl From<LoginReq> for LoginCommand {
    fn from(req: LoginReq) -> Self {
        Self {
            username: req.username,
            password: req.password,
            auth_type: req.auth_type,
            client_id: req.client_id,
            captcha: req.captcha,
            captcha_key: req.captcha_key,
            uuid: req.uuid,
        }
    }
}

impl From<LoginResult> for LoginResp {
    fn from(result: LoginResult) -> Self {
        Self {
            token: result.token,
            expire: result.expire,
        }
    }
}

impl From<OAuthAuthorizePreview> for OAuthAuthorizeResp {
    fn from(result: OAuthAuthorizePreview) -> Self {
        Self {
            provider_code: result.provider_code,
            provider_type: result.provider_type,
            authorization_url: result.authorization_url,
            state: result.state,
            requested_scopes: result.requested_scopes,
            expires_at: result.expires_at,
        }
    }
}

impl From<CurrentUserDetails> for UserInfoResp {
    fn from(details: CurrentUserDetails) -> Self {
        let user = details.user;
        Self {
            id: user.id,
            username: user.username,
            nickname: user.nickname,
            gender: user.gender,
            email: user.email.unwrap_or_default(),
            phone: user.phone.unwrap_or_default(),
            avatar: user.avatar.unwrap_or_default(),
            description: user.description.unwrap_or_default(),
            pwd_reset_time: format_optional_datetime(user.pwd_reset_time),
            pwd_expired: false,
            registration_date: user.create_time.format("%Y-%m-%d").to_string(),
            dept_name: user.dept_name,
            roles: role_codes(details.roles),
            permissions: details.permissions,
        }
    }
}

fn role_codes(roles: Vec<RoleContext>) -> Vec<String> {
    roles.into_iter().map(|role| role.code).collect()
}

fn format_optional_datetime(value: Option<NaiveDateTime>) -> String {
    value
        .map(|time| time.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

fn require_non_empty<'a>(value: Option<&'a str>, message: &str) -> Result<&'a str, AppError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err(AppError::bad_request(message));
    };
    Ok(value)
}

fn oauth_scopes_from_query(query: &OAuthAuthorizeQuery) -> Vec<String> {
    query
        .scope
        .iter()
        .chain(query.scopes.iter())
        .flat_map(|value| value.split([',', ' ']))
        .filter_map(|scope| {
            let scope = scope.trim();
            if scope.is_empty() {
                None
            } else {
                Some(scope.to_owned())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{header, Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::*;
    use crate::{infrastructure::security::jwt::JwtService, interfaces::http::build_router};

    fn test_app() -> Router {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap()
    }

    #[tokio::test]
    async fn oauth_authorize_route_rejects_missing_redirect_before_database_lookup() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/auth/oauth/github.login/authorize")
                    .header(header::ACCEPT, "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice::<Value>(&body).unwrap();
        assert_eq!(body["code"], "400");
        assert_eq!(body["msg"], "redirectUri不能为空");
    }

    #[tokio::test]
    async fn oauth_callback_route_rejects_missing_code_and_state_before_database_lookup() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/oauth/github.login/callback")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"redirectUri":"https://novex.example/callback"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice::<Value>(&body).unwrap();
        assert_eq!(body["code"], "400");
        assert_eq!(body["msg"], "code不能为空");
    }
}
