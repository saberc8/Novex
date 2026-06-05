use axum::{routing::get, Json, Router};

use crate::{
    application::ai::foundation_service::{FoundationService, FoundationSummary},
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, response::ApiResponse},
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/ai/foundation/summary", get(summary))
}

async fn summary(
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<FoundationSummary>>, AppError> {
    require_permission(&current_user, "ai:foundation:read")?;

    Ok(Json(ApiResponse::ok(FoundationService::summary())))
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
    use crate::{
        domain::auth::model::{CurrentUser, RoleContext},
        infrastructure::security::jwt::JwtService,
        interfaces::http::build_router,
        shared::error::AppError,
    };

    fn user_with_permissions(permissions: Vec<&str>) -> CurrentUser {
        CurrentUser {
            id: 1,
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
    async fn summary_handler_returns_foundation_metadata_with_permission() {
        let response = summary(user_with_permissions(vec!["ai:foundation:read"]))
            .await
            .unwrap();

        assert_eq!(response.0.code, "200");
        assert!(response
            .0
            .data
            .modules
            .iter()
            .any(|module| module.id == "novex-model"));
    }

    #[tokio::test]
    async fn summary_handler_rejects_missing_permission() {
        let err = summary(user_with_permissions(vec![])).await.unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn summary_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/foundation/summary")
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

    #[test]
    fn foundation_control_plane_migration_contains_required_tables() {
        let migration = include_str!(
            "../../../../migrations/202606050014_create_foundation_control_plane.sql"
        );

        for table in [
            "sys_tenant",
            "sys_tenant_user",
            "sys_tenant_role",
            "sys_member_group",
            "sys_member_group_user",
            "sys_resource_permission",
            "sys_quota_policy",
            "sys_usage_meter",
            "sys_rate_limit_policy",
            "sys_identity_provider",
            "sys_external_account",
            "sys_oauth_state",
            "sys_secret",
        ] {
            assert!(migration.contains(table), "{table} missing from migration");
        }

        for field in [
            "tenant_id",
            "resource_type",
            "subject_type",
            "permission_value",
            "scope_type",
            "ciphertext",
            "masked_value",
            "key_version",
            "quota_limit",
            "usage_value",
            "window_seconds",
        ] {
            assert!(migration.contains(field), "{field} missing from migration");
        }
    }
}
