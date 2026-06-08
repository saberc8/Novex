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

    #[tokio::test]
    async fn integration_route_is_registered_and_requires_auth() {
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

    #[test]
    fn foundation_control_plane_migration_contains_required_tables() {
        let migration =
            include_str!("../../../../migrations/202606050014_create_foundation_control_plane.sql");

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
            "ai_api_key",
            "ai_public_link",
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
            "key_hash",
            "masked_key",
            "permission_scope",
            "qps_limit",
            "quota_limit",
            "token_hash",
            "public_url",
        ] {
            assert!(migration.contains(field), "{field} missing from migration");
        }
    }

    #[test]
    fn local_poc_compose_declares_foundation_runtime_services() {
        let compose = include_str!("../../../../../infra/docker-compose.yml");

        for service in [
            "postgres:",
            "etcd:",
            "minio:",
            "milvus:",
            "rabbitmq:",
            "redis:",
            "backend:",
            "parser-worker:",
            "admin:",
            "training-web:",
        ] {
            assert!(compose.contains(service), "{service} missing from compose");
        }

        for contract in [
            "${POSTGRES_IMAGE:-postgres:16-alpine}",
            "${ETCD_IMAGE:-quay.io/coreos/etcd:v3.5.18}",
            "${MINIO_IMAGE:-minio/minio:RELEASE.2025-01-20T14-49-07Z}",
            "${MILVUS_IMAGE:-milvusdb/milvus:v2.5.4}",
            "${RABBITMQ_IMAGE:-rabbitmq:4.0-management-alpine}",
            "${REDIS_IMAGE:-redis:7-alpine}",
            "${RUST_IMAGE:-rust:1.85-bookworm}",
            "${NODE_IMAGE:-node:24-bookworm-slim}",
            "${POSTGRES_PORT:-5432}:5432",
            "${MINIO_API_PORT:-9000}:9000",
            "${MINIO_CONSOLE_PORT:-9001}:9001",
            "${MILVUS_PORT:-19530}:19530",
            "${MILVUS_METRICS_PORT:-9091}:9091",
            "${RABBITMQ_AMQP_PORT:-5672}:5672",
            "${RABBITMQ_MANAGEMENT_PORT:-15672}:15672",
            "${REDIS_PORT:-6379}:6379",
            "${BACKEND_PORT:-4398}:4398",
            "${ADMIN_PORT:-4399}:4399",
            "${TRAINING_WEB_PORT:-4401}:4401",
            "${CHAT_WEB_PORT:-4402}:4402",
            "${AGENT_WORKSPACE_PORT:-4403}:4403",
            "DB_AUTO_MIGRATE",
            "AUTH_JWT_SECRET",
            "MILVUS_ENDPOINT",
            "PARSER_QUEUE_ENABLED",
            "PARSER_QUEUE_PUBLISHER_ENABLED",
            "PARSER_CALLBACK_TOKEN",
            "RABBITMQ_URL",
            "REDIS_URL",
            "RABBITMQ_PARSER_EXCHANGE",
            "RABBITMQ_PARSER_EXECUTE_QUEUE",
            "python3 -m parser_worker.worker",
            "NEXT_PUBLIC_API_BASE_URL",
        ] {
            assert!(
                compose.contains(contract),
                "{contract} missing from compose"
            );
        }
    }
}
