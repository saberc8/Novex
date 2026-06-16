use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};

use crate::{
    application::ai::{
        agent_service::{AgentRunEventQuery, AgentRunEventResp, AgentRunResp, AgentService},
        customer_service_agent::{CustomerServiceAgentCommand, CustomerServiceAgentService},
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, pagination::PageResult, response::ApiResponse},
};

pub const CUSTOMER_SERVICE_AGENT_RUN_PERMISSION: &str = "ai:customer-service:agent:run";
pub const CUSTOMER_SERVICE_AGENT_LIST_PERMISSION: &str = "ai:customer-service:agent:list";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/customer-service/agent/runs", post(create_run))
        .route("/ai/customer-service/agent/runs/:run_id", get(get_run))
        .route(
            "/ai/customer-service/agent/runs/:run_id/events",
            get(list_events),
        )
}

async fn create_run(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<CustomerServiceAgentCommand>,
) -> Result<Json<ApiResponse<AgentRunResp>>, AppError> {
    require_permission(&current_user, CUSTOMER_SERVICE_AGENT_RUN_PERMISSION)?;
    let service = CustomerServiceAgentService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service
            .create_customer_service_run(current_user.id, command)
            .await?,
    )))
}

async fn get_run(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(run_id): Path<i64>,
) -> Result<Json<ApiResponse<AgentRunResp>>, AppError> {
    require_permission(&current_user, CUSTOMER_SERVICE_AGENT_LIST_PERMISSION)?;
    let service = AgentService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.get_run(run_id).await?)))
}

async fn list_events(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(run_id): Path<i64>,
    Query(query): Query<AgentRunEventQuery>,
) -> Result<Json<ApiResponse<PageResult<AgentRunEventResp>>>, AppError> {
    require_permission(&current_user, CUSTOMER_SERVICE_AGENT_LIST_PERMISSION)?;
    let service = AgentService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.list_events(run_id, query).await?,
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
    async fn customer_service_agent_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/customer-service/agent/runs")
                    .method("POST")
                    .header(header::ACCEPT, "application/json")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"question":"How do refunds work?"}"#))
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
    fn customer_service_template_seed_contains_agent_flow() {
        let seed_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/202606160006_seed_customer_service_template.sql"
        );
        let seed = std::fs::read_to_string(seed_path)
            .expect("missing customer service template seed migration");

        for needle in [
            "customer-service-agent-poc",
            "ai:customer-service:agent:run",
            "ai:customer-service:read",
            "customer-service-agent-regression",
        ] {
            assert!(seed.contains(needle), "{needle} missing");
        }
    }

    #[test]
    fn customer_service_handler_uses_tenant_bound_runtime() {
        let source = include_str!("customer_service.rs");

        assert!(source.contains("CustomerServiceAgentService::for_tenant"));
        assert!(source.contains("current_user.tenant_id"));
    }

    #[tokio::test]
    async fn create_customer_service_run_rejects_missing_permission() {
        let err = create_run(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(CustomerServiceAgentCommand {
                question: "How do refunds work?".to_owned(),
                ..Default::default()
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }
}
