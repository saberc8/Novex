use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};

use crate::{
    application::ai::studio_service::{
        StudioActionQuery, StudioActionResp, StudioArtifactGenerateCommand, StudioArtifactResp,
        StudioService,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, response::ApiResponse},
};

pub const STUDIO_ACTION_LIST_PERMISSION: &str = "ai:studio:action:list";
pub const STUDIO_ARTIFACT_LIST_PERMISSION: &str = "ai:studio:artifact:list";
pub const STUDIO_ARTIFACT_CREATE_PERMISSION: &str = "ai:studio:artifact:create";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/studio/actions", get(list_actions))
        .route("/ai/studio/artifacts/:artifact_id", get(get_artifact))
        .route(
            "/ai/knowledge/datasets/:dataset_id/artifacts",
            get(list_dataset_artifacts).post(generate_dataset_artifact),
        )
}

async fn list_actions(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<StudioActionQuery>,
) -> Result<Json<ApiResponse<Vec<StudioActionResp>>>, AppError> {
    require_permission(&current_user, STUDIO_ACTION_LIST_PERMISSION)?;
    let service = StudioService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service.list_actions(current_user.tenant_id, query).await?,
    )))
}

async fn list_dataset_artifacts(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(dataset_id): Path<i64>,
) -> Result<Json<ApiResponse<Vec<StudioArtifactResp>>>, AppError> {
    require_permission(&current_user, STUDIO_ARTIFACT_LIST_PERMISSION)?;
    let service = StudioService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .list_dataset_artifacts(current_user.tenant_id, current_user.id, dataset_id)
            .await?,
    )))
}

async fn get_artifact(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(artifact_id): Path<i64>,
) -> Result<Json<ApiResponse<StudioArtifactResp>>, AppError> {
    require_permission(&current_user, STUDIO_ARTIFACT_LIST_PERMISSION)?;
    let service = StudioService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .get_artifact(current_user.tenant_id, current_user.id, artifact_id)
            .await?,
    )))
}

async fn generate_dataset_artifact(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(dataset_id): Path<i64>,
    Json(command): Json<StudioArtifactGenerateCommand>,
) -> Result<Json<ApiResponse<StudioArtifactResp>>, AppError> {
    require_permission(&current_user, STUDIO_ARTIFACT_CREATE_PERMISSION)?;
    let service = StudioService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .generate_artifact(current_user.tenant_id, current_user.id, dataset_id, command)
            .await?,
    )))
}

#[cfg(test)]
mod tests {
    use axum::extract::{Path, State};
    use sqlx::postgres::PgPoolOptions;

    use super::*;
    use crate::{
        application::ai::studio_service::StudioArtifactGenerateCommand,
        domain::auth::model::{CurrentUser, RoleContext},
        infrastructure::security::jwt::JwtService,
        shared::error::AppError,
    };

    #[test]
    fn studio_migrations_define_actions_artifacts_mind_map_and_permissions() {
        let schema =
            include_str!("../../../../migrations/202606090004_create_ai_studio_artifact.sql");
        let permissions =
            include_str!("../../../../migrations/202606090005_seed_ai_studio_permissions.sql");

        for table in ["ai_studio_action", "ai_studio_artifact"] {
            assert!(schema.contains(table), "{table} missing from migration");
        }
        for field in [
            "artifact_type",
            "content_json",
            "source_snapshot",
            "citations",
            "run_id",
        ] {
            assert!(schema.contains(field), "{field} missing from migration");
        }
        assert!(
            schema.contains("mind_map.generate"),
            "mind_map.generate seed missing"
        );
        for permission in [
            "ai:studio:action:list",
            "ai:studio:artifact:list",
            "ai:studio:artifact:create",
        ] {
            assert!(
                permissions.contains(permission),
                "{permission} missing from seed"
            );
        }
    }

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
    async fn generate_studio_artifact_rejects_missing_permission() {
        let err = generate_dataset_artifact(
            State(test_state()),
            user_with_permissions(vec![]),
            Path(10),
            axum::Json(StudioArtifactGenerateCommand {
                action_code: "mind_map.generate".to_owned(),
                topic: "Policy".to_owned(),
                session_id: None,
                max_nodes: Some(8),
                answer_model_route_id: None,
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }
}
