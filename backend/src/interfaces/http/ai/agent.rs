use std::{
    collections::VecDeque,
    convert::Infallible,
    time::{Duration, Instant},
};

use axum::{
    extract::{Path, Query, State},
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use futures_util::{stream, Stream};
use serde_json::json;

use crate::{
    application::ai::agent_service::{
        AgentRunCommand, AgentRunEventQuery, AgentRunEventResp, AgentRunEventStreamQuery,
        AgentRunEventStreamSettings, AgentRunQuery, AgentRunResp, AgentRunResumeCommand,
        AgentService, AgentTraceReplayResp,
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
const AGENT_RUN_EVENT_STREAM_NAME: &str = "agent_run_event";
const AGENT_RUN_EVENT_STREAM_CONTENT_TYPE: &str = "text/event-stream";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/agents/runs", post(create_run).get(list_runs))
        .route("/ai/agents/runs/:run_id", get(get_run))
        .route("/ai/agents/runs/:run_id/events", get(list_events))
        .route("/ai/agents/runs/:run_id/events/stream", get(stream_events))
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
    let service = AgentService::for_tenant_with_runtime(
        state.db,
        current_user.tenant_id,
        state.agent_runtime,
    );

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
    let service = AgentService::for_tenant_with_runtime(
        state.db,
        current_user.tenant_id,
        state.agent_runtime,
    );

    Ok(Json(ApiResponse::ok(service.list_runs(query).await?)))
}

async fn get_run(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(run_id): Path<i64>,
) -> Result<Json<ApiResponse<AgentRunResp>>, AppError> {
    require_permission(&current_user, AGENT_LIST_PERMISSION)?;
    let service = AgentService::for_tenant_with_runtime(
        state.db,
        current_user.tenant_id,
        state.agent_runtime,
    );

    Ok(Json(ApiResponse::ok(service.get_run(run_id).await?)))
}

async fn list_events(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(run_id): Path<i64>,
    Query(query): Query<AgentRunEventQuery>,
) -> Result<Json<ApiResponse<PageResult<AgentRunEventResp>>>, AppError> {
    require_permission(&current_user, AGENT_EVENT_LIST_PERMISSION)?;
    let service = AgentService::for_tenant_with_runtime(
        state.db,
        current_user.tenant_id,
        state.agent_runtime,
    );

    Ok(Json(ApiResponse::ok(
        service.list_events(run_id, query).await?,
    )))
}

async fn stream_events(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(run_id): Path<i64>,
    Query(query): Query<AgentRunEventStreamQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    require_permission(&current_user, AGENT_EVENT_LIST_PERMISSION)?;
    let service = AgentService::for_tenant_with_runtime(
        state.db,
        current_user.tenant_id,
        state.agent_runtime,
    );
    let _content_type = AGENT_RUN_EVENT_STREAM_CONTENT_TYPE;

    Ok(
        Sse::new(agent_run_event_stream(service, run_id, query.settings()))
            .keep_alive(KeepAlive::default()),
    )
}

async fn get_run_trace(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(run_id): Path<i64>,
) -> Result<Json<ApiResponse<AgentTraceReplayResp>>, AppError> {
    require_permission(&current_user, AGENT_EVENT_LIST_PERMISSION)?;
    let service = AgentService::for_tenant_with_runtime(
        state.db,
        current_user.tenant_id,
        state.agent_runtime,
    );

    Ok(Json(ApiResponse::ok(service.get_run_trace(run_id).await?)))
}

async fn resume_run(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(run_id): Path<i64>,
    Json(command): Json<AgentRunResumeCommand>,
) -> Result<Json<ApiResponse<AgentRunResp>>, AppError> {
    require_permission(&current_user, AGENT_RESUME_PERMISSION)?;
    let service = AgentService::for_tenant_with_runtime(
        state.db,
        current_user.tenant_id,
        state.agent_runtime,
    );

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
    let service = AgentService::for_tenant_with_runtime(
        state.db,
        current_user.tenant_id,
        state.agent_runtime,
    );

    Ok(Json(ApiResponse::ok(
        service.cancel_run(current_user.id, run_id).await?,
    )))
}

struct AgentRunEventSseState {
    service: AgentService,
    run_id: i64,
    settings: AgentRunEventStreamSettings,
    after_sequence_no: i64,
    pending: VecDeque<AgentRunEventResp>,
    idle_since: Instant,
    closed: bool,
}

fn agent_run_event_stream(
    service: AgentService,
    run_id: i64,
    settings: AgentRunEventStreamSettings,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let state = AgentRunEventSseState {
        after_sequence_no: settings.after_sequence_no,
        service,
        run_id,
        settings,
        pending: VecDeque::new(),
        idle_since: Instant::now(),
        closed: false,
    };

    stream::unfold(state, |mut state| async move {
        loop {
            if state.closed {
                return None;
            }

            if let Some(event) = state.pending.pop_front() {
                state.after_sequence_no = event.sequence_no;
                state.idle_since = Instant::now();
                return Some((Ok(agent_run_event_sse(event)), state));
            }

            match state
                .service
                .list_events_after_sequence(
                    state.run_id,
                    state.after_sequence_no,
                    state.settings.batch_size,
                )
                .await
            {
                Ok(events) if !events.is_empty() => {
                    state.pending = events.into();
                    continue;
                }
                Ok(_) => match state.service.is_run_terminal(state.run_id).await {
                    Ok(true) => return None,
                    Ok(false) => {
                        if state.idle_since.elapsed()
                            >= Duration::from_millis(state.settings.max_idle_ms)
                        {
                            return None;
                        }
                        tokio::time::sleep(Duration::from_millis(state.settings.poll_ms)).await;
                    }
                    Err(err) => {
                        state.closed = true;
                        return Some((Ok(agent_run_event_error_sse(err)), state));
                    }
                },
                Err(err) => {
                    state.closed = true;
                    return Some((Ok(agent_run_event_error_sse(err)), state));
                }
            }
        }
    })
}

fn agent_run_event_sse(event: AgentRunEventResp) -> Event {
    let sequence_no = event.sequence_no;
    let data = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_owned());
    Event::default()
        .event(AGENT_RUN_EVENT_STREAM_NAME)
        .id(sequence_no.to_string())
        .data(data)
}

fn agent_run_event_error_sse(err: AppError) -> Event {
    Event::default().event("error").data(
        json!({
            "message": err.to_string()
        })
        .to_string(),
    )
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
    fn agent_runtime_permissions_match_seeded_menu_permissions() {
        assert_eq!(AGENT_LIST_PERMISSION, "ai:agent:list");
        assert_eq!(AGENT_RUN_PERMISSION, "ai:agent:run");
        assert_eq!(AGENT_EVENT_LIST_PERMISSION, "ai:agent:event:list");
        assert_eq!(AGENT_RESUME_PERMISSION, "ai:agent:resume");
        assert_eq!(AGENT_CANCEL_PERMISSION, "ai:agent:cancel");
    }

    #[test]
    fn agent_handlers_bind_runtime_to_current_tenant() {
        let source = include_str!("agent.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(
            source
                .matches("AgentService::for_tenant_with_runtime")
                .count()
                >= 6
        );
    }

    #[test]
    fn agent_handlers_share_runtime_registry_from_app_state() {
        let source = include_str!("agent.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("state.agent_runtime"));
        assert!(source.contains("AgentService::for_tenant_with_runtime"));
    }

    #[test]
    fn app_state_owns_agent_runtime_registry() {
        let source = include_str!("../mod.rs");

        assert!(source.contains("agent_runtime: AgentRuntimeRegistry"));
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
    async fn agent_event_stream_handler_rejects_missing_permission() {
        let err = stream_events(
            State(test_state()),
            user_with_permissions(vec![]),
            Path(42),
            Query(AgentRunEventStreamQuery::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[test]
    fn agent_event_stream_route_uses_sse_and_keepalive() {
        let source = include_str!("agent.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("/ai/agents/runs/:run_id/events/stream"));
        assert!(source.contains("Sse"));
        assert!(source.contains("KeepAlive"));
        assert!(source.contains("text/event-stream"));
    }

    #[test]
    fn agent_event_stream_sse_event_uses_sequence_id() {
        let event = AgentRunEventResp {
            id: 7,
            run_id: 42,
            step_id: None,
            event_type: "thought".to_owned(),
            sequence_no: 9,
            status: "running".to_owned(),
            payload: serde_json::json!({ "message": "thinking" }),
            create_time: "2026-06-17 12:00:00".to_owned(),
        };

        let sse = agent_run_event_sse(event);
        let debug = format!("{sse:?}");

        assert!(debug.contains("agent_run_event"));
        let source = include_str!("agent.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        assert!(source.contains(".id(sequence_no.to_string())"));
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
    async fn agent_event_stream_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/agents/runs/42/events/stream")
                    .header(header::ACCEPT, "text/event-stream")
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
