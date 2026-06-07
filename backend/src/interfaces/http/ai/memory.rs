use axum::{
    extract::{Path, Query, State},
    routing::{delete, get},
    Json, Router,
};

use crate::{
    application::ai::memory_service::{MemoryCommand, MemoryQuery, MemoryResp, MemoryService},
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, pagination::PageResult, response::ApiResponse},
};

const MEMORY_LIST_PERMISSION: &str = "ai:memory:list";
const MEMORY_UPSERT_PERMISSION: &str = "ai:memory:upsert";
const MEMORY_DELETE_PERMISSION: &str = "ai:memory:delete";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/memories", get(list_memories).post(upsert_memory))
        .route("/ai/memories/:id", delete(delete_memory))
}

async fn list_memories(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<MemoryQuery>,
) -> Result<Json<ApiResponse<PageResult<MemoryResp>>>, AppError> {
    require_permission(&current_user, MEMORY_LIST_PERMISSION)?;
    let service = MemoryService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.list_memories(query).await?)))
}

async fn upsert_memory(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<MemoryCommand>,
) -> Result<Json<ApiResponse<MemoryResp>>, AppError> {
    require_permission(&current_user, MEMORY_UPSERT_PERMISSION)?;
    let service = MemoryService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.upsert_memory(current_user.id, command).await?,
    )))
}

async fn delete_memory(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    require_permission(&current_user, MEMORY_DELETE_PERMISSION)?;
    let service = MemoryService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.delete_memory(current_user.id, id).await?,
    )))
}

#[cfg(test)]
mod tests {
    use axum::{extract::State, Json};
    use sqlx::postgres::PgPoolOptions;

    use super::*;
    use crate::{
        application::ai::memory_service::MemoryCommand,
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

    #[tokio::test]
    async fn memory_upsert_handler_rejects_missing_permission() {
        let err = upsert_memory(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(MemoryCommand {
                scope_type: "user".to_owned(),
                scope_id: "1".to_owned(),
                source_kind: "manual".to_owned(),
                source_id: None,
                content: "prefers concise updates".to_owned(),
                summary: "concise updates".to_owned(),
                sensitivity: "preference".to_owned(),
                write_policy: "user_approved".to_owned(),
                ttl_days: Some(90),
                metadata: serde_json::json!({ "confirmedByUser": true }),
                status: 1,
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[test]
    fn memory_permissions_are_seeded_under_ai_control_plane() {
        let seed = include_str!("../../../../migrations/202606050001_seed_ai_foundation_menus.sql");

        assert!(seed.contains("ai:memory:list"));
        assert!(seed.contains("ai:memory:upsert"));
        assert!(seed.contains("ai:memory:delete"));
    }

    #[test]
    fn memory_handlers_bind_runtime_to_current_tenant() {
        let source = include_str!("memory.rs");

        assert!(
            source
                .matches("MemoryService::for_tenant(state.db, current_user.tenant_id)")
                .count()
                >= 3
        );
    }
}
