use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};

use crate::{
    application::ai::model_service::{
        ModelChatCommand, ModelChatConversationResp, ModelChatResp, ModelHealthCheckCommand,
        ModelHealthCheckResp, ModelOpsSummaryResp, ModelRegistrySummary,
        ModelRouteCircuitBreakerResp, ModelRuntimeService,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, response::ApiResponse},
};

pub const MODEL_LIST_PERMISSION: &str = "ai:model:list";
pub const MODEL_HEALTH_PERMISSION: &str = "ai:model:healthCheck";
pub const MODEL_CHAT_PERMISSION: &str = "ai:model:chat";
pub const MODEL_CIRCUIT_BREAKER_LIST_PERMISSION: &str = "ai:model:circuitBreaker:list";
pub const MODEL_CIRCUIT_BREAKER_CLEAR_PERMISSION: &str = "ai:model:circuitBreaker:clear";
pub const MODEL_OPS_SUMMARY_PERMISSION: &str = "ai:model:opsSummary";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/models/runtime-config", get(runtime_config))
        .route("/ai/models/registry", get(model_registry))
        .route("/ai/models/ops-summary", get(model_ops_summary))
        .route("/ai/models/health-check", post(health_check))
        .route(
            "/ai/models/route-circuit-breakers",
            get(list_route_circuit_breakers),
        )
        .route(
            "/ai/models/route-circuit-breakers/:route_id",
            delete(clear_route_circuit_breaker),
        )
        .route(
            "/ai/models/chat/conversations",
            get(list_chat_conversations),
        )
        .route("/ai/models/chat", post(chat_completion))
}

async fn runtime_config(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<novex_model::ModelRuntimeSummary>>, AppError> {
    require_permission(&current_user, MODEL_LIST_PERMISSION)?;
    let service = ModelRuntimeService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.effective_runtime_summary().await?,
    )))
}

async fn model_registry(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<ModelRegistrySummary>>, AppError> {
    require_permission(&current_user, MODEL_LIST_PERMISSION)?;

    Ok(Json(ApiResponse::ok(
        ModelRuntimeService::registry_summary_for_tenant(&state.db, current_user.tenant_id).await?,
    )))
}

async fn health_check(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<ModelHealthCheckCommand>,
) -> Result<Json<ApiResponse<ModelHealthCheckResp>>, AppError> {
    require_permission(&current_user, MODEL_HEALTH_PERMISSION)?;
    let service = ModelRuntimeService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.health_check_for_tenant(command).await?,
    )))
}

async fn model_ops_summary(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<ModelOpsSummaryResp>>, AppError> {
    require_permission(&current_user, MODEL_OPS_SUMMARY_PERMISSION)?;
    let service = ModelRuntimeService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.model_ops_summary().await?)))
}

async fn list_route_circuit_breakers(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<ModelRouteCircuitBreakerResp>>>, AppError> {
    require_permission(&current_user, MODEL_CIRCUIT_BREAKER_LIST_PERMISSION)?;
    let service = ModelRuntimeService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.list_route_circuit_breakers().await?,
    )))
}

async fn clear_route_circuit_breaker(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(route_id): Path<String>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    require_permission(&current_user, MODEL_CIRCUIT_BREAKER_CLEAR_PERMISSION)?;
    let service = ModelRuntimeService::for_tenant(state.db, current_user.tenant_id);

    service.clear_route_circuit_breaker(&route_id).await?;

    Ok(Json(ApiResponse::ok(())))
}

async fn chat_completion(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<ModelChatCommand>,
) -> Result<Json<ApiResponse<ModelChatResp>>, AppError> {
    require_permission(&current_user, MODEL_CHAT_PERMISSION)?;
    let service = ModelRuntimeService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service
            .chat_completion_with_usage(current_user.id, command)
            .await?,
    )))
}

async fn list_chat_conversations(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<ModelChatConversationResp>>>, AppError> {
    require_permission(&current_user, MODEL_CHAT_PERMISSION)?;
    let service = ModelRuntimeService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.list_chat_conversations(current_user.id).await?,
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

    fn test_state() -> AppState {
        AppState {
            db: PgPoolOptions::new()
                .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
                .unwrap(),
            jwt: JwtService::new("test-secret".to_owned(), 24),
            captcha: Default::default(),
            agent_runtime: Default::default(),
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
    fn model_circuit_breaker_permission_seed_contains_controls() {
        let seed = include_str!(
            "../../../../migrations/202606170002_seed_ai_model_circuit_breaker_permissions.sql"
        );

        assert!(seed.contains(MODEL_CIRCUIT_BREAKER_LIST_PERMISSION));
        assert!(seed.contains(MODEL_CIRCUIT_BREAKER_CLEAR_PERMISSION));
    }

    #[test]
    fn model_ops_summary_permission_seed_contains_control() {
        let seed =
            include_str!("../../../../migrations/202606170003_seed_ai_model_ops_summary_permission.sql");

        assert!(seed.contains(MODEL_OPS_SUMMARY_PERMISSION));
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

    #[test]
    fn model_route_circuit_breaker_migration_defines_runtime_state_table() {
        let migration = include_str!(
            "../../../../migrations/202606170001_create_ai_model_route_circuit_breaker.sql"
        );

        for required in [
            "CREATE TABLE IF NOT EXISTS ai_model_route_circuit_breaker",
            "tenant_id",
            "route_id",
            "opened_until",
            "open_reason",
            "last_error_kind",
            "last_http_status",
            "uk_ai_model_route_circuit_breaker_tenant_route",
            "idx_ai_model_route_circuit_breaker_opened_until",
        ] {
            assert!(migration.contains(required), "missing {required}");
        }
    }

    #[test]
    fn model_chat_handlers_bind_runtime_to_current_tenant() {
        let source = include_str!("model.rs");

        assert!(
            source
                .matches("ModelRuntimeService::for_tenant(state.db, current_user.tenant_id)")
                .count()
                >= 4
        );
    }

    #[test]
    fn model_runtime_config_and_health_handlers_use_tenant_routes() {
        let source = include_str!("model.rs");

        assert!(source.contains(".effective_runtime_summary().await?"));
        assert!(source.contains(".health_check_for_tenant(command).await?"));
    }

    #[tokio::test]
    async fn runtime_config_handler_rejects_missing_permission() {
        let err = runtime_config(State(test_state()), user_with_permissions(vec![]))
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn health_check_handler_rejects_missing_permission_before_network() {
        let err = health_check(
            State(test_state()),
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
            State(test_state()),
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
    async fn model_chat_conversation_list_handler_rejects_missing_permission() {
        let err = list_chat_conversations(State(test_state()), user_with_permissions(vec![]))
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn model_circuit_breaker_list_handler_rejects_missing_permission() {
        let err = list_route_circuit_breakers(State(test_state()), user_with_permissions(vec![]))
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn model_circuit_breaker_clear_handler_rejects_missing_permission() {
        let err = clear_route_circuit_breaker(
            State(test_state()),
            user_with_permissions(vec![]),
            axum::extract::Path("runtime.llm.code_agent".to_owned()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn model_ops_summary_handler_rejects_missing_permission() {
        let err = model_ops_summary(State(test_state()), user_with_permissions(vec![]))
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[test]
    fn model_circuit_breaker_routes_are_registered() {
        let source = include_str!("model.rs");

        assert!(source.contains("/ai/models/route-circuit-breakers"));
        assert!(source.contains("/ai/models/route-circuit-breakers/:route_id"));
    }

    #[test]
    fn model_ops_summary_route_is_registered() {
        let source = include_str!("model.rs");

        assert!(source.contains("/ai/models/ops-summary"));
        assert!(source.contains("MODEL_OPS_SUMMARY_PERMISSION"));
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
