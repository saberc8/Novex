use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};

use crate::{
    application::ai::knowledge_service::{
        DatasetCommand, DatasetQuery, DatasetResp, DocumentQuery, DocumentResp,
        DocumentUploadCommand, KnowledgeService,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, pagination::PageResult, response::ApiResponse},
};

const DATASET_LIST_PERMISSION: &str = "ai:knowledge:list";
const DATASET_CREATE_PERMISSION: &str = "ai:knowledge:create";
const DOCUMENT_CREATE_PERMISSION: &str = "ai:knowledge:document:create";
const DOCUMENT_LIST_PERMISSION: &str = "ai:knowledge:document:list";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/ai/knowledge/datasets/:dataset_id/documents/text",
            axum::routing::post(upload_text_document),
        )
        .route(
            "/ai/knowledge/datasets/:dataset_id/documents",
            get(list_documents),
        )
        .route(
            "/ai/knowledge/datasets",
            get(list_datasets).post(create_dataset),
        )
}

async fn list_datasets(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<DatasetQuery>,
) -> Result<Json<ApiResponse<PageResult<DatasetResp>>>, AppError> {
    require_permission(&current_user, DATASET_LIST_PERMISSION)?;
    let service = KnowledgeService::new(state.db);

    Ok(Json(ApiResponse::ok(service.list_datasets(query).await?)))
}

async fn create_dataset(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<DatasetCommand>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    require_permission(&current_user, DATASET_CREATE_PERMISSION)?;
    let service = KnowledgeService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service.create_dataset(current_user.id, command).await?,
    )))
}

async fn list_documents(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(dataset_id): Path<i64>,
    Query(query): Query<DocumentQuery>,
) -> Result<Json<ApiResponse<PageResult<DocumentResp>>>, AppError> {
    require_permission(&current_user, DOCUMENT_LIST_PERMISSION)?;
    let service = KnowledgeService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service.list_documents(dataset_id, query).await?,
    )))
}

async fn upload_text_document(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(dataset_id): Path<i64>,
    Json(command): Json<DocumentUploadCommand>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    require_permission(&current_user, DOCUMENT_CREATE_PERMISSION)?;
    let service = KnowledgeService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .upload_text_document(current_user.id, dataset_id, command)
            .await?,
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
        application::ai::knowledge_service::{
            DatasetCommand, DatasetQuery, DocumentQuery, DocumentUploadCommand,
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
        }
    }

    fn user_with_permissions(permissions: Vec<&str>) -> CurrentUser {
        CurrentUser {
            id: 1,
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
    fn dataset_list_permission_matches_seeded_menu_permission() {
        assert_eq!(DATASET_LIST_PERMISSION, "ai:knowledge:list");
    }

    #[test]
    fn document_create_permission_matches_seeded_menu_permission() {
        assert_eq!(DOCUMENT_CREATE_PERMISSION, "ai:knowledge:document:create");
    }

    #[tokio::test]
    async fn dataset_list_handler_rejects_missing_permission() {
        let err = list_datasets(
            State(test_state()),
            user_with_permissions(vec![]),
            Query(DatasetQuery::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn dataset_create_handler_rejects_missing_permission() {
        let err = create_dataset(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(DatasetCommand::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn document_list_handler_rejects_missing_permission() {
        let err = list_documents(
            State(test_state()),
            user_with_permissions(vec![]),
            Path(1),
            Query(DocumentQuery::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn document_upload_handler_rejects_missing_permission() {
        let err = upload_text_document(
            State(test_state()),
            user_with_permissions(vec![]),
            Path(1),
            Json(DocumentUploadCommand {
                name: "handbook.txt".to_owned(),
                content: "hello".to_owned(),
                ..DocumentUploadCommand::default()
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn knowledge_dataset_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/knowledge/datasets")
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
    async fn knowledge_document_upload_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/knowledge/datasets/1/documents/text")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"name":"handbook.txt","content":"hello"}"#))
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
