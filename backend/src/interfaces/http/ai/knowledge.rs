use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, Multipart, Path, Query, State},
    http::{header, request::Parts},
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::{
    application::ai::knowledge_service::{
        parse_job_command_from_uploaded_file, AiFeedbackCommand, AiFeedbackResp, DatasetCommand,
        DatasetQuery, DatasetResp, DocumentParseJobCommand, DocumentQuery, DocumentResp,
        DocumentUploadCommand, FeedbackResp, KnowledgeService, ParsedDocumentUploadCommand,
        ParserJobResp, ParserJobStatusUpdateCommand, RagAskCommand, RagAskResp, RagFeedbackCommand,
    },
    application::system::file_service::{FileResp, FileService},
    domain::auth::model::CurrentUser,
    interfaces::http::{
        middleware::permission::require_permission,
        system::file::{file_upload_body_limit, multipart_upload_command},
        AppState,
    },
    shared::{error::AppError, pagination::PageResult, response::ApiResponse},
};

const DATASET_LIST_PERMISSION: &str = "ai:knowledge:list";
const DATASET_CREATE_PERMISSION: &str = "ai:knowledge:create";
const DOCUMENT_CREATE_PERMISSION: &str = "ai:knowledge:document:create";
const DOCUMENT_LIST_PERMISSION: &str = "ai:knowledge:document:list";
const RAG_ASK_PERMISSION: &str = "ai:knowledge:ask";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct KnowledgeFileUploadResp {
    file: FileResp,
    parse_job: ParserJobResp,
}

fn dataset_access_context(current_user: &CurrentUser) -> (Vec<i64>, bool) {
    let role_ids = current_user.roles.iter().map(|role| role.id).collect();
    let is_admin = current_user.roles.iter().any(|role| role.is_admin());
    (role_ids, is_admin)
}

enum KnowledgeWriteActor {
    User(CurrentUser),
    ParserCallback { user_id: i64 },
}

impl KnowledgeWriteActor {
    fn require_permission(&self, permission: &'static str) -> Result<(), AppError> {
        match self {
            Self::User(current_user) => require_permission(current_user, permission),
            Self::ParserCallback { .. } => Ok(()),
        }
    }

    fn user_id(&self) -> i64 {
        match self {
            Self::User(current_user) => current_user.id,
            Self::ParserCallback { user_id } => *user_id,
        }
    }

    fn parsed_document_tenant_id(
        &self,
        command: &ParsedDocumentUploadCommand,
    ) -> Result<i64, AppError> {
        match self {
            Self::User(current_user) => Ok(current_user.tenant_id),
            Self::ParserCallback { .. } if command.parser_result.tenant_id > 0 => {
                Ok(command.parser_result.tenant_id)
            }
            Self::ParserCallback { .. } => {
                Err(AppError::bad_request("parser callback missing tenantId"))
            }
        }
    }

    fn parser_status_tenant_id(
        &self,
        command: &ParserJobStatusUpdateCommand,
    ) -> Result<i64, AppError> {
        match self {
            Self::User(current_user) => Ok(current_user.tenant_id),
            Self::ParserCallback { .. } => json_i64_field(&command.parser_result, "tenantId")
                .filter(|tenant_id| *tenant_id > 0)
                .ok_or_else(|| AppError::bad_request("parser callback missing tenantId")),
        }
    }
}

#[async_trait]
impl FromRequestParts<AppState> for KnowledgeWriteActor {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let authorization = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());

        if let (Some(expected), Some(actual)) = (
            state.parser_callback_token.as_deref(),
            authorization.and_then(bearer_token),
        ) {
            if actual == expected {
                return Ok(Self::ParserCallback {
                    user_id: state.parser_callback_user_id,
                });
            }
        }

        CurrentUser::from_request_parts(parts, state)
            .await
            .map(Self::User)
    }
}

fn bearer_token(authorization: &str) -> Option<&str> {
    authorization
        .trim()
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn json_i64_field(value: &serde_json::Value, field: &str) -> Option<i64> {
    value.get(field).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))
    })
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/ai/knowledge/datasets/:dataset_id/ask",
            axum::routing::post(ask_dataset_handler),
        )
        .route(
            "/ai/knowledge/feedback",
            axum::routing::post(submit_rag_feedback),
        )
        .route("/ai/feedback", axum::routing::post(submit_ai_feedback))
        .route(
            "/ai/knowledge/datasets/:dataset_id/documents/text",
            axum::routing::post(upload_text_document),
        )
        .route(
            "/ai/knowledge/datasets/:dataset_id/documents/parsed",
            axum::routing::post(upload_parsed_document),
        )
        .route(
            "/ai/knowledge/datasets/:dataset_id/documents/files",
            axum::routing::post(upload_file_document).layer(file_upload_body_limit()),
        )
        .route(
            "/ai/knowledge/datasets/:dataset_id/parse-jobs/:job_id",
            get(get_parse_job),
        )
        .route(
            "/ai/knowledge/datasets/:dataset_id/parse-jobs/:job_id/status",
            axum::routing::post(update_parse_job_status),
        )
        .route(
            "/ai/knowledge/datasets/:dataset_id/parse-jobs",
            axum::routing::post(create_parse_job),
        )
        .route(
            "/ai/knowledge/datasets/:dataset_id/documents",
            get(list_documents),
        )
        .route(
            "/ai/knowledge/datasets/:dataset_id",
            axum::routing::delete(delete_dataset),
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
    let (role_ids, is_admin) = dataset_access_context(&current_user);

    Ok(Json(ApiResponse::ok(
        service
            .list_datasets_for_user(
                current_user.tenant_id,
                current_user.id,
                &role_ids,
                is_admin,
                query,
            )
            .await?,
    )))
}

async fn create_dataset(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<DatasetCommand>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    require_permission(&current_user, DATASET_CREATE_PERMISSION)?;
    let service = KnowledgeService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .create_dataset_for_tenant(current_user.tenant_id, current_user.id, command)
            .await?,
    )))
}

async fn delete_dataset(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(dataset_id): Path<i64>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    require_permission(&current_user, DATASET_CREATE_PERMISSION)?;
    let service = KnowledgeService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .delete_dataset_for_tenant(current_user.tenant_id, dataset_id)
            .await?,
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
    let (role_ids, is_admin) = dataset_access_context(&current_user);

    Ok(Json(ApiResponse::ok(
        service
            .list_documents_for_user(
                current_user.tenant_id,
                current_user.id,
                &role_ids,
                is_admin,
                dataset_id,
                query,
            )
            .await?,
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
            .upload_text_document_for_tenant(
                current_user.tenant_id,
                current_user.id,
                dataset_id,
                command,
            )
            .await?,
    )))
}

async fn upload_parsed_document(
    State(state): State<AppState>,
    actor: KnowledgeWriteActor,
    Path(dataset_id): Path<i64>,
    Json(command): Json<ParsedDocumentUploadCommand>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    actor.require_permission(DOCUMENT_CREATE_PERMISSION)?;
    let tenant_id = actor.parsed_document_tenant_id(&command)?;
    let user_id = actor.user_id();
    let service = KnowledgeService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .upload_parsed_document_for_tenant(tenant_id, user_id, dataset_id, command)
            .await?,
    )))
}

async fn upload_file_document(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(dataset_id): Path<i64>,
    multipart: Multipart,
) -> Result<Json<ApiResponse<KnowledgeFileUploadResp>>, AppError> {
    require_permission(&current_user, DOCUMENT_CREATE_PERMISSION)?;
    let upload_command = multipart_upload_command(multipart).await?;
    let file_service = FileService::new(state.db.clone());
    let file = file_service.upload(current_user.id, upload_command).await?;
    let parse_command = parse_job_command_from_uploaded_file(&file)?;
    let knowledge_service = KnowledgeService::new(state.db);
    let parse_job = knowledge_service
        .create_parse_job_for_tenant(
            current_user.tenant_id,
            current_user.id,
            dataset_id,
            parse_command,
        )
        .await?;

    Ok(Json(ApiResponse::ok(KnowledgeFileUploadResp {
        file,
        parse_job,
    })))
}

async fn create_parse_job(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(dataset_id): Path<i64>,
    Json(command): Json<DocumentParseJobCommand>,
) -> Result<Json<ApiResponse<ParserJobResp>>, AppError> {
    require_permission(&current_user, DOCUMENT_CREATE_PERMISSION)?;
    let service = KnowledgeService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .create_parse_job_for_tenant(
                current_user.tenant_id,
                current_user.id,
                dataset_id,
                command,
            )
            .await?,
    )))
}

async fn get_parse_job(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path((dataset_id, job_id)): Path<(i64, i64)>,
) -> Result<Json<ApiResponse<ParserJobResp>>, AppError> {
    require_permission(&current_user, DOCUMENT_LIST_PERMISSION)?;
    let service = KnowledgeService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .get_parse_job_for_tenant(current_user.tenant_id, dataset_id, job_id)
            .await?,
    )))
}

async fn update_parse_job_status(
    State(state): State<AppState>,
    actor: KnowledgeWriteActor,
    Path((dataset_id, job_id)): Path<(i64, i64)>,
    Json(command): Json<ParserJobStatusUpdateCommand>,
) -> Result<Json<ApiResponse<ParserJobResp>>, AppError> {
    actor.require_permission(DOCUMENT_CREATE_PERMISSION)?;
    let tenant_id = actor.parser_status_tenant_id(&command)?;
    let user_id = actor.user_id();
    let service = KnowledgeService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .update_parse_job_status_for_tenant(tenant_id, user_id, dataset_id, job_id, command)
            .await?,
    )))
}

async fn ask_dataset_handler(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(dataset_id): Path<i64>,
    Json(command): Json<RagAskCommand>,
) -> Result<Json<ApiResponse<RagAskResp>>, AppError> {
    require_permission(&current_user, RAG_ASK_PERMISSION)?;
    let service = KnowledgeService::new(state.db);
    let (role_ids, is_admin) = dataset_access_context(&current_user);

    Ok(Json(ApiResponse::ok(
        service
            .ask_dataset_for_user(
                current_user.tenant_id,
                current_user.id,
                &role_ids,
                is_admin,
                dataset_id,
                command,
            )
            .await?,
    )))
}

async fn submit_rag_feedback(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<RagFeedbackCommand>,
) -> Result<Json<ApiResponse<FeedbackResp>>, AppError> {
    require_permission(&current_user, RAG_ASK_PERMISSION)?;
    let service = KnowledgeService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .submit_rag_feedback_for_tenant(current_user.tenant_id, current_user.id, command)
            .await?,
    )))
}

async fn submit_ai_feedback(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<AiFeedbackCommand>,
) -> Result<Json<ApiResponse<AiFeedbackResp>>, AppError> {
    require_permission(&current_user, RAG_ASK_PERMISSION)?;
    let service = KnowledgeService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .submit_ai_feedback_for_tenant(current_user.tenant_id, current_user.id, command)
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
            AiFeedbackCommand, DatasetCommand, DatasetQuery, DocumentQuery, DocumentUploadCommand,
            ParsedDocumentUploadCommand, ParserJobStatusUpdateCommand, RagAskCommand,
            RagFeedbackCommand,
        },
        domain::auth::model::{CurrentUser, RoleContext},
        infrastructure::security::jwt::JwtService,
        interfaces::http::{build_router, build_router_with_parser_callback_token, AppState},
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
    fn dataset_list_permission_matches_seeded_menu_permission() {
        assert_eq!(DATASET_LIST_PERMISSION, "ai:knowledge:list");
    }

    #[test]
    fn document_create_permission_matches_seeded_menu_permission() {
        assert_eq!(DOCUMENT_CREATE_PERMISSION, "ai:knowledge:document:create");
    }

    #[test]
    fn rag_ask_permission_matches_seeded_menu_permission() {
        assert_eq!(RAG_ASK_PERMISSION, "ai:knowledge:ask");
    }

    #[test]
    fn knowledge_routes_expose_dataset_delete_endpoint() {
        let source = include_str!("knowledge.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "\"/ai/knowledge/datasets/:dataset_id\"",
            "axum::routing::delete(delete_dataset)",
            ".delete_dataset_for_tenant(",
        ] {
            assert!(source.contains(needle), "{needle} missing from handler");
        }
    }

    #[test]
    fn knowledge_handlers_pass_current_user_tenant_to_service() {
        let source = include_str!("knowledge.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            ".list_datasets_for_user(",
            ".create_dataset_for_tenant(",
            ".list_documents_for_user(",
            ".upload_text_document_for_tenant(",
            ".ask_dataset_for_user(",
        ] {
            assert!(source.contains(needle), "{needle} missing from handler");
        }
        assert!(source.matches("current_user.tenant_id").count() >= 10);
    }

    #[test]
    fn knowledge_handlers_pass_current_user_identity_to_dataset_access_checks() {
        let source = include_str!("knowledge.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            ".list_datasets_for_user(",
            ".list_documents_for_user(",
            ".ask_dataset_for_user(",
            "current_user.roles",
            "current_user.roles.iter().any(|role| role.is_admin())",
        ] {
            assert!(source.contains(needle), "{needle} missing from handler");
        }
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
    async fn parsed_document_upload_handler_rejects_missing_permission() {
        let command = serde_json::from_value::<ParsedDocumentUploadCommand>(serde_json::json!({
            "name": "handbook.pdf",
            "contentType": "application/pdf",
            "parserResult": {
                "tenantId": 1,
                "datasetId": 1,
                "documentId": 42,
                "parserJobId": 99,
                "status": "succeeded",
                "blocks": [],
                "chunks": [
                    {
                        "chunkUid": "42:0",
                        "chunkIndex": 0,
                        "text": "入职培训第一天开始。",
                        "tokenCount": 3,
                        "citation": {
                            "documentId": "42",
                            "chunkId": "42:0",
                            "sectionPath": []
                        }
                    }
                ],
                "metadata": {"parser": "mineru", "warnings": []}
            }
        }))
        .unwrap();

        let err = upload_parsed_document(
            State(test_state()),
            KnowledgeWriteActor::User(user_with_permissions(vec![])),
            Path(1),
            Json(command),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn parse_job_create_handler_rejects_missing_permission() {
        let err = create_parse_job(
            State(test_state()),
            user_with_permissions(vec![]),
            Path(1),
            Json(
                serde_json::from_value(serde_json::json!({
                    "name": "handbook.pdf",
                    "fileId": 10,
                    "sourceUri": "/uploads/handbook.pdf"
                }))
                .unwrap(),
            ),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn parse_job_detail_handler_rejects_missing_permission() {
        let err = get_parse_job(
            State(test_state()),
            user_with_permissions(vec![]),
            Path((1, 99)),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn parse_job_status_handler_rejects_missing_permission() {
        let command = serde_json::from_value::<ParserJobStatusUpdateCommand>(serde_json::json!({
            "status": "submitted",
            "callbackStatus": "deferred",
            "parserResult": {
                "status": "submitted",
                "tenantId": 1,
                "datasetId": 1,
                "documentId": 42,
                "parserJobId": 99
            }
        }))
        .unwrap();

        let err = update_parse_job_status(
            State(test_state()),
            KnowledgeWriteActor::User(user_with_permissions(vec![])),
            Path((1, 99)),
            Json(command),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn rag_ask_handler_rejects_missing_permission() {
        let err = ask_dataset_handler(
            State(test_state()),
            user_with_permissions(vec![]),
            Path(1),
            Json(RagAskCommand {
                question: "培训什么时候开始？".to_owned(),
                ..RagAskCommand::default()
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn rag_feedback_handler_rejects_missing_permission() {
        let err = submit_rag_feedback(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(RagFeedbackCommand {
                trace_id: 42,
                rating: "helpful".to_owned(),
                reason: String::new(),
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn ai_feedback_handler_rejects_missing_permission() {
        let err = submit_ai_feedback(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(AiFeedbackCommand {
                resource_type: "training_quiz".to_owned(),
                resource_id: "900".to_owned(),
                trace_id: Some("agent-900".to_owned()),
                rating: "quiz_wrong_answer".to_owned(),
                reason: "错题答案需要复核".to_owned(),
                metadata: serde_json::json!({ "source": "training-web" }),
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

    #[tokio::test]
    async fn parse_job_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/knowledge/datasets/1/parse-jobs")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"name":"handbook.pdf","fileId":10,"sourceUri":"/uploads/handbook.pdf"}"#))
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
    async fn parse_job_status_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/knowledge/datasets/1/parse-jobs/99/status")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"status":"submitted","callbackStatus":"deferred"}"#,
                    ))
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
    async fn parse_job_status_route_accepts_parser_callback_token() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router_with_parser_callback_token(
            db,
            &["http://localhost:4399".to_owned()],
            jwt,
            Some("parser-callback-token".to_owned()),
            1,
        )
        .unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/knowledge/datasets/1/parse-jobs/99/status")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, "Bearer parser-callback-token")
                    .body(Body::from(
                        r#"{"status":"submitted","callbackStatus":"deferred","parserResult":{"tenantId":1,"datasetId":1,"documentId":42,"parserJobId":99,"status":"submitted"}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice::<Value>(&body).unwrap();
        assert_ne!(body["code"], "401");
    }

    #[tokio::test]
    async fn knowledge_file_upload_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/knowledge/datasets/1/documents/files")
                    .header(header::CONTENT_TYPE, "multipart/form-data; boundary=novex")
                    .body(Body::from("--novex--\r\n"))
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
    async fn rag_ask_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/knowledge/datasets/1/ask")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"question":"培训什么时候开始？"}"#))
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
    async fn rag_feedback_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/knowledge/feedback")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"traceId":42,"rating":"helpful"}"#))
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
    async fn ai_feedback_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/feedback")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"resourceType":"training_quiz","resourceId":"900","rating":"quiz_wrong_answer"}"#,
                    ))
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
    async fn training_learning_records_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/training/learning-records?scope=self")
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
}
