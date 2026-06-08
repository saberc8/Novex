use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap},
    routing::{get, post},
    Json, Router,
};

use crate::{
    application::ai::integration_service::{
        ApiKeyCommand, ApiKeyResp, IntegrationQuery, IntegrationService, OpenApiInvokeCommand,
        OpenApiInvokeResp, PublicLinkCommand, PublicLinkResp, PublicShareResp,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, pagination::PageResult, response::ApiResponse},
};

const INTEGRATION_LIST_PERMISSION: &str = "ai:integration:list";
const INTEGRATION_CREATE_PERMISSION: &str = "ai:integration:create";
const INTEGRATION_REVOKE_PERMISSION: &str = "ai:integration:revoke";
const X_API_KEY_HEADER: &str = "x-api-key";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/openapi/invoke", post(openapi_invoke))
        .route("/share/:token", get(resolve_public_share))
        .route(
            "/ai/integrations/api-keys",
            get(list_api_keys).post(create_api_key),
        )
        .route("/ai/integrations/api-keys/:id/revoke", post(revoke_api_key))
        .route(
            "/ai/integrations/public-links",
            get(list_public_links).post(create_public_link),
        )
        .route(
            "/ai/integrations/public-links/:id/revoke",
            post(revoke_public_link),
        )
}

async fn openapi_invoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(command): Json<OpenApiInvokeCommand>,
) -> Result<Json<ApiResponse<OpenApiInvokeResp>>, AppError> {
    let api_key = api_key_from_headers(&headers)?;
    let service = IntegrationService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service.invoke_openapi(&api_key, command).await?,
    )))
}

async fn resolve_public_share(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<ApiResponse<PublicShareResp>>, AppError> {
    let service = IntegrationService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service.resolve_public_share(&token).await?,
    )))
}

async fn list_api_keys(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<IntegrationQuery>,
) -> Result<Json<ApiResponse<PageResult<ApiKeyResp>>>, AppError> {
    require_permission(&current_user, INTEGRATION_LIST_PERMISSION)?;
    let service = IntegrationService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service.list_api_keys(current_user.tenant_id, query).await?,
    )))
}

async fn create_api_key(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<ApiKeyCommand>,
) -> Result<Json<ApiResponse<ApiKeyResp>>, AppError> {
    require_permission(&current_user, INTEGRATION_CREATE_PERMISSION)?;
    let service = IntegrationService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .create_api_key(current_user.tenant_id, current_user.id, command)
            .await?,
    )))
}

async fn revoke_api_key(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    require_permission(&current_user, INTEGRATION_REVOKE_PERMISSION)?;
    let service = IntegrationService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .revoke_api_key(current_user.tenant_id, current_user.id, id)
            .await?,
    )))
}

async fn list_public_links(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<IntegrationQuery>,
) -> Result<Json<ApiResponse<PageResult<PublicLinkResp>>>, AppError> {
    require_permission(&current_user, INTEGRATION_LIST_PERMISSION)?;
    let service = IntegrationService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .list_public_links(current_user.tenant_id, query)
            .await?,
    )))
}

async fn create_public_link(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<PublicLinkCommand>,
) -> Result<Json<ApiResponse<PublicLinkResp>>, AppError> {
    require_permission(&current_user, INTEGRATION_CREATE_PERMISSION)?;
    let service = IntegrationService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .create_public_link(current_user.tenant_id, current_user.id, command)
            .await?,
    )))
}

async fn revoke_public_link(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    require_permission(&current_user, INTEGRATION_REVOKE_PERMISSION)?;
    let service = IntegrationService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .revoke_public_link(current_user.tenant_id, current_user.id, id)
            .await?,
    )))
}

fn api_key_from_headers(headers: &HeaderMap) -> Result<String, AppError> {
    if let Some(value) = headers
        .get(X_API_KEY_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(value.to_owned());
    }

    if let Some(value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().split_once(' '))
        .and_then(|(scheme, token)| {
            scheme
                .eq_ignore_ascii_case("bearer")
                .then_some(token.trim())
        })
        .filter(|value| !value.is_empty())
    {
        return Ok(value.to_owned());
    }

    Err(AppError::Unauthorized)
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        extract::State,
        http::{header, Request, StatusCode},
        Json,
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::*;
    use crate::{
        domain::auth::model::{CurrentUser, RoleContext},
        infrastructure::security::jwt::JwtService,
        interfaces::http::{build_router, AppState},
        shared::error::AppError,
    };

    fn test_state() -> AppState {
        AppState {
            db: PgPoolOptions::new()
                .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
                .unwrap(),
            jwt: JwtService::new("test-secret".to_owned(), 24),
            captcha: Default::default(),
            scheduler_http_safety: Default::default(),
            parser_callback_token: None,
            parser_callback_user_id: 1,
        }
    }

    fn user_with_permissions(permissions: Vec<&str>) -> CurrentUser {
        CurrentUser {
            id: 1,
            tenant_id: 1,
            username: "tester".to_owned(),
            dept_id: 1,
            roles: vec![RoleContext {
                id: 2,
                name: "普通用户".to_owned(),
                code: "general".to_owned(),
                data_scope: 4,
            }],
            permissions: permissions.into_iter().map(str::to_owned).collect(),
        }
    }

    #[test]
    fn integration_permission_seed_contains_route_permissions() {
        let seed = include_str!("../../../../migrations/202606050001_seed_ai_foundation_menus.sql");

        for permission in [
            INTEGRATION_LIST_PERMISSION,
            INTEGRATION_CREATE_PERMISSION,
            INTEGRATION_REVOKE_PERMISSION,
        ] {
            assert!(seed.contains(permission), "{permission} missing from seed");
        }
    }

    #[tokio::test]
    async fn api_key_create_handler_rejects_missing_permission() {
        let err = create_api_key(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(ApiKeyCommand {
                app_id: "training_app".to_owned(),
                name: "Training API".to_owned(),
                permission_scope: vec!["app:training:ask".to_owned()],
                qps_limit: 5,
                quota_limit: 1000,
                expires_at: None,
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[test]
    fn openapi_api_key_header_accepts_x_api_key_and_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert(X_API_KEY_HEADER, " nxk_live_from_header ".parse().unwrap());
        assert_eq!(
            api_key_from_headers(&headers).unwrap(),
            "nxk_live_from_header"
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            "Bearer nxk_live_from_bearer".parse().unwrap(),
        );
        assert_eq!(
            api_key_from_headers(&headers).unwrap(),
            "nxk_live_from_bearer"
        );
    }

    #[tokio::test]
    async fn integration_routes_are_registered_and_require_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/integrations/api-keys")
                    .header(header::ACCEPT, "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice::<Value>(&body).unwrap();
        assert_eq!(body["code"], "401");
    }

    #[tokio::test]
    async fn openapi_invoke_route_is_registered_and_rejects_missing_api_key() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/openapi/invoke")
                    .header(header::ACCEPT, "application/json")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"appId":"training_app","operation":"training.ask","input":{"question":"hi"}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice::<Value>(&body).unwrap();
        assert_eq!(body["code"], "401");
    }

    #[tokio::test]
    async fn public_share_route_is_registered_and_rejects_invalid_token_before_database_lookup() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/share/not-a-token")
                    .header(header::ACCEPT, "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice::<Value>(&body).unwrap();
        assert_eq!(body["code"], "401");
    }
}
