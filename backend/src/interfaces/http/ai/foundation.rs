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
        let app = build_router(db, &["http://localhost:62602".to_owned()], jwt).unwrap();

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
        let app = build_router(db, &["http://localhost:62602".to_owned()], jwt).unwrap();

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
    fn local_poc_contract_uses_common_infra_and_local_processes() {
        let readme = include_str!("../../../../../README.md");
        let env = include_str!("../../../../../.env.example");
        let backend_env = include_str!("../../../../../backend/.env.example");
        let parser_env = include_str!("../../../../../services/parser-worker/.env.example");
        let admin_env = include_str!("../../../../../admin/.env.example");
        let training_env = include_str!("../../../../../apps/training-web/.env.example");
        let chat_env = include_str!("../../../../../apps/notebooklm/.env.example");
        let agent_env = include_str!("../../../../../apps/agent-workspace/.env.example");
        let codex_env = include_str!("../../../../../apps/codex-app-poc/.env.example");

        for contract in [
            "docker-common",
            "cargo run -p backend",
            "cargo run -p backend --bin eval_worker",
            "uv run --no-project --with-requirements services/parser-worker/requirements.txt",
            "services/parser-worker/.venv/bin/python -m parser_worker.worker",
            "pnpm dev",
            "NEXT_PUBLIC_API_BASE_URL",
            "根目录 `.env`",
            "根目录 `.env.example`",
            "backend/.env.example",
            "services/parser-worker/.env.example",
            "apps/codex-app-poc/.env.example",
        ] {
            assert!(
                readme.contains(contract),
                "{contract} missing from local POC docs"
            );
        }

        for contract in [
            "DATABASE_URL=postgres://postgres:postgres@127.0.0.1:15432/novex",
            "RABBITMQ_URL=amqp://guest:guest@127.0.0.1:5673/%2f",
            "REDIS_URL=redis://127.0.0.1:16379/0",
            "MILVUS_ENDPOINT=http://127.0.0.1:19540",
            "MINIO_ENDPOINT=http://127.0.0.1:19010",
            "LOGIN_CAPTCHA_ENABLED=false",
            "PARSER_QUEUE_ENABLED=true",
            "PARSER_QUEUE_PUBLISHER_ENABLED=true",
        ] {
            assert!(
                env.contains(contract),
                "{contract} missing from POC env schema"
            );
        }

        for contract in [
            "DATABASE_URL=postgres://postgres:postgres@127.0.0.1:15432/novex",
            "AUTH_JWT_SECRET=local-dev-only-change-this-secret-32chars-min",
            "LLM_API_KEY=",
        ] {
            assert!(
                backend_env.contains(contract),
                "{contract} missing from backend env schema"
            );
        }

        for contract in [
            "PARSER_BACKEND_BASE_URL=http://127.0.0.1:62601",
            "RABBITMQ_PARSER_EXECUTE_QUEUE=novex.parser.execute",
            "MINERU_TOKEN=",
        ] {
            assert!(
                parser_env.contains(contract),
                "{contract} missing from parser-worker env schema"
            );
        }

        for frontend_env in [admin_env, training_env, chat_env, agent_env, codex_env] {
            assert!(
                frontend_env.contains("NEXT_PUBLIC_API_BASE_URL=http://localhost:62601"),
                "frontend env schema should declare the backend API URL"
            );
        }
        assert!(
            admin_env.contains("NEXT_PUBLIC_CLIENT_ID=default"),
            "admin env schema should declare the client id"
        );
        assert!(
            codex_env.contains("NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID=runtime.llm"),
            "codex app env schema should declare the default model route"
        );
    }

    #[test]
    fn run_poc_script_starts_runtime_and_checks_common_stack() {
        let script = include_str!("../../../../../scripts/run-poc.sh");

        assert!(
            script.contains("ENV_FILE=\"${ROOT_DIR}/.env\""),
            "run script should use root .env as the POC aggregate env entry"
        );
        assert!(
            !script.contains(&["backend", ".env"].join("/")),
            "run script should not load a backend-local env file for the local POC flow"
        );
        let retired_env_name = [".env", "poc"].join(".");
        assert!(
            !script.contains(&retired_env_name),
            "run script should not use the retired POC env filename"
        );

        for needle in [
            "require_common_docker_services",
            "ensure_common_postgres_database",
            "docker network inspect",
            "docker exec",
            "docker-rabbitmq",
            "docker-common_default",
            "COMMON_POSTGRES_DATABASE",
            "PARSER_CALLBACK_TOKEN",
            "cargo run -p backend",
            "cargo run -p backend --bin eval_worker",
            "uv run --no-project --with-requirements services/parser-worker/requirements.txt",
            "services/parser-worker/.venv/bin/python -m parser_worker.worker",
            "pnpm dev",
            "LLM_API_KEY",
            "LLM_BASE_URL",
            "LLM_MODEL",
            "EMBEDDING_API_KEY",
            "EMBEDDING_BASE_URL",
            "EMBEDDING_MODEL",
            "RERANKER_API_KEY",
            "RERANKER_BASE_URL",
            "RERANKER_MODEL",
            "RIGHT_CODE_DRAW_BASE_URL",
            "RIGHT_CODE_DRAW_API_KEY",
            "MINERU_TOKEN",
            "PARSER_WORKER_MODE",
            "http://localhost:62603",
            "http://localhost:15673",
            "http://localhost:19011",
        ] {
            assert!(script.contains(needle), "{needle} missing from run script");
        }
        assert!(
            !script.contains("pull \"${POC_SERVICES[@]}\""),
            "pull command should not refresh already-local compose images"
        );
        for removed in [
            concat!("--profile ", "parser"),
            concat!("--profile ", "apps"),
            "--pull never",
            "require_local_images",
            "pull_missing_images",
            "docker image inspect",
            "docker pull",
            "Run './scripts/run-poc.sh pull'",
            "docker compose --env-file",
        ] {
            assert!(
                !script.contains(removed),
                "{removed} should not be part of the local process POC flow"
            );
        }
    }
}
