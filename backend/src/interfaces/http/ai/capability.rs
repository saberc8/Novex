use axum::{
    extract::{Multipart, Path, Query, State},
    routing::{get, post},
    Json, Router,
};

use crate::{
    application::ai::capability_service::{
        CapabilityItemResp, CapabilityQuery, CapabilityService, CapabilitySummaryResp,
        ConnectorCredentialCommand, ConnectorCredentialQuery, ConnectorCredentialResp,
        McpDiscoveryCommand, McpServerCommand, McpServerResp, McpToolResp, PluginInstallCommand,
        PluginInstallationQuery, PluginInstallationResp, SkillImportFromSourceCommand,
        SkillImportPreviewCommand, SkillImportPreviewResp, SkillImportResultResp,
        ToolCallAuditQuery, ToolCallAuditResp, ToolDryRunCommand, ToolDryRunResp,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, pagination::PageResult, response::ApiResponse},
};

const CAPABILITY_SUMMARY_PERMISSION: &str = "ai:foundation:read";
const SKILL_LIST_PERMISSION: &str = "ai:skill:list";
const SKILL_IMPORT_PERMISSION: &str = "ai:skill:import";
const TOOL_LIST_PERMISSION: &str = "ai:tool:list";
const CONNECTOR_LIST_PERMISSION: &str = "ai:connector:list";
const CONNECTOR_CREDENTIAL_UPDATE_PERMISSION: &str = "ai:connector:credential:update";
const PLUGIN_LIST_PERMISSION: &str = "ai:plugin:list";
const PLUGIN_INSTALL_PERMISSION: &str = "ai:plugin:install";
const TRIGGER_LIST_PERMISSION: &str = "ai:trigger:list";
const MCP_LIST_PERMISSION: &str = "ai:mcp:list";
const MCP_UPDATE_PERMISSION: &str = "ai:mcp:update";
const TOOL_DRY_RUN_PERMISSION: &str = "ai:tool:dryRun";
const TOOL_AUDIT_LIST_PERMISSION: &str = "ai:tool:audit:list";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ai/capabilities/summary", get(summary))
        .route("/ai/capabilities/skills", get(list_skills))
        .route("/ai/capabilities/skills/import", post(import_skill))
        .route(
            "/ai/capabilities/skills/import/preview",
            post(preview_skill_import),
        )
        .route(
            "/ai/capabilities/skills/import/source",
            post(import_skill_from_source),
        )
        .route(
            "/ai/capabilities/skills/import/package",
            post(import_skill_package),
        )
        .route(
            "/ai/capabilities/tools/dry-run",
            axum::routing::post(dry_run_tool),
        )
        .route("/ai/capabilities/tools/audits", get(list_tool_audits))
        .route("/ai/capabilities/tools", get(list_tools))
        .route(
            "/ai/capabilities/connectors/credentials",
            get(list_connector_credentials).post(upsert_connector_credential),
        )
        .route("/ai/capabilities/connectors", get(list_connectors))
        .route(
            "/ai/capabilities/plugins/installations",
            get(list_plugin_installations).post(install_plugin),
        )
        .route("/ai/capabilities/plugins", get(list_plugins))
        .route("/ai/capabilities/triggers", get(list_triggers))
        .route(
            "/ai/capabilities/mcp/servers",
            get(list_mcp_servers).post(upsert_mcp_server),
        )
        .route(
            "/ai/capabilities/mcp/servers/:server_id/discover",
            post(discover_mcp_tools),
        )
        .route(
            "/ai/capabilities/mcp/servers/:server_id/tools",
            get(list_mcp_tools),
        )
        .route(
            "/ai/capabilities/mcp-servers",
            get(list_mcp_servers).post(upsert_mcp_server),
        )
}

async fn list_connector_credentials(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<ConnectorCredentialQuery>,
) -> Result<Json<ApiResponse<PageResult<ConnectorCredentialResp>>>, AppError> {
    require_permission(&current_user, CONNECTOR_LIST_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.list_connector_credentials(query).await?,
    )))
}

async fn upsert_connector_credential(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<ConnectorCredentialCommand>,
) -> Result<Json<ApiResponse<ConnectorCredentialResp>>, AppError> {
    require_permission(&current_user, CONNECTOR_CREDENTIAL_UPDATE_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service
            .upsert_connector_credential(current_user.id, command)
            .await?,
    )))
}

async fn summary(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<CapabilitySummaryResp>>, AppError> {
    require_permission(&current_user, CAPABILITY_SUMMARY_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.summary().await?)))
}

async fn list_tools(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<CapabilityQuery>,
) -> Result<Json<ApiResponse<PageResult<CapabilityItemResp>>>, AppError> {
    require_permission(&current_user, TOOL_LIST_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.list_tools(query).await?)))
}

async fn list_skills(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<CapabilityQuery>,
) -> Result<Json<ApiResponse<PageResult<CapabilityItemResp>>>, AppError> {
    require_permission(&current_user, SKILL_LIST_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.list_skills(query).await?)))
}

async fn import_skill(
    State(state): State<AppState>,
    current_user: CurrentUser,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<CapabilityItemResp>>, AppError> {
    require_permission(&current_user, SKILL_IMPORT_PERMISSION)?;
    let file = multipart_skill_file(&mut multipart).await?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service
            .import_skill(current_user.id, file.filename.as_deref(), &file.bytes)
            .await?,
    )))
}

async fn preview_skill_import(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<SkillImportPreviewCommand>,
) -> Result<Json<ApiResponse<SkillImportPreviewResp>>, AppError> {
    require_permission(&current_user, SKILL_IMPORT_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.preview_skill_import(command).await?,
    )))
}

async fn import_skill_from_source(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<SkillImportFromSourceCommand>,
) -> Result<Json<ApiResponse<SkillImportResultResp>>, AppError> {
    require_permission(&current_user, SKILL_IMPORT_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service
            .import_skill_from_source(current_user.id, command)
            .await?,
    )))
}

async fn import_skill_package(
    State(state): State<AppState>,
    current_user: CurrentUser,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<SkillImportResultResp>>, AppError> {
    require_permission(&current_user, SKILL_IMPORT_PERMISSION)?;
    let file = multipart_skill_file(&mut multipart).await?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service
            .import_skill_package(current_user.id, file.filename.as_deref(), &file.bytes)
            .await?,
    )))
}

async fn list_connectors(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<CapabilityQuery>,
) -> Result<Json<ApiResponse<PageResult<CapabilityItemResp>>>, AppError> {
    require_permission(&current_user, CONNECTOR_LIST_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.list_connectors(query).await?)))
}

async fn list_plugins(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<CapabilityQuery>,
) -> Result<Json<ApiResponse<PageResult<CapabilityItemResp>>>, AppError> {
    require_permission(&current_user, PLUGIN_LIST_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.list_plugins(query).await?)))
}

async fn list_plugin_installations(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<PluginInstallationQuery>,
) -> Result<Json<ApiResponse<PageResult<PluginInstallationResp>>>, AppError> {
    require_permission(&current_user, PLUGIN_LIST_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.list_plugin_installations(query).await?,
    )))
}

async fn install_plugin(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<PluginInstallCommand>,
) -> Result<Json<ApiResponse<PluginInstallationResp>>, AppError> {
    require_permission(&current_user, PLUGIN_INSTALL_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.install_plugin(current_user.id, command).await?,
    )))
}

async fn list_triggers(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<CapabilityQuery>,
) -> Result<Json<ApiResponse<PageResult<CapabilityItemResp>>>, AppError> {
    require_permission(&current_user, TRIGGER_LIST_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(service.list_triggers(query).await?)))
}

async fn list_mcp_servers(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<CapabilityQuery>,
) -> Result<Json<ApiResponse<PageResult<CapabilityItemResp>>>, AppError> {
    require_permission(&current_user, MCP_LIST_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.list_mcp_servers(query).await?,
    )))
}

async fn upsert_mcp_server(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<McpServerCommand>,
) -> Result<Json<ApiResponse<McpServerResp>>, AppError> {
    require_permission(&current_user, MCP_UPDATE_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.upsert_mcp_server(current_user.id, command).await?,
    )))
}

async fn discover_mcp_tools(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(server_id): Path<i64>,
    Json(command): Json<McpDiscoveryCommand>,
) -> Result<Json<ApiResponse<Vec<McpToolResp>>>, AppError> {
    require_permission(&current_user, MCP_UPDATE_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service
            .discover_mcp_tools(current_user.id, server_id, command)
            .await?,
    )))
}

async fn list_mcp_tools(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(server_id): Path<i64>,
) -> Result<Json<ApiResponse<Vec<McpToolResp>>>, AppError> {
    require_permission(&current_user, MCP_LIST_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.list_mcp_tools(server_id).await?,
    )))
}

async fn dry_run_tool(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(command): Json<ToolDryRunCommand>,
) -> Result<Json<ApiResponse<ToolDryRunResp>>, AppError> {
    require_permission(&current_user, TOOL_DRY_RUN_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.dry_run_tool(current_user.id, command).await?,
    )))
}

async fn list_tool_audits(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<ToolCallAuditQuery>,
) -> Result<Json<ApiResponse<PageResult<ToolCallAuditResp>>>, AppError> {
    require_permission(&current_user, TOOL_AUDIT_LIST_PERMISSION)?;
    let service = CapabilityService::for_tenant(state.db, current_user.tenant_id);

    Ok(Json(ApiResponse::ok(
        service.list_tool_audits(query).await?,
    )))
}

struct SkillUploadFile {
    filename: Option<String>,
    bytes: Vec<u8>,
}

async fn multipart_skill_file(multipart: &mut Multipart) -> Result<SkillUploadFile, AppError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad_request("Skill 导入文件解析失败"))?
    {
        let is_file_field = field.name() == Some("file") || field.file_name().is_some();
        if !is_file_field {
            continue;
        }
        let filename = field.file_name().map(ToOwned::to_owned);
        let bytes = field
            .bytes()
            .await
            .map_err(|_| AppError::bad_request("Skill 导入文件读取失败"))?
            .to_vec();
        return Ok(SkillUploadFile { filename, bytes });
    }

    Err(AppError::bad_request("Skill 导入文件不能为空"))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        extract::{FromRequest, Query, State},
        http::{header, Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::*;
    use crate::{
        application::ai::capability_service::McpDiscoveryToolCommand,
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
    fn capability_permissions_match_seeded_menu_permissions() {
        assert_eq!(TOOL_LIST_PERMISSION, "ai:tool:list");
        assert_eq!(SKILL_LIST_PERMISSION, "ai:skill:list");
        assert_eq!(SKILL_IMPORT_PERMISSION, "ai:skill:import");
        assert_eq!(CONNECTOR_LIST_PERMISSION, "ai:connector:list");
        assert_eq!(
            CONNECTOR_CREDENTIAL_UPDATE_PERMISSION,
            "ai:connector:credential:update"
        );
        assert_eq!(PLUGIN_LIST_PERMISSION, "ai:plugin:list");
        assert_eq!(PLUGIN_INSTALL_PERMISSION, "ai:plugin:install");
        assert_eq!(TRIGGER_LIST_PERMISSION, "ai:trigger:list");
        assert_eq!(MCP_LIST_PERMISSION, "ai:mcp:list");
        assert_eq!(MCP_UPDATE_PERMISSION, "ai:mcp:update");
        assert_eq!(TOOL_DRY_RUN_PERMISSION, "ai:tool:dryRun");
        assert_eq!(TOOL_AUDIT_LIST_PERMISSION, "ai:tool:audit:list");
    }

    #[test]
    fn capability_query_defaults_to_enabled_poc_records() {
        let query = CapabilityQuery::default();

        assert_eq!(query.page_query().limit(), 20);
        assert_eq!(query.status, Some(1));
    }

    #[test]
    fn capability_handlers_bind_runtime_to_current_tenant() {
        let source = include_str!("capability.rs");

        assert!(
            source
                .matches("CapabilityService::for_tenant(state.db, current_user.tenant_id)")
                .count()
                >= 15
        );
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
    async fn skill_list_handler_rejects_missing_permission() {
        let err = list_skills(
            State(test_state()),
            user_with_permissions(vec![]),
            Query(CapabilityQuery::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn skill_import_handler_rejects_missing_permission() {
        let boundary = "novex-test-boundary";
        let body = format!(
            "--{boundary}\r\n\
Content-Disposition: form-data; name=\"file\"; filename=\"SKILL.md\"\r\n\
Content-Type: text/markdown\r\n\r\n\
---\nname: demo-skill\ndescription: Demo skill.\n---\n# Demo\n\r\n\
--{boundary}--\r\n"
        );
        let multipart = axum::extract::Multipart::from_request(
            Request::builder()
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
            &(),
        )
        .await
        .unwrap();

        let err = import_skill(
            State(test_state()),
            user_with_permissions(vec![]),
            multipart,
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn skill_ai_import_handlers_reject_missing_permission() {
        let boundary = "novex-test-boundary";
        let body = format!(
            "--{boundary}\r\n\
Content-Disposition: form-data; name=\"file\"; filename=\"skills.zip\"\r\n\
Content-Type: application/zip\r\n\r\n\
zip-bytes\r\n\
--{boundary}--\r\n"
        );
        let multipart = axum::extract::Multipart::from_request(
            Request::builder()
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
            &(),
        )
        .await
        .unwrap();
        let preview_err = preview_skill_import(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(SkillImportPreviewCommand {
                source: "https://github.com/KKKKhazix/khazix-skills".to_owned(),
            }),
        )
        .await
        .unwrap_err();
        let source_err = import_skill_from_source(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(SkillImportFromSourceCommand {
                source: "https://github.com/KKKKhazix/khazix-skills".to_owned(),
                skill_path: Some("khazix-writer".to_owned()),
            }),
        )
        .await
        .unwrap_err();
        let package_err = import_skill_package(
            State(test_state()),
            user_with_permissions(vec![]),
            multipart,
        )
        .await
        .unwrap_err();

        assert!(matches!(preview_err, AppError::Forbidden));
        assert!(matches!(source_err, AppError::Forbidden));
        assert!(matches!(package_err, AppError::Forbidden));
    }

    #[test]
    fn skill_list_permission_seed_contains_skill_menu() {
        let seed = include_str!("../../../../migrations/202606050001_seed_ai_foundation_menus.sql");
        let import_seed =
            include_str!("../../../../migrations/202606090002_seed_skill_import_permission.sql");
        assert!(seed.contains("/ai/skills"));
        assert!(seed.contains("ai:skill:list"));
        assert!(import_seed.contains("/ai/skills"));
        assert!(import_seed.contains("ai:skill:import"));
    }

    #[tokio::test]
    async fn tool_dry_run_handler_rejects_missing_permission() {
        let err = dry_run_tool(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(ToolDryRunCommand {
                tool_code: "rag.search".to_owned(),
                input: Value::Null,
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn tool_audit_list_handler_rejects_missing_permission() {
        let err = list_tool_audits(
            State(test_state()),
            user_with_permissions(vec![]),
            Query(ToolCallAuditQuery::default()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn connector_credential_upsert_handler_rejects_missing_permission() {
        let err = upsert_connector_credential(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(ConnectorCredentialCommand {
                connector_code: "github.default".to_owned(),
                scope_type: "tenant".to_owned(),
                scope_id: "1".to_owned(),
                auth_type: "oauth_app".to_owned(),
                secret_ref: "env:GITHUB_CONNECTOR_TOKEN".to_owned(),
                scopes: serde_json::json!(["repo"]),
                status: 1,
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[test]
    fn connector_credential_permission_seed_contains_update_permission() {
        let seed = include_str!("../../../../migrations/202606050001_seed_ai_foundation_menus.sql");
        assert!(seed.contains("ai:connector:credential:update"));
    }

    #[tokio::test]
    async fn plugin_install_handler_rejects_missing_permission() {
        let err = install_plugin(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(PluginInstallCommand {
                plugin_code: "builtin.github-basic".to_owned(),
                version: "0.1.0".to_owned(),
                enabled: true,
                permission_grants: serde_json::json!(["ai:connector:list"]),
                config: serde_json::json!({}),
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[test]
    fn plugin_install_permission_seed_contains_install_permission() {
        let seed = include_str!("../../../../migrations/202606050001_seed_ai_foundation_menus.sql");
        assert!(seed.contains("ai:plugin:install"));
    }

    #[tokio::test]
    async fn mcp_server_upsert_handler_rejects_missing_permission() {
        let err = upsert_mcp_server(
            State(test_state()),
            user_with_permissions(vec![]),
            Json(McpServerCommand {
                code: "docs.search".to_owned(),
                name: "Docs Search".to_owned(),
                endpoint_url: Some("https://mcp.example.com/sse".to_owned()),
                transport_kind: "streamable_http".to_owned(),
                auth_scope: "tenant".to_owned(),
                auth_type: "bearer_env".to_owned(),
                secret_ref: Some("env:DOCS_MCP_TOKEN".to_owned()),
                network_allowlist: serde_json::json!(["mcp.example.com"]),
                tool_allowlist: serde_json::json!(["docs.search"]),
                discovered_tools: serde_json::json!([]),
                enabled: true,
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn mcp_discovery_handler_rejects_missing_permission() {
        let err = discover_mcp_tools(
            State(test_state()),
            user_with_permissions(vec![]),
            Path(42),
            Json(McpDiscoveryCommand {
                tools: vec![McpDiscoveryToolCommand {
                    tool_name: "search".to_owned(),
                    description: "Search docs".to_owned(),
                    input_schema: serde_json::json!({"type":"object"}),
                    output_schema: serde_json::json!({}),
                    risk_level: 1,
                    metadata: serde_json::json!({}),
                    enabled: true,
                }],
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[tokio::test]
    async fn mcp_tool_list_handler_rejects_missing_permission() {
        let err = list_mcp_tools(State(test_state()), user_with_permissions(vec![]), Path(42))
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Forbidden));
    }

    #[test]
    fn mcp_server_update_permission_seed_contains_update_permission() {
        let seed = include_str!("../../../../migrations/202606050001_seed_ai_foundation_menus.sql");
        assert!(seed.contains("ai:mcp:update"));
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

    #[tokio::test]
    async fn skill_list_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ai/capabilities/skills")
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
    async fn tool_dry_run_route_is_registered_and_requires_auth() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let jwt = JwtService::new("test-secret".to_owned(), 24);
        let app = build_router(db, &["http://localhost:4399".to_owned()], jwt).unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/capabilities/tools/dry-run")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"toolCode":"rag.search","input":{}}"#))
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
