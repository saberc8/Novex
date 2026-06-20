use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};

use crate::{
    application::ai::notebook_service::{
        NotebookArtifactGenerateCommand, NotebookArtifactResp, NotebookAskCommand, NotebookAskResp,
        NotebookService, NotebookSourceCommand, NotebookSourceResp, NotebookWorkspaceCommand,
        NotebookWorkspaceResp,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, response::ApiResponse},
};

pub const NOTEBOOK_LIST_PERMISSION: &str = "ai:notebook:list";
pub const NOTEBOOK_CREATE_PERMISSION: &str = "ai:notebook:create";
pub const NOTEBOOK_SOURCE_PERMISSION: &str = "ai:notebook:source";
pub const NOTEBOOK_ARTIFACT_PERMISSION: &str = "ai:notebook:artifact";
pub const NOTEBOOK_ASK_PERMISSION: &str = "ai:notebook:ask";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/ai/notebooks/workspaces",
            get(list_workspaces).post(create_workspace),
        )
        .route(
            "/ai/notebooks/workspaces/:workspace_id/sources",
            get(list_sources).post(add_source),
        )
        .route(
            "/ai/notebooks/workspaces/:workspace_id/artifacts",
            get(list_artifacts).post(generate_artifact),
        )
        .route(
            "/ai/notebooks/workspaces/:workspace_id/ask",
            axum::routing::post(ask_workspace),
        )
}

async fn create_workspace(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<NotebookWorkspaceCommand>,
) -> Result<Json<ApiResponse<NotebookWorkspaceResp>>, AppError> {
    require_permission(&current_user, NOTEBOOK_CREATE_PERMISSION)?;
    let tenant_id = current_user.tenant_id;
    let user_id = current_user.id;
    let service = NotebookService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .create_workspace(tenant_id, user_id, command)
            .await?,
    )))
}

async fn list_workspaces(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<NotebookWorkspaceResp>>>, AppError> {
    require_permission(&current_user, NOTEBOOK_LIST_PERMISSION)?;
    let tenant_id = current_user.tenant_id;
    let user_id = current_user.id;
    let service = NotebookService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service.list_workspaces(tenant_id, user_id).await?,
    )))
}

async fn add_source(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(workspace_id): Path<i64>,
    Json(command): Json<NotebookSourceCommand>,
) -> Result<Json<ApiResponse<NotebookSourceResp>>, AppError> {
    require_permission(&current_user, NOTEBOOK_SOURCE_PERMISSION)?;
    let tenant_id = current_user.tenant_id;
    let user_id = current_user.id;
    let service = NotebookService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .add_source(tenant_id, user_id, workspace_id, command)
            .await?,
    )))
}

async fn list_sources(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(workspace_id): Path<i64>,
) -> Result<Json<ApiResponse<Vec<NotebookSourceResp>>>, AppError> {
    require_permission(&current_user, NOTEBOOK_SOURCE_PERMISSION)?;
    let tenant_id = current_user.tenant_id;
    let user_id = current_user.id;
    let service = NotebookService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .list_sources(tenant_id, user_id, workspace_id)
            .await?,
    )))
}

async fn list_artifacts(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(workspace_id): Path<i64>,
) -> Result<Json<ApiResponse<Vec<NotebookArtifactResp>>>, AppError> {
    require_permission(&current_user, NOTEBOOK_ARTIFACT_PERMISSION)?;
    let tenant_id = current_user.tenant_id;
    let user_id = current_user.id;
    let service = NotebookService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .list_artifacts(tenant_id, user_id, workspace_id)
            .await?,
    )))
}

async fn generate_artifact(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(workspace_id): Path<i64>,
    Json(command): Json<NotebookArtifactGenerateCommand>,
) -> Result<Json<ApiResponse<NotebookArtifactResp>>, AppError> {
    require_permission(&current_user, NOTEBOOK_ARTIFACT_PERMISSION)?;
    let tenant_id = current_user.tenant_id;
    let user_id = current_user.id;
    let service = NotebookService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .generate_artifact(tenant_id, user_id, workspace_id, command)
            .await?,
    )))
}

async fn ask_workspace(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(workspace_id): Path<i64>,
    Json(command): Json<NotebookAskCommand>,
) -> Result<Json<ApiResponse<NotebookAskResp>>, AppError> {
    require_permission(&current_user, NOTEBOOK_ASK_PERMISSION)?;
    let tenant_id = current_user.tenant_id;
    let user_id = current_user.id;
    let service = NotebookService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .ask_workspace(tenant_id, user_id, workspace_id, command)
            .await?,
    )))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        extract::State,
        http::{header, Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::*;
    use crate::{
        application::ai::notebook_service::NotebookWorkspaceCommand,
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

    #[tokio::test]
    async fn notebook_workspace_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:62602".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/notebooks/workspaces")
                    .method("POST")
                    .header(header::ACCEPT, "application/json")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"name":"Policy Notebook"}"#))
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
    fn notebook_handlers_bind_runtime_to_current_tenant() {
        let source = include_str!("notebook.rs");

        assert!(
            source
                .matches("let tenant_id = current_user.tenant_id;")
                .count()
                >= 5,
            "notebook handlers must bind tenant identity from CurrentUser"
        );
        assert!(
            source.matches("let user_id = current_user.id;").count() >= 5,
            "notebook handlers must bind user identity from CurrentUser"
        );
    }

    #[test]
    fn notebook_permission_seed_contains_route_permissions() {
        let seed_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/202606160004_seed_ai_notebook_permissions.sql"
        );
        let seed =
            std::fs::read_to_string(seed_path).expect("missing notebook permission seed migration");

        for permission in [
            NOTEBOOK_LIST_PERMISSION,
            NOTEBOOK_CREATE_PERMISSION,
            NOTEBOOK_SOURCE_PERMISSION,
            NOTEBOOK_ARTIFACT_PERMISSION,
            NOTEBOOK_ASK_PERMISSION,
        ] {
            assert!(
                seed.contains(permission),
                "{permission} missing from notebook permission seed"
            );
        }
    }

    #[tokio::test]
    async fn create_notebook_workspace_rejects_missing_permission() {
        let err = create_workspace(
            State(test_state()),
            user_with_permissions(vec![]),
            axum::Json(NotebookWorkspaceCommand {
                name: "Policy Notebook".to_owned(),
                ..Default::default()
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn generate_notebook_artifact_rejects_missing_permission() {
        let err = generate_artifact(
            State(test_state()),
            user_with_permissions(vec![]),
            Path(10),
            axum::Json(NotebookArtifactGenerateCommand {
                artifact_kind: "summary".to_owned(),
                title: "Policy Summary".to_owned(),
                topic: "refunds".to_owned(),
                ..Default::default()
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn notebook_ask_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:62602".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/notebooks/workspaces/10/ask")
                    .method("POST")
                    .header(header::ACCEPT, "application/json")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"question":"What changed?"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice::<Value>(&body).unwrap();
        assert_eq!(body["code"], "401");
    }
}
