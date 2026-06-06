use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};

use crate::{
    application::ai::chat_flow_service::{
        ChatFlowMessageCommand, ChatFlowMessageResp, ChatFlowSendMessageResp,
        ChatFlowService, ChatFlowSessionCommand, ChatFlowSessionQuery, ChatFlowSessionResp,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, response::ApiResponse},
};

pub const CHAT_FLOW_LIST_PERMISSION: &str = "ai:chatFlow:list";
pub const CHAT_FLOW_CREATE_PERMISSION: &str = "ai:chatFlow:create";
pub const CHAT_FLOW_MESSAGE_PERMISSION: &str = "ai:chatFlow:message";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/chat-flow/sessions", get(list_sessions).post(create_session))
        .route(
            "/ai/chat-flow/sessions/:session_id/messages",
            get(list_messages).post(send_message),
        )
}

async fn create_session(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<ChatFlowSessionCommand>,
) -> Result<Json<ApiResponse<ChatFlowSessionResp>>, AppError> {
    require_permission(&current_user, CHAT_FLOW_CREATE_PERMISSION)?;
    let service = ChatFlowService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .create_session(current_user.tenant_id, current_user.id, command)
            .await?,
    )))
}

async fn list_sessions(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<ChatFlowSessionQuery>,
) -> Result<Json<ApiResponse<Vec<ChatFlowSessionResp>>>, AppError> {
    require_permission(&current_user, CHAT_FLOW_LIST_PERMISSION)?;
    let service = ChatFlowService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .list_sessions(current_user.tenant_id, current_user.id, query)
            .await?,
    )))
}

async fn list_messages(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(session_id): Path<i64>,
) -> Result<Json<ApiResponse<Vec<ChatFlowMessageResp>>>, AppError> {
    require_permission(&current_user, CHAT_FLOW_MESSAGE_PERMISSION)?;
    let service = ChatFlowService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .list_messages(current_user.tenant_id, current_user.id, session_id)
            .await?,
    )))
}

async fn send_message(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(session_id): Path<i64>,
    Json(command): Json<ChatFlowMessageCommand>,
) -> Result<Json<ApiResponse<ChatFlowSendMessageResp>>, AppError> {
    require_permission(&current_user, CHAT_FLOW_MESSAGE_PERMISSION)?;
    let service = ChatFlowService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .send_message(current_user.tenant_id, current_user.id, session_id, command)
            .await?,
    )))
}

#[cfg(test)]
mod tests {
    use axum::extract::State;
    use sqlx::postgres::PgPoolOptions;

    use super::*;
    use crate::{
        application::ai::chat_flow_service::ChatFlowSessionCommand,
        domain::auth::model::{CurrentUser, RoleContext},
        infrastructure::security::jwt::JwtService,
        interfaces::http::AppState,
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
    fn chat_flow_migrations_define_session_message_and_permissions() {
        let schema = include_str!(
            "../../../../migrations/202606060002_create_ai_chat_flow.sql"
        );
        let permissions = include_str!(
            "../../../../migrations/202606060003_seed_ai_chat_flow_permissions.sql"
        );

        for table in ["ai_chat_flow_session", "ai_chat_flow_message"] {
            assert!(schema.contains(table), "{table} missing from migration");
        }
        for field in [
            "dataset_id",
            "mode",
            "rag_trace_id",
            "citations",
            "message_count",
        ] {
            assert!(schema.contains(field), "{field} missing from migration");
        }
        for permission in [
            "ai:chatFlow:list",
            "ai:chatFlow:create",
            "ai:chatFlow:message",
        ] {
            assert!(
                permissions.contains(permission),
                "{permission} missing from seed"
            );
        }
    }

    #[tokio::test]
    async fn create_chat_flow_session_rejects_missing_permission() {
        let err = create_session(
            State(test_state()),
            user_with_permissions(vec![]),
            axum::Json(ChatFlowSessionCommand {
                mode: "knowledge".to_owned(),
                dataset_id: Some(10),
                title: "Policy".to_owned(),
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }
}
