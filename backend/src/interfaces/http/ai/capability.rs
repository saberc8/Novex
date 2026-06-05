use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};

use crate::{
    application::ai::capability_service::{
        CapabilityItemResp, CapabilityQuery, CapabilityService, CapabilitySummaryResp,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, pagination::PageResult, response::ApiResponse},
};

const CAPABILITY_SUMMARY_PERMISSION: &str = "ai:foundation:read";
const TOOL_LIST_PERMISSION: &str = "ai:tool:list";
const CONNECTOR_LIST_PERMISSION: &str = "ai:connector:list";
const PLUGIN_LIST_PERMISSION: &str = "ai:plugin:list";
const TRIGGER_LIST_PERMISSION: &str = "ai:trigger:list";
const MCP_LIST_PERMISSION: &str = "ai:mcp:list";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/capabilities/summary", get(summary))
        .route("/ai/capabilities/tools", get(list_tools))
        .route("/ai/capabilities/connectors", get(list_connectors))
        .route("/ai/capabilities/plugins", get(list_plugins))
        .route("/ai/capabilities/triggers", get(list_triggers))
        .route("/ai/capabilities/mcp-servers", get(list_mcp_servers))
}

async fn summary(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<CapabilitySummaryResp>>, AppError> {
    require_permission(&current_user, CAPABILITY_SUMMARY_PERMISSION)?;
    let service = CapabilityService::new(state.db);

    Ok(Json(ApiResponse::ok(service.summary().await?)))
}

async fn list_tools(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<CapabilityQuery>,
) -> Result<Json<ApiResponse<PageResult<CapabilityItemResp>>>, AppError> {
    require_permission(&current_user, TOOL_LIST_PERMISSION)?;
    let service = CapabilityService::new(state.db);

    Ok(Json(ApiResponse::ok(service.list_tools(query).await?)))
}

async fn list_connectors(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<CapabilityQuery>,
) -> Result<Json<ApiResponse<PageResult<CapabilityItemResp>>>, AppError> {
    require_permission(&current_user, CONNECTOR_LIST_PERMISSION)?;
    let service = CapabilityService::new(state.db);

    Ok(Json(ApiResponse::ok(service.list_connectors(query).await?)))
}

async fn list_plugins(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<CapabilityQuery>,
) -> Result<Json<ApiResponse<PageResult<CapabilityItemResp>>>, AppError> {
    require_permission(&current_user, PLUGIN_LIST_PERMISSION)?;
    let service = CapabilityService::new(state.db);

    Ok(Json(ApiResponse::ok(service.list_plugins(query).await?)))
}

async fn list_triggers(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<CapabilityQuery>,
) -> Result<Json<ApiResponse<PageResult<CapabilityItemResp>>>, AppError> {
    require_permission(&current_user, TRIGGER_LIST_PERMISSION)?;
    let service = CapabilityService::new(state.db);

    Ok(Json(ApiResponse::ok(service.list_triggers(query).await?)))
}

async fn list_mcp_servers(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<CapabilityQuery>,
) -> Result<Json<ApiResponse<PageResult<CapabilityItemResp>>>, AppError> {
    require_permission(&current_user, MCP_LIST_PERMISSION)?;
    let service = CapabilityService::new(state.db);

    Ok(Json(ApiResponse::ok(
        service.list_mcp_servers(query).await?,
    )))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        extract::{Query, State},
        http::{header, Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::*;
    use crate::{
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
    fn capability_permissions_match_seeded_menu_permissions() {
        assert_eq!(TOOL_LIST_PERMISSION, "ai:tool:list");
        assert_eq!(CONNECTOR_LIST_PERMISSION, "ai:connector:list");
        assert_eq!(PLUGIN_LIST_PERMISSION, "ai:plugin:list");
        assert_eq!(TRIGGER_LIST_PERMISSION, "ai:trigger:list");
        assert_eq!(MCP_LIST_PERMISSION, "ai:mcp:list");
    }

    #[test]
    fn capability_query_defaults_to_enabled_poc_records() {
        let query = CapabilityQuery::default();

        assert_eq!(query.page_query().limit(), 20);
        assert_eq!(query.status, Some(1));
    }

    #[tokio::test]
    async fn capability_list_handler_rejects_missing_permission() {
        let err = list_tools(
            State(test_state()),
            user_with_permissions(vec![]),
            Query(CapabilityQuery::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn capability_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/capabilities/summary")
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
