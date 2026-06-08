use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};

use crate::{
    application::ai::training_service::{
        TrainingLearningQuery, TrainingLearningRecordsResp, TrainingService,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, response::ApiResponse},
};

const TRAINING_LEARNING_LIST_PERMISSION: &str = "ai:knowledge:ask";

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/ai/training/learning-records",
        get(list_training_learning_records),
    )
}

async fn list_training_learning_records(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<TrainingLearningQuery>,
) -> Result<Json<ApiResponse<TrainingLearningRecordsResp>>, AppError> {
    require_permission(&current_user, TRAINING_LEARNING_LIST_PERMISSION)?;
    let service = TrainingService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .list_learning_records_for_tenant(current_user.tenant_id, current_user.id, query)
            .await?,
    )))
}

#[cfg(test)]
mod tests {
    use axum::extract::{Query, State};
    use sqlx::postgres::PgPoolOptions;

    use super::*;
    use crate::{
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
    fn training_learning_permission_reuses_customer_training_entry_permission() {
        assert_eq!(TRAINING_LEARNING_LIST_PERMISSION, "ai:knowledge:ask");
    }

    #[tokio::test]
    async fn training_learning_handler_rejects_missing_permission() {
        let err = list_training_learning_records(
            State(test_state()),
            user_with_permissions(vec![]),
            Query(TrainingLearningQuery {
                scope: Some("self".to_owned()),
                user_id: None,
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }
}
