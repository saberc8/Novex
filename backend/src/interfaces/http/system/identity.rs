use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};

use crate::{
    application::identity::provider_service::{
        ExternalAccountResp, IdentityPolicyResp, IdentityProviderResp, IdentityProviderService,
        IdentityResourceQuery,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, pagination::PageResult, response::ApiResponse},
};

pub const IDENTITY_PROVIDER_LIST_PERMISSION: &str = "system:identityProvider:list";
pub const EXTERNAL_ACCOUNT_LIST_PERMISSION: &str = "system:externalAccount:list";
pub const IDENTITY_POLICY_LIST_PERMISSION: &str = "system:identityPolicy:list";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/system/identity/providers", get(list_providers))
        .route("/system/identity/accounts", get(list_accounts))
        .route("/system/identity/policies", get(list_policies))
}

async fn list_providers(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<IdentityResourceQuery>,
) -> Result<Json<ApiResponse<PageResult<IdentityProviderResp>>>, AppError> {
    require_permission(&current_user, IDENTITY_PROVIDER_LIST_PERMISSION)?;
    let service = IdentityProviderService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.list_providers(query).await?)))
}

async fn list_accounts(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<IdentityResourceQuery>,
) -> Result<Json<ApiResponse<PageResult<ExternalAccountResp>>>, AppError> {
    require_permission(&current_user, EXTERNAL_ACCOUNT_LIST_PERMISSION)?;
    let service = IdentityProviderService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.list_accounts(query).await?)))
}

async fn list_policies(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<IdentityResourceQuery>,
) -> Result<Json<ApiResponse<PageResult<IdentityPolicyResp>>>, AppError> {
    require_permission(&current_user, IDENTITY_POLICY_LIST_PERMISSION)?;
    let service = IdentityProviderService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.list_policies(query).await?)))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        extract::{Query, State},
        http::{header, Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::*;
    use crate::{
        application::identity::provider_service::IdentityResourceQuery,
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

    #[tokio::test]
    async fn identity_provider_list_handler_rejects_missing_permission() {
        let err = list_providers(
            State(test_state()),
            user_with_permissions(vec![]),
            Query(IdentityResourceQuery::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn identity_provider_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/system/identity/providers")
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
