use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};

use crate::{
    application::ai::eval_service::{
        EvalCaseQuery, EvalCaseResp, EvalDatasetQuery, EvalDatasetResp, EvalResultQuery,
        EvalResultResp, EvalRunCommand, EvalRunQuery, EvalRunResp, EvalService,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, pagination::PageResult, response::ApiResponse},
};

const EVAL_LIST_PERMISSION: &str = "ai:eval:list";
const EVAL_RUN_PERMISSION: &str = "ai:eval:run";
const EVAL_CASE_LIST_PERMISSION: &str = "ai:eval:case:list";
const EVAL_REPORT_PERMISSION: &str = "ai:eval:report";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/evals/datasets", get(list_datasets))
        .route("/ai/evals/datasets/:dataset_id/cases", get(list_cases))
        .route("/ai/evals/runs", post(run_eval).get(list_runs))
        .route("/ai/evals/runs/:run_id", get(get_run))
        .route("/ai/evals/runs/:run_id/results", get(list_results))
}

async fn list_datasets(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<EvalDatasetQuery>,
) -> Result<Json<ApiResponse<PageResult<EvalDatasetResp>>>, AppError> {
    require_permission(&current_user, EVAL_LIST_PERMISSION)?;
    let service = EvalService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.list_datasets(query).await?)))
}

async fn list_cases(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(dataset_id): Path<i64>,
    Query(query): Query<EvalCaseQuery>,
) -> Result<Json<ApiResponse<PageResult<EvalCaseResp>>>, AppError> {
    require_permission(&current_user, EVAL_CASE_LIST_PERMISSION)?;
    let service = EvalService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.list_cases(dataset_id, query).await?,
    )))
}

async fn run_eval(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<EvalRunCommand>,
) -> Result<Json<ApiResponse<EvalRunResp>>, AppError> {
    require_permission(&current_user, EVAL_RUN_PERMISSION)?;
    let service = EvalService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.run_eval(current_user.id, command).await?,
    )))
}

async fn list_runs(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<EvalRunQuery>,
) -> Result<Json<ApiResponse<PageResult<EvalRunResp>>>, AppError> {
    require_permission(&current_user, EVAL_REPORT_PERMISSION)?;
    let service = EvalService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.list_runs(query).await?)))
}

async fn get_run(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(run_id): Path<i64>,
) -> Result<Json<ApiResponse<EvalRunResp>>, AppError> {
    require_permission(&current_user, EVAL_REPORT_PERMISSION)?;
    let service = EvalService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.get_run(run_id).await?)))
}

async fn list_results(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(run_id): Path<i64>,
    Query(query): Query<EvalResultQuery>,
) -> Result<Json<ApiResponse<PageResult<EvalResultResp>>>, AppError> {
    require_permission(&current_user, EVAL_REPORT_PERMISSION)?;
    let service = EvalService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.list_results(run_id, query).await?,
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
        application::ai::eval_service::{
            EvalCaseQuery, EvalDatasetQuery, EvalResultQuery, EvalRunCommand, EvalRunQuery,
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
    fn eval_runtime_permissions_match_seeded_menu_permissions() {
        assert_eq!(EVAL_LIST_PERMISSION, "ai:eval:list");
        assert_eq!(EVAL_RUN_PERMISSION, "ai:eval:run");
        assert_eq!(EVAL_CASE_LIST_PERMISSION, "ai:eval:case:list");
        assert_eq!(EVAL_REPORT_PERMISSION, "ai:eval:report");
    }

    #[test]
    fn eval_runtime_permission_seed_contains_all_route_permissions() {
        let seed = include_str!("../../../../migrations/202606050011_seed_ai_eval_permissions.sql");

        for permission in [
            EVAL_LIST_PERMISSION,
            EVAL_RUN_PERMISSION,
            EVAL_CASE_LIST_PERMISSION,
            EVAL_REPORT_PERMISSION,
        ] {
            assert!(
                seed.contains(permission),
                "missing permission seed: {permission}"
            );
        }
    }

    #[test]
    fn eval_handlers_bind_runtime_to_current_tenant() {
        let source = include_str!("eval.rs");

        assert!(
            source
                .matches("EvalService::for_tenant(state.db, current_user.tenant_id)")
                .count()
                >= 6
        );
    }

    #[tokio::test]
    async fn eval_dataset_list_handler_rejects_missing_permission() {
        let err = list_datasets(
            State(test_state()),
            user_with_permissions(vec![]),
            Query(EvalDatasetQuery::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn eval_case_list_handler_rejects_missing_permission() {
        let err = list_cases(
            State(test_state()),
            user_with_permissions(vec![]),
            Path(1),
            Query(EvalCaseQuery::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn eval_run_handler_rejects_missing_permission() {
        let err = run_eval(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(EvalRunCommand {
                dataset_id: None,
                dataset_code: "training_regression".to_owned(),
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn eval_result_list_handler_rejects_missing_permission() {
        let err = list_results(
            State(test_state()),
            user_with_permissions(vec![]),
            Path(1),
            Query(EvalResultQuery::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn eval_run_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/evals/runs")
                    .method("POST")
                    .header(header::ACCEPT, "application/json")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"datasetCode":"training_regression"}"#))
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
    async fn eval_dataset_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/evals/datasets")
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
    async fn eval_run_list_handler_rejects_missing_permission() {
        let err = list_runs(
            State(test_state()),
            user_with_permissions(vec![]),
            Query(EvalRunQuery::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }
}
