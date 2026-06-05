use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};

use crate::{
    application::ai::model_service::{
        ModelChatCommand, ModelChatResp, ModelHealthCheckCommand, ModelHealthCheckResp,
        ModelRegistrySummary, ModelRuntimeService,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, response::ApiResponse},
};

pub const MODEL_LIST_PERMISSION: &str = "ai:model:list";
pub const MODEL_HEALTH_PERMISSION: &str = "ai:model:healthCheck";
pub const MODEL_CHAT_PERMISSION: &str = "ai:model:chat";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/models/runtime-config", get(runtime_config))
        .route("/ai/models/registry", get(model_registry))
        .route("/ai/models/health-check", post(health_check))
        .route("/ai/models/chat", post(chat_completion))
}

async fn runtime_config(
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<novex_model::ModelRuntimeSummary>>, AppError> {
    require_permission(&current_user, MODEL_LIST_PERMISSION)?;

    Ok(Json(ApiResponse::ok(ModelRuntimeService::runtime_config())))
}

async fn model_registry(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<ModelRegistrySummary>>, AppError> {
    require_permission(&current_user, MODEL_LIST_PERMISSION)?;

    Ok(Json(ApiResponse::ok(
        ModelRuntimeService::registry_summary(&state.db).await?,
    )))
}

async fn health_check(
    current_user: CurrentUser,
    Json(command): Json<ModelHealthCheckCommand>,
) -> Result<Json<ApiResponse<ModelHealthCheckResp>>, AppError> {
    require_permission(&current_user, MODEL_HEALTH_PERMISSION)?;

    Ok(Json(ApiResponse::ok(
        ModelRuntimeService::health_check(command).await?,
    )))
}

async fn chat_completion(
    current_user: CurrentUser,
    Json(command): Json<ModelChatCommand>,
) -> Result<Json<ApiResponse<ModelChatResp>>, AppError> {
    require_permission(&current_user, MODEL_CHAT_PERMISSION)?;

    Ok(Json(ApiResponse::ok(
        ModelRuntimeService::chat_completion(command).await?,
    )))
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
        application::ai::model_service::ModelChatMessage,
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

    #[test]
    fn model_runtime_permission_seed_contains_route_permissions() {
        let seed = include_str!(
            "../../../../migrations/202606050013_seed_ai_model_runtime_permissions.sql"
        );
        let chat_seed =
            include_str!("../../../../migrations/202606050019_seed_ai_model_chat_permission.sql");

        assert!(seed.contains(MODEL_LIST_PERMISSION));
        assert!(seed.contains(MODEL_HEALTH_PERMISSION));
        assert!(chat_seed.contains(MODEL_CHAT_PERMISSION));
    }

    #[test]
    fn model_registry_migration_contains_required_tables_and_fields() {
        let migration =
            include_str!("../../../../migrations/202606050015_create_ai_model_registry.sql");
        let sanitize_migration = include_str!(
            "../../../../migrations/202606050016_sanitize_model_registry_masked_credentials.sql"
        );

        for table in [
            "ai_model_provider",
            "ai_model_deployment",
            "ai_model_profile",
            "ai_model_credential",
            "ai_model_route",
            "ai_model_health_check",
            "ai_model_usage",
        ] {
            assert!(migration.contains(table), "{table} missing from migration");
        }

        for field in [
            "provider_type",
            "endpoint",
            "network_zone",
            "credential_ref",
            "model_kind",
            "capabilities",
            "limits",
            "embedding_spec",
            "rerank_spec",
            "cost_spec",
            "fallback_policy",
            "route_purpose",
            "model_profile_id",
            "latency_ms",
            "cost_cents",
        ] {
            assert!(migration.contains(field), "{field} missing from migration");
        }

        assert!(
            !migration.contains("sk-"),
            "model registry migration must not seed raw API keys"
        );
        assert!(sanitize_migration.contains("masked_value = 'configured'"));
        assert!(sanitize_migration.contains("masked_value LIKE 'env:%'"));
    }

    #[tokio::test]
    async fn runtime_config_handler_rejects_missing_permission() {
        let err = runtime_config(user_with_permissions(vec![]))
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn health_check_handler_rejects_missing_permission_before_network() {
        let err = health_check(
            user_with_permissions(vec![]),
            axum::Json(ModelHealthCheckCommand {
                target: Some("all".to_owned()),
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn model_chat_handler_rejects_missing_permission_before_network() {
        let err = chat_completion(
            user_with_permissions(vec![]),
            axum::Json(ModelChatCommand {
                messages: vec![ModelChatMessage {
                    role: "user".to_owned(),
                    content: "hello".to_owned(),
                }],
                ..ModelChatCommand::default()
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn model_runtime_routes_are_registered_and_require_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/models/runtime-config")
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
    async fn model_registry_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/models/registry")
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
    async fn model_chat_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/models/chat")
                    .header(header::ACCEPT, "application/json")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"messages":[{"role":"user","content":"hello"}]}"#,
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
}
