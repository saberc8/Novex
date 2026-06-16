use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};

use crate::{
    application::ai::agent_service::{
        AgentRunCommand, AgentRunEventQuery, AgentRunEventResp, AgentRunQuery, AgentRunResp,
        AgentRunResumeCommand, AgentService, AgentTraceReplayResp,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, pagination::PageResult, response::ApiResponse},
};

const AGENT_LIST_PERMISSION: &str = "ai:agent:list";
const AGENT_RUN_PERMISSION: &str = "ai:agent:run";
const AGENT_EVENT_LIST_PERMISSION: &str = "ai:agent:event:list";
const AGENT_RESUME_PERMISSION: &str = "ai:agent:resume";
const AGENT_CANCEL_PERMISSION: &str = "ai:agent:cancel";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/agents/runs", post(create_run).get(list_runs))
        .route("/ai/agents/runs/:run_id", get(get_run))
        .route("/ai/agents/runs/:run_id/events", get(list_events))
        .route("/ai/agents/runs/:run_id/trace", get(get_run_trace))
        .route("/ai/agents/runs/:run_id/resume", post(resume_run))
        .route("/ai/agents/runs/:run_id/cancel", post(cancel_run))
}

async fn create_run(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<AgentRunCommand>,
) -> Result<Json<ApiResponse<AgentRunResp>>, AppError> {
    require_permission(&current_user, AGENT_RUN_PERMISSION)?;
    let service = AgentService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.create_run(current_user.id, command).await?,
    )))
}

async fn list_runs(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<AgentRunQuery>,
) -> Result<Json<ApiResponse<PageResult<AgentRunResp>>>, AppError> {
    require_permission(&current_user, AGENT_LIST_PERMISSION)?;
    let service = AgentService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.list_runs(query).await?)))
}

async fn get_run(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(run_id): Path<i64>,
) -> Result<Json<ApiResponse<AgentRunResp>>, AppError> {
    require_permission(&current_user, AGENT_LIST_PERMISSION)?;
    let service = AgentService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.get_run(run_id).await?)))
}

async fn list_events(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(run_id): Path<i64>,
    Query(query): Query<AgentRunEventQuery>,
) -> Result<Json<ApiResponse<PageResult<AgentRunEventResp>>>, AppError> {
    require_permission(&current_user, AGENT_EVENT_LIST_PERMISSION)?;
    let service = AgentService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.list_events(run_id, query).await?,
    )))
}

async fn get_run_trace(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(run_id): Path<i64>,
) -> Result<Json<ApiResponse<AgentTraceReplayResp>>, AppError> {
    require_permission(&current_user, AGENT_EVENT_LIST_PERMISSION)?;
    let service = AgentService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.get_run_trace(run_id).await?)))
}

async fn resume_run(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(run_id): Path<i64>,
    Json(command): Json<AgentRunResumeCommand>,
) -> Result<Json<ApiResponse<AgentRunResp>>, AppError> {
    require_permission(&current_user, AGENT_RESUME_PERMISSION)?;
    let service = AgentService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.resume_run(current_user.id, run_id, command).await?,
    )))
}

async fn cancel_run(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(run_id): Path<i64>,
) -> Result<Json<ApiResponse<AgentRunResp>>, AppError> {
    require_permission(&current_user, AGENT_CANCEL_PERMISSION)?;
    let service = AgentService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.cancel_run(current_user.id, run_id).await?,
    )))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        extract::{Path, Query, State},
        http::{header, Request, StatusCode},
        Json,
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::*;
    use crate::{
        application::ai::agent_service::{
            AgentRunCommand, AgentRunEventQuery, AgentRunQuery, AgentRunResumeCommand,
        },
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
    fn agent_runtime_permissions_match_seeded_menu_permissions() {
        assert_eq!(AGENT_LIST_PERMISSION, "ai:agent:list");
        assert_eq!(AGENT_RUN_PERMISSION, "ai:agent:run");
        assert_eq!(AGENT_EVENT_LIST_PERMISSION, "ai:agent:event:list");
        assert_eq!(AGENT_RESUME_PERMISSION, "ai:agent:resume");
        assert_eq!(AGENT_CANCEL_PERMISSION, "ai:agent:cancel");
    }

    #[test]
    fn agent_handlers_bind_runtime_to_current_tenant() {
        let source = include_str!("agent.rs");

        assert!(
            source
                .matches("AgentService::for_tenant(state.db, current_user.tenant_id)")
                .count()
                >= 6
        );
    }

    #[tokio::test]
    async fn agent_run_handler_rejects_missing_permission() {
        let err = create_run(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(AgentRunCommand::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn agent_event_list_handler_rejects_missing_permission() {
        let err = list_events(
            State(test_state()),
            user_with_permissions(vec![]),
            Path(42),
            Query(AgentRunEventQuery::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn agent_trace_handler_rejects_missing_permission() {
        let err = get_run_trace(State(test_state()), user_with_permissions(vec![]), Path(42))
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn agent_resume_handler_rejects_missing_permission() {
        let err = resume_run(
            State(test_state()),
            user_with_permissions(vec![]),
            Path(42),
            Json(AgentRunResumeCommand {
                approved: true,
                input: Value::Null,
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn agent_cancel_handler_rejects_missing_permission() {
        let err = cancel_run(State(test_state()), user_with_permissions(vec![]), Path(42))
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn agent_run_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/agents/runs")
                    .method("POST")
                    .header(header::ACCEPT, "application/json")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"input":"search handbook"}"#))
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
    async fn agent_event_snapshot_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/agents/runs/42/events")
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
    async fn agent_trace_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/agents/runs/42/trace")
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
    async fn agent_list_handler_rejects_missing_permission() {
        let err = list_runs(
            State(test_state()),
            user_with_permissions(vec![]),
            Query(AgentRunQuery::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }
}
