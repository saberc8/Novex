use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};

use crate::{
    application::ai::trigger_service::{
        TriggerEventQuery, TriggerEventResp, TriggerService, TriggerWebhookCommand,
        TriggerWebhookResp,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, pagination::PageResult, response::ApiResponse},
};

const SIGNATURE_HEADER: &str = "X-Novex-Signature";
const IDEMPOTENCY_HEADER: &str = "Idempotency-Key";
const TRIGGER_LIST_PERMISSION: &str = "ai:trigger:list";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/triggers/events", get(list_trigger_events))
        .route("/ai/triggers/webhook/:trigger_code", post(receive_webhook))
}

async fn receive_webhook(
    State(state): State<AppState>,
    Path(trigger_code): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ApiResponse<TriggerWebhookResp>>, AppError> {
    let signature = required_header(&headers, SIGNATURE_HEADER, "Webhook 签名不能为空")?;
    let idempotency_key = required_header(&headers, IDEMPOTENCY_HEADER, "幂等键不能为空")?;
    let service = TriggerService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service
            .receive_webhook(TriggerWebhookCommand {
                trigger_code,
                signature: signature.to_owned(),
                idempotency_key: idempotency_key.to_owned(),
                body: body.to_vec(),
            })
            .await?,
    )))
}

async fn list_trigger_events(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<TriggerEventQuery>,
) -> Result<Json<ApiResponse<PageResult<TriggerEventResp>>>, AppError> {
    require_permission(&current_user, TRIGGER_LIST_PERMISSION)?;
    let service = TriggerService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.list_events(query).await?)))
}

fn required_header<'a>(
    headers: &'a HeaderMap,
    name: &'static str,
    message: &'static str,
) -> Result<&'a str, AppError> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request(message))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{header, Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::*;
    use crate::{infrastructure::security::jwt::JwtService, interfaces::http::build_router};

    #[test]
    fn trigger_event_migration_defines_signature_and_idempotency_contract() {
        let migration =
            include_str!("../../../../migrations/202606060004_create_ai_trigger_event.sql");

        for required in [
            "ai_trigger_event",
            "signature_secret_ref",
            "idempotency_key",
            "uk_ai_trigger_event_tenant_trigger_idempotency",
            "idx_ai_trigger_event_trace_id",
        ] {
            assert!(migration.contains(required), "missing {required}");
        }
    }

    #[tokio::test]
    async fn trigger_webhook_route_requires_signature_before_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/triggers/webhook/training")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"event":"training.completed"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice::<Value>(&body).unwrap();
        assert_eq!(body["code"], "400");
        assert!(body["msg"].as_str().unwrap().contains("签名"));
    }

    #[tokio::test]
    async fn trigger_webhook_route_validates_json_before_database_lookup() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/triggers/webhook/training")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header("X-Novex-Signature", "sha256=abc")
                    .header("Idempotency-Key", "tenant-1:event-7")
                    .body(Body::from("not-json"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice::<Value>(&body).unwrap();
        assert_eq!(body["code"], "400");
        assert!(body["msg"].as_str().unwrap().contains("JSON"));
    }

    #[tokio::test]
    async fn trigger_event_list_handler_rejects_missing_permission() {
        let err = list_trigger_events(
            axum::extract::State(crate::interfaces::http::AppState {
                db: PgPoolOptions::new()
                    .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
                    .unwrap(),
                jwt: JwtService::new("test-secret".to_owned(), 24),
                captcha: Default::default(),
                scheduler_http_safety: Default::default(),
                parser_callback_token: None,
                parser_callback_user_id: 1,
            }),
            crate::domain::auth::model::CurrentUser {
                id: 1,
                tenant_id: 1,
                username: "tester".to_owned(),
                dept_id: 1,
                roles: vec![],
                permissions: vec![],
            },
            axum::extract::Query(
                crate::application::ai::trigger_service::TriggerEventQuery::default(),
            ),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, crate::shared::error::AppError::Forbidden));
    }

    #[tokio::test]
    async fn trigger_event_list_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/triggers/events")
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
