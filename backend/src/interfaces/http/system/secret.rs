use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};

use crate::{
    application::system::secret_service::{
        SecretCommand, SecretQuery, SecretRecordPublicResp, SecretService,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::middleware::permission::require_permission,
    shared::{error::AppError, pagination::PageResult, response::ApiResponse},
};

use super::super::AppState;

pub const SECRET_LIST_PERMISSION: &str = "system:secret:list";
pub const SECRET_UPSERT_PERMISSION: &str = "system:secret:upsert";

pub fn routes() -> Router<AppState> {
    Router::new().route("/system/secrets", get(list).post(upsert))
}

async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<SecretQuery>,
) -> Result<Json<ApiResponse<PageResult<SecretRecordPublicResp>>>, AppError> {
    require_permission(&current_user, SECRET_LIST_PERMISSION)?;
    let service = SecretService::new(state.db);

    Ok(Json(ApiResponse::ok(service.list(query).await?)))
}

async fn upsert(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<SecretCommand>,
) -> Result<Json<ApiResponse<SecretRecordPublicResp>>, AppError> {
    require_permission(&current_user, SECRET_UPSERT_PERMISSION)?;
    let service = SecretService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service.upsert(current_user.id, command).await?,
    )))
}

#[cfg(test)]
mod tests {
    use axum::extract::State;
    use serde_json::json;
    use sqlx::postgres::PgPoolOptions;

    use super::*;
    use crate::{
        domain::auth::model::{CurrentUser, RoleContext},
        infrastructure::security::jwt::JwtService,
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
    fn secret_permissions_are_seeded_under_system_identity_control_plane() {
        let seed = include_str!("../../../../migrations/202606050001_seed_ai_foundation_menus.sql");

        assert!(seed.contains(SECRET_LIST_PERMISSION));
        assert!(seed.contains(SECRET_UPSERT_PERMISSION));
    }

    #[tokio::test]
    async fn secret_upsert_handler_rejects_missing_permission_before_database() {
        let err = upsert(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(SecretCommand {
                scope_type: "tenant".to_owned(),
                scope_id: "1".to_owned(),
                code: "github.connector".to_owned(),
                plaintext: "github_pat_1234567890".to_owned(),
                metadata: json!({}),
                status: 1,
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }
}
