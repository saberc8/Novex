use axum::{routing::post, Json, Router};

use crate::{
    application::ai::research_radar_service::{
        ResearchRadarScanCommand, ResearchRadarScanResp, ResearchRadarService,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, response::ApiResponse},
};

pub const RESEARCH_RADAR_SCAN_PERMISSION: &str = "ai:research-radar:scan";

pub fn routes() -> Router<AppState> {
    Router::new().route("/ai/research-radar/scans", post(scan))
}

async fn scan(
    current_user: CurrentUser,
    Json(command): Json<ResearchRadarScanCommand>,
) -> Result<Json<ApiResponse<ResearchRadarScanResp>>, AppError> {
    require_permission(&current_user, RESEARCH_RADAR_SCAN_PERMISSION)?;

    Ok(Json(ApiResponse::ok(
        ResearchRadarService::new().scan(command).await?,
    )))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{header, Request, StatusCode},
        Json,
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::*;
    use crate::{
        application::ai::research_radar_service::{
            ResearchRadarRanking, ResearchRadarScanCommand, ResearchRadarSource,
        },
        domain::auth::model::{CurrentUser, RoleContext},
        infrastructure::security::jwt::JwtService,
        interfaces::http::build_router,
        shared::error::AppError,
    };

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
    async fn research_radar_scan_handler_rejects_missing_permission() {
        let err = scan(
            user_with_permissions(vec![]),
            Json(ResearchRadarScanCommand {
                topic: "agent workflow".to_owned(),
                sources: vec![ResearchRadarSource::Arxiv],
                ranking: ResearchRadarRanking::Balanced,
                limit_per_source: Some(1),
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn research_radar_scan_handler_runs_with_permission() {
        let response = scan(
            user_with_permissions(vec!["ai:research-radar:scan"]),
            Json(ResearchRadarScanCommand {
                topic: "agent workflow".to_owned(),
                sources: vec![ResearchRadarSource::Paperswithcode],
                ranking: ResearchRadarRanking::Balanced,
                limit_per_source: Some(1),
            }),
        )
        .await
        .unwrap();

        assert_eq!(response.0.code, "200");
        assert_eq!(response.0.data.topic, "agent workflow");
    }

    #[tokio::test]
    async fn research_radar_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:62602".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/research-radar/scans")
                    .method("POST")
                    .header(header::ACCEPT, "application/json")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"topic":"agent workflow"}"#))
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
    fn research_radar_permission_seed_contains_scan_permission() {
        let seed =
            include_str!("../../../../migrations/202606250001_seed_research_radar_permission.sql");

        assert!(seed.contains("ai:research-radar:scan"));
        assert!(seed.contains("Research Radar"));
        assert!(seed.contains("ON CONFLICT DO NOTHING"));
    }
}
