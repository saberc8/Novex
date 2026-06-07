use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};

use crate::{
    application::ai::template_service::{
        apply_customer_package, build_customer_package, get_delivery_template,
        list_delivery_templates, run_template_smoke, CustomerPackageApplyResp,
        CustomerPackageCommand, CustomerPackageResp, DeliveryTemplate, DeliveryTemplateQuery,
        TemplateSmokeRunCommand, TemplateSmokeRunResp,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, pagination::PageResult, response::ApiResponse},
};

const TEMPLATE_LIST_PERMISSION: &str = "ai:template:list";
const TEMPLATE_INIT_PERMISSION: &str = "ai:template:init";
const TEMPLATE_SMOKE_PERMISSION: &str = "ai:template:smoke";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/templates", get(list_templates))
        .route("/ai/templates/:code", get(get_template))
        .route("/ai/templates/packages", post(generate_package))
        .route("/ai/templates/packages/apply", post(apply_package))
        .route("/ai/templates/smoke/runs", post(run_smoke))
}

async fn list_templates(
    current_user: CurrentUser,
    Query(query): Query<DeliveryTemplateQuery>,
) -> Result<Json<ApiResponse<PageResult<DeliveryTemplate>>>, AppError> {
    require_permission(&current_user, TEMPLATE_LIST_PERMISSION)?;

    Ok(Json(ApiResponse::ok(list_delivery_templates(query)?)))
}

async fn get_template(
    current_user: CurrentUser,
    Path(code): Path<String>,
) -> Result<Json<ApiResponse<DeliveryTemplate>>, AppError> {
    require_permission(&current_user, TEMPLATE_LIST_PERMISSION)?;

    Ok(Json(ApiResponse::ok(get_delivery_template(&code)?)))
}

async fn generate_package(
    State(_state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<CustomerPackageCommand>,
) -> Result<Json<ApiResponse<CustomerPackageResp>>, AppError> {
    require_permission(&current_user, TEMPLATE_INIT_PERMISSION)?;

    Ok(Json(ApiResponse::ok(build_customer_package(command)?)))
}

async fn apply_package(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<CustomerPackageCommand>,
) -> Result<Json<ApiResponse<CustomerPackageApplyResp>>, AppError> {
    require_permission(&current_user, TEMPLATE_INIT_PERMISSION)?;

    Ok(Json(ApiResponse::ok(
        apply_customer_package(&state.db, current_user.id, command).await?,
    )))
}

async fn run_smoke(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<TemplateSmokeRunCommand>,
) -> Result<Json<ApiResponse<TemplateSmokeRunResp>>, AppError> {
    require_permission(&current_user, TEMPLATE_SMOKE_PERMISSION)?;

    Ok(Json(ApiResponse::ok(
        run_template_smoke(&state.db, current_user.tenant_id, current_user.id, command).await?,
    )))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        extract::State,
        http::{header, Request, StatusCode},
        Json,
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::*;
    use crate::{
        application::ai::template_service::CustomerPackageCommand,
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
    fn delivery_template_permission_seed_contains_route_permissions() {
        let seed =
            include_str!("../../../../migrations/202606050012_seed_ai_template_permissions.sql");

        assert!(seed.contains(TEMPLATE_LIST_PERMISSION));
        assert!(seed.contains(TEMPLATE_INIT_PERMISSION));
        assert!(seed.contains("ai:template:smoke"));
    }

    #[test]
    fn delivery_template_smoke_route_is_registered_with_smoke_permission() {
        let source = include_str!("template.rs");
        let route = ["/ai/templates/", "smoke/runs"].concat();
        let permission_const = ["TEMPLATE_", "SMOKE_PERMISSION"].concat();
        let runner = ["run_template_", "smoke"].concat();

        assert!(source.contains(&route));
        assert!(source.contains(&permission_const));
        assert!(source.contains(&runner));
    }

    #[tokio::test]
    async fn delivery_template_package_handler_rejects_missing_permission() {
        let err = generate_package(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(CustomerPackageCommand {
                template_code: "training_app".to_owned(),
                customer_name: "Acme".to_owned(),
                app_name: "Acme Training".to_owned(),
                industry: None,
                brand_name: None,
                primary_color: None,
                public_url: None,
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn delivery_template_package_apply_handler_rejects_missing_permission() {
        let err = apply_package(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(CustomerPackageCommand {
                template_code: "training_app".to_owned(),
                customer_name: "Acme".to_owned(),
                app_name: "Acme Training".to_owned(),
                industry: None,
                brand_name: None,
                primary_color: None,
                public_url: None,
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn delivery_template_smoke_handler_rejects_missing_permission() {
        let err = run_smoke(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(TemplateSmokeRunCommand {
                template_code: "training_app".to_owned(),
                package_id: None,
                dry_run: true,
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn delivery_template_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/templates/packages")
                    .method("POST")
                    .header(header::ACCEPT, "application/json")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"templateCode":"training_app","customerName":"Acme","appName":"Acme Training"}"#))
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
    async fn delivery_template_apply_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/templates/packages/apply")
                    .method("POST")
                    .header(header::ACCEPT, "application/json")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"templateCode":"training_app","customerName":"Acme","appName":"Acme Training"}"#))
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
    async fn delivery_template_smoke_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/templates/smoke/runs")
                    .method("POST")
                    .header(header::ACCEPT, "application/json")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"templateCode":"training_app","dryRun":true}"#,
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
}
