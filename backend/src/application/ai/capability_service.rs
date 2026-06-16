use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use novex_mcp::{
    mcp_tool_code, validate_mcp_registration_policy, McpAuthScope, McpAuthType, McpDiscoveredTool,
    McpRegistrationPolicy, McpTransportKind,
};
use novex_skill::{
    normalize_skill_package_path as normalize_skill_package_path_core,
    normalize_skill_package_path_or_empty as normalize_skill_package_path_or_empty_core,
    selected_skill_md_index as selected_skill_md_index_core, skill_resource_kind,
    skill_root_from_skill_md_path, strip_skill_root, SkillPackageError, SkillPackagePath,
    SkillResourceKind,
};
use novex_tools::ToolRiskLevel;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::{env, io::Read};
use url::Url;

use crate::{
    application::system::{ensure_max_chars, format_datetime},
    infrastructure::persistence::ai_capability_repository::{
        AiCapabilityRepository, CapabilityFilter, CapabilityRecord, CapabilityResource,
        ConnectorCredentialFilter, ConnectorCredentialRecord, ConnectorCredentialSaveRecord,
        McpServerRecord, McpServerSaveRecord, McpToolRecord, McpToolSaveRecord,
        PluginInstallationFilter, PluginInstallationRecord, PluginInstallationSaveRecord,
        SkillResourceSaveRecord, SkillSaveRecord, ToolAuditFilter, ToolAuditRecord,
        ToolAuditSaveRecord, ToolSaveRecord,
    },
    shared::{
        error::AppError,
        id::next_id,
        pagination::{PageQuery, PageResult, DEFAULT_PAGE},
    },
};

const DEFAULT_TENANT_ID: i64 = 1;
const DEFAULT_CAPABILITY_PAGE_SIZE: u64 = 20;
const ENABLED_STATUS: i16 = 1;
const MAX_SKILL_IMPORT_BYTES: usize = 512 * 1024;
const MAX_SKILL_PACKAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_SKILL_PACKAGE_FILES: usize = 200;
const MAX_SKILL_PACKAGE_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_SKILL_PREVIEW_ITEMS: usize = 20;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_capability_size")]
    pub size: u64,
    #[serde(default = "default_enabled_status")]
    pub status: Option<i16>,
    #[serde(default)]
    pub kind: Option<String>,
}

impl Default for CapabilityQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            size: DEFAULT_CAPABILITY_PAGE_SIZE,
            status: Some(ENABLED_STATUS),
            kind: None,
        }
    }
}

impl CapabilityQuery {
    pub fn page_query(&self) -> PageQuery {
        PageQuery {
            page: self.page,
            size: self.size,
        }
        .normalized()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitySummaryResp {
    pub skill_count: i64,
    pub tool_count: i64,
    pub connector_count: i64,
    pub plugin_count: i64,
    pub trigger_count: i64,
    pub mcp_server_count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityItemResp {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub status: i16,
    pub risk_level: Option<i16>,
    pub metadata: Value,
    pub create_time: String,
}

#[derive(Debug, Clone)]
pub struct SkillImportCommand {
    pub code: String,
    pub name: String,
    pub description: String,
    pub model_route_policy: Value,
    pub capability_refs: Value,
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct SkillImportFile {
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

impl SkillPackagePath for SkillImportFile {
    fn relative_path(&self) -> &str {
        &self.relative_path
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkillImportResourceCommand {
    pub resource_type: String,
    pub relative_path: String,
    pub mime_type: String,
    pub content_text: Option<String>,
    pub storage_ref: Option<String>,
    pub content_sha256: String,
    pub size_bytes: i64,
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct SkillDirectoryImportCommand {
    pub skill: SkillImportCommand,
    pub resources: Vec<SkillImportResourceCommand>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubSkillSource {
    pub owner: String,
    pub repo: String,
    pub reference: String,
    pub path: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillImportPreviewCommand {
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillImportFromSourceCommand {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub skill_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillImportPreviewResp {
    pub source_url: String,
    pub skills: Vec<SkillImportPreviewItemResp>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillImportPreviewItemResp {
    pub code: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub reference_count: usize,
    pub script_count: usize,
    pub asset_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillImportResultResp {
    pub skill: CapabilityItemResp,
    pub resource_count: usize,
    pub reference_count: usize,
    pub script_count: usize,
    pub asset_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CodexSkillFrontmatter {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    metadata: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonSkillImport {
    #[serde(default)]
    code: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    instruction: String,
    #[serde(default)]
    skill_md: Option<String>,
    #[serde(default)]
    model_route_policy: Value,
    #[serde(default)]
    capability_refs: Value,
    #[serde(default)]
    metadata: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDryRunCommand {
    #[serde(default)]
    pub tool_code: String,
    #[serde(default)]
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDryRunResp {
    pub audit_id: i64,
    pub tool_code: String,
    pub status: String,
    pub dry_run: bool,
    pub response: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallAuditQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_capability_size")]
    pub size: u64,
    #[serde(default)]
    pub tool_code: Option<String>,
}

impl Default for ToolCallAuditQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            size: DEFAULT_CAPABILITY_PAGE_SIZE,
            tool_code: None,
        }
    }
}

impl ToolCallAuditQuery {
    pub fn page_query(&self) -> PageQuery {
        PageQuery {
            page: self.page,
            size: self.size,
        }
        .normalized()
    }
}

impl ConnectorCredentialQuery {
    pub fn page_query(&self) -> PageQuery {
        PageQuery {
            page: self.page,
            size: self.size,
        }
        .normalized()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallAuditResp {
    pub id: i64,
    pub tool_code: String,
    pub status: String,
    pub dry_run: bool,
    pub risk_level: i16,
    pub permission_code: String,
    pub create_time: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorCredentialQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_capability_size")]
    pub size: u64,
    #[serde(default)]
    pub connector_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorCredentialCommand {
    #[serde(default)]
    pub connector_code: String,
    #[serde(default)]
    pub scope_type: String,
    #[serde(default)]
    pub scope_id: String,
    #[serde(default)]
    pub auth_type: String,
    #[serde(default)]
    pub secret_ref: String,
    #[serde(default)]
    pub scopes: Value,
    #[serde(default = "default_enabled_status_i16")]
    pub status: i16,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorCredentialResp {
    pub id: i64,
    pub connector_id: i64,
    pub connector_code: String,
    pub scope_type: String,
    pub scope_id: String,
    pub auth_type: String,
    pub secret_ref: String,
    pub masked_value: String,
    pub scopes: Value,
    pub status: i16,
    pub create_time: String,
    pub update_time: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallationQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_capability_size")]
    pub size: u64,
    #[serde(default)]
    pub plugin_code: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

impl PluginInstallationQuery {
    pub fn page_query(&self) -> PageQuery {
        PageQuery {
            page: self.page,
            size: self.size,
        }
        .normalized()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallCommand {
    #[serde(default)]
    pub plugin_code: String,
    #[serde(default)]
    pub version: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub permission_grants: Value,
    #[serde(default)]
    pub config: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallationResp {
    pub id: i64,
    pub plugin_id: i64,
    pub plugin_code: String,
    pub plugin_name: String,
    pub version: String,
    pub enabled: bool,
    pub permission_grants: Value,
    pub capabilities: Value,
    pub config: Value,
    pub install_source: String,
    pub create_time: String,
    pub update_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerCommand {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub endpoint_url: Option<String>,
    #[serde(default)]
    pub transport_kind: String,
    #[serde(default)]
    pub auth_scope: String,
    #[serde(default)]
    pub auth_type: String,
    #[serde(default)]
    pub secret_ref: Option<String>,
    #[serde(default)]
    pub network_allowlist: Value,
    #[serde(default)]
    pub tool_allowlist: Value,
    #[serde(default)]
    pub discovered_tools: Value,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerResp {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub endpoint_url: Option<String>,
    pub transport_kind: String,
    pub auth_scope: String,
    pub auth_type: String,
    pub secret_ref: Option<String>,
    pub masked_secret_ref: String,
    pub network_allowlist: Value,
    pub tool_allowlist: Value,
    pub discovered_tools: Value,
    pub enabled: bool,
    pub create_time: String,
    pub update_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpDiscoveryCommand {
    #[serde(default)]
    pub tools: Vec<McpDiscoveryToolCommand>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpDiscoveryToolCommand {
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_json_object")]
    pub input_schema: Value,
    #[serde(default = "default_json_object")]
    pub output_schema: Value,
    #[serde(default = "default_low_risk_level")]
    pub risk_level: i16,
    #[serde(default = "default_json_object")]
    pub metadata: Value,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolResp {
    pub id: i64,
    pub server_id: i64,
    pub server_code: String,
    pub tool_name: String,
    pub tool_code: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub risk_level: i16,
    pub permission_code: Option<String>,
    pub status: i16,
    pub metadata: Value,
    pub create_time: String,
    pub update_time: Option<String>,
}

#[derive(Debug, Clone)]
struct McpDiscoverySavePlan {
    mcp_tools: Vec<McpToolSaveRecord>,
    tools: Vec<ToolSaveRecord>,
    discovered_tools: Value,
}

#[derive(Debug, Clone)]
pub struct CapabilityService {
    tenant_id: i64,
    repo: AiCapabilityRepository,
}

impl CapabilityService {
    pub fn new(db: PgPool) -> Self {
        Self::for_tenant(db, DEFAULT_TENANT_ID)
    }

    pub fn for_tenant(db: PgPool, tenant_id: i64) -> Self {
        Self {
            tenant_id,
            repo: AiCapabilityRepository::new(db),
        }
    }

    pub async fn summary(&self) -> Result<CapabilitySummaryResp, AppError> {
        let filter = summary_filter(self.tenant_id);
        Ok(CapabilitySummaryResp {
            skill_count: self.repo.count(CapabilityResource::Skill, &filter).await?,
            tool_count: self.repo.count(CapabilityResource::Tool, &filter).await?,
            connector_count: self
                .repo
                .count(CapabilityResource::Connector, &filter)
                .await?,
            plugin_count: self.repo.count(CapabilityResource::Plugin, &filter).await?,
            trigger_count: self
                .repo
                .count(CapabilityResource::Trigger, &filter)
                .await?,
            mcp_server_count: self
                .repo
                .count(CapabilityResource::McpServer, &filter)
                .await?,
        })
    }

    pub async fn list_tools(
        &self,
        query: CapabilityQuery,
    ) -> Result<PageResult<CapabilityItemResp>, AppError> {
        self.list(CapabilityResource::Tool, query).await
    }

    pub async fn list_skills(
        &self,
        query: CapabilityQuery,
    ) -> Result<PageResult<CapabilityItemResp>, AppError> {
        self.list(CapabilityResource::Skill, query).await
    }

    pub async fn import_skill(
        &self,
        user_id: i64,
        filename: Option<&str>,
        bytes: &[u8],
    ) -> Result<CapabilityItemResp, AppError> {
        let command = parse_skill_import(filename, bytes)?;
        let record = SkillSaveRecord {
            id: next_id(),
            tenant_id: self.tenant_id,
            code: command.code,
            name: command.name,
            description: command.description,
            status: ENABLED_STATUS,
            model_route_policy: command.model_route_policy,
            capability_refs: command.capability_refs,
            metadata: command.metadata,
            user_id,
            now: Utc::now().naive_utc(),
        };

        Ok(CapabilityItemResp::from(
            self.repo.upsert_skill(&record).await?,
        ))
    }

    pub async fn preview_skill_import(
        &self,
        command: SkillImportPreviewCommand,
    ) -> Result<SkillImportPreviewResp, AppError> {
        let source_url = extract_skill_source_url(&command.source)?;
        let source = parse_github_skill_source(&source_url)?;
        let tree = fetch_github_skill_tree(&source).await?;
        let mut warnings = Vec::new();
        let skill_paths = discover_skill_paths(&source, &tree);
        if skill_paths.len() > MAX_SKILL_PREVIEW_ITEMS {
            warnings.push(format!(
                "仅展示前 {} 个 Skill，建议粘贴更具体的目录地址",
                MAX_SKILL_PREVIEW_ITEMS
            ));
        }

        let mut skills = Vec::new();
        for skill_path in skill_paths.into_iter().take(MAX_SKILL_PREVIEW_ITEMS) {
            let Some(skill_md_path) = skill_md_path_for_dir(&skill_path) else {
                continue;
            };
            let Some(skill_md_item) = tree_item_for_path(&tree, &skill_md_path) else {
                continue;
            };
            let bytes = fetch_github_tree_item_file(&source, skill_md_item).await?;
            let command = parse_skill_import(Some("SKILL.md"), &bytes)?;
            let (reference_count, script_count, asset_count) =
                skill_resource_counts(&tree, &skill_path);
            skills.push(SkillImportPreviewItemResp {
                code: command.code.clone(),
                name: command.name,
                description: command.description,
                path: skill_path,
                reference_count,
                script_count,
                asset_count,
            });
        }

        Ok(SkillImportPreviewResp {
            source_url,
            skills,
            warnings,
        })
    }

    pub async fn import_skill_from_source(
        &self,
        user_id: i64,
        command: SkillImportFromSourceCommand,
    ) -> Result<SkillImportResultResp, AppError> {
        let source_url = extract_skill_source_url(&command.source)?;
        let mut source = parse_github_skill_source(&source_url)?;
        if let Some(skill_path) = command
            .skill_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            source.path = normalize_skill_package_path(skill_path)?;
        }
        let tree = fetch_github_skill_tree(&source).await?;
        let skill_path =
            if source.path.trim().is_empty() || !tree_contains_skill_dir(&tree, &source.path) {
                let skill_paths = discover_skill_paths(&source, &tree);
                match skill_paths.as_slice() {
                    [only] => only.clone(),
                    [] => return Err(AppError::bad_request("未在目标地址发现 SKILL.md")),
                    _ => {
                        return Err(AppError::bad_request(
                            "目标地址包含多个 Skill，请先预览并选择具体目录",
                        ))
                    }
                }
            } else {
                source.path.clone()
            };
        let files = fetch_github_skill_files(&source, &tree, &skill_path).await?;
        self.import_skill_directory_from_files(user_id, Some(&source_url), files)
            .await
    }

    pub async fn import_skill_package(
        &self,
        user_id: i64,
        filename: Option<&str>,
        bytes: &[u8],
    ) -> Result<SkillImportResultResp, AppError> {
        let files = parse_skill_zip_package(filename, bytes)?;
        self.import_skill_directory_from_files(user_id, filename, files)
            .await
    }

    pub async fn import_skill_directory_from_files(
        &self,
        user_id: i64,
        source_url: Option<&str>,
        files: Vec<SkillImportFile>,
    ) -> Result<SkillImportResultResp, AppError> {
        let command = parse_skill_directory_import(source_url, files)?;
        let now = Utc::now().naive_utc();
        let record = SkillSaveRecord {
            id: next_id(),
            tenant_id: self.tenant_id,
            code: command.skill.code,
            name: command.skill.name,
            description: command.skill.description,
            status: ENABLED_STATUS,
            model_route_policy: command.skill.model_route_policy,
            capability_refs: command.skill.capability_refs,
            metadata: command.skill.metadata,
            user_id,
            now,
        };
        let skill = self.repo.upsert_skill(&record).await?;
        let resources = command
            .resources
            .iter()
            .map(|resource| SkillResourceSaveRecord {
                id: next_id(),
                tenant_id: self.tenant_id,
                skill_id: skill.id,
                resource_type: resource.resource_type.clone(),
                relative_path: resource.relative_path.clone(),
                mime_type: resource.mime_type.clone(),
                content_text: resource.content_text.clone(),
                storage_ref: resource.storage_ref.clone(),
                content_sha256: resource.content_sha256.clone(),
                size_bytes: resource.size_bytes,
                metadata: resource.metadata.clone(),
                user_id,
                now,
            })
            .collect::<Vec<_>>();
        self.repo
            .replace_skill_resources(self.tenant_id, skill.id, &resources)
            .await?;

        Ok(skill_import_result(
            skill,
            &command.resources,
            command.warnings,
        ))
    }

    pub async fn list_connectors(
        &self,
        query: CapabilityQuery,
    ) -> Result<PageResult<CapabilityItemResp>, AppError> {
        self.list(CapabilityResource::Connector, query).await
    }

    pub async fn list_plugins(
        &self,
        query: CapabilityQuery,
    ) -> Result<PageResult<CapabilityItemResp>, AppError> {
        self.list(CapabilityResource::Plugin, query).await
    }

    pub async fn list_plugin_installations(
        &self,
        query: PluginInstallationQuery,
    ) -> Result<PageResult<PluginInstallationResp>, AppError> {
        let page = query.page_query();
        let plugin_code = query
            .plugin_code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let filter = PluginInstallationFilter {
            tenant_id: self.tenant_id,
            plugin_code,
            enabled: query.enabled,
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_plugin_installations(&filter).await?;
        let list = self
            .repo
            .list_plugin_installations(&filter)
            .await?
            .into_iter()
            .map(PluginInstallationResp::from)
            .collect();

        Ok(PageResult::new(list, total))
    }

    pub async fn install_plugin(
        &self,
        user_id: i64,
        command: PluginInstallCommand,
    ) -> Result<PluginInstallationResp, AppError> {
        let command = normalize_plugin_install_command(command)?;
        let record = PluginInstallationSaveRecord {
            id: next_id(),
            tenant_id: self.tenant_id,
            plugin_code: command.plugin_code,
            version: command.version,
            enabled: command.enabled,
            permission_grants: command.permission_grants,
            config: command.config,
            user_id,
            now: Utc::now().naive_utc(),
        };
        let Some(record) = self.repo.upsert_plugin_installation(&record).await? else {
            return Err(AppError::NotFound);
        };

        Ok(PluginInstallationResp::from(record))
    }

    pub async fn list_triggers(
        &self,
        query: CapabilityQuery,
    ) -> Result<PageResult<CapabilityItemResp>, AppError> {
        self.list(CapabilityResource::Trigger, query).await
    }

    pub async fn list_mcp_servers(
        &self,
        query: CapabilityQuery,
    ) -> Result<PageResult<CapabilityItemResp>, AppError> {
        self.list(CapabilityResource::McpServer, query).await
    }

    pub async fn upsert_mcp_server(
        &self,
        user_id: i64,
        command: McpServerCommand,
    ) -> Result<McpServerResp, AppError> {
        let command = normalize_mcp_server_command(command)?;
        let record = McpServerSaveRecord {
            id: next_id(),
            tenant_id: self.tenant_id,
            code: command.code,
            name: command.name,
            endpoint_url: command.endpoint_url,
            transport_kind: command.transport_kind,
            auth_scope: command.auth_scope,
            auth_type: command.auth_type,
            secret_ref: command.secret_ref,
            network_allowlist: command.network_allowlist,
            tool_allowlist: command.tool_allowlist,
            discovered_tools: command.discovered_tools,
            status: if command.enabled { ENABLED_STATUS } else { 0 },
            user_id,
            now: Utc::now().naive_utc(),
        };

        Ok(McpServerResp::from(
            self.repo.upsert_mcp_server(&record).await?,
        ))
    }

    pub async fn discover_mcp_tools(
        &self,
        user_id: i64,
        server_id: i64,
        command: McpDiscoveryCommand,
    ) -> Result<Vec<McpToolResp>, AppError> {
        let Some(server) = self
            .repo
            .find_mcp_server_by_id(self.tenant_id, server_id)
            .await?
        else {
            return Err(AppError::NotFound);
        };
        let now = Utc::now().naive_utc();
        let plan = build_mcp_discovery_save_plan(self.tenant_id, user_id, now, &server, command)?;
        for tool in &plan.tools {
            self.repo.upsert_tool(tool).await?;
        }
        self.repo.save_discovered_mcp_tools(&plan.mcp_tools).await?;
        self.repo
            .update_mcp_server_discovered_tools(
                self.tenant_id,
                server_id,
                &plan.discovered_tools,
                user_id,
                now,
            )
            .await?;

        self.list_mcp_tools(server_id).await
    }

    pub async fn list_mcp_tools(&self, server_id: i64) -> Result<Vec<McpToolResp>, AppError> {
        Ok(self
            .repo
            .list_mcp_tools_by_server(self.tenant_id, server_id)
            .await?
            .into_iter()
            .map(McpToolResp::from)
            .collect())
    }

    pub async fn dry_run_tool(
        &self,
        user_id: i64,
        command: ToolDryRunCommand,
    ) -> Result<ToolDryRunResp, AppError> {
        let command = normalize_tool_dry_run_command(command)?;
        let Some(tool) = self
            .repo
            .find_tool_by_code(self.tenant_id, &command.tool_code)
            .await?
        else {
            return Err(AppError::NotFound);
        };
        let audit_id = next_id();
        let response_payload = json!({
            "dryRun": true,
            "toolCode": tool.code,
            "status": "succeeded",
            "inputEcho": command.input,
            "message": "dry-run only; no external side effects"
        });
        let now = Utc::now().naive_utc();
        let record = ToolAuditSaveRecord {
            id: audit_id,
            tenant_id: self.tenant_id,
            tool_id: tool.id,
            tool_code: tool.code.clone(),
            caller_kind: "admin".to_owned(),
            caller_id: Some(user_id),
            request_payload: json!({
                "toolCode": tool.code,
                "input": response_payload["inputEcho"].clone()
            }),
            response_payload: response_payload.clone(),
            status: "succeeded".to_owned(),
            dry_run: true,
            risk_level: tool.risk_level,
            permission_code: tool.permission_code,
            error_message: None,
            user_id,
            now,
        };
        self.repo.create_tool_call_audit(&record).await?;

        Ok(tool_dry_run_response(
            audit_id,
            &tool.code,
            response_payload,
        ))
    }

    pub async fn list_tool_audits(
        &self,
        query: ToolCallAuditQuery,
    ) -> Result<PageResult<ToolCallAuditResp>, AppError> {
        let page = query.page_query();
        let filter = ToolAuditFilter {
            tenant_id: self.tenant_id,
            tool_code: query.tool_code.as_deref(),
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_tool_call_audits(&filter).await?;
        let list = self
            .repo
            .list_tool_call_audits(&filter)
            .await?
            .into_iter()
            .map(ToolCallAuditResp::from)
            .collect();

        Ok(PageResult::new(list, total))
    }

    pub async fn list_connector_credentials(
        &self,
        query: ConnectorCredentialQuery,
    ) -> Result<PageResult<ConnectorCredentialResp>, AppError> {
        let page = query.page_query();
        let connector_code = query
            .connector_code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let filter = ConnectorCredentialFilter {
            tenant_id: self.tenant_id,
            connector_code,
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_connector_credentials(&filter).await?;
        let list = self
            .repo
            .list_connector_credentials(&filter)
            .await?
            .into_iter()
            .map(ConnectorCredentialResp::from)
            .collect();

        Ok(PageResult::new(list, total))
    }

    pub async fn upsert_connector_credential(
        &self,
        user_id: i64,
        command: ConnectorCredentialCommand,
    ) -> Result<ConnectorCredentialResp, AppError> {
        let command = normalize_connector_credential_command(command)?;
        let record = ConnectorCredentialSaveRecord {
            id: next_id(),
            tenant_id: self.tenant_id,
            connector_code: command.connector_code,
            scope_type: command.scope_type,
            scope_id: command.scope_id,
            auth_type: command.auth_type,
            secret_ref: command.secret_ref,
            scopes: command.scopes,
            status: command.status,
            user_id,
            now: Utc::now().naive_utc(),
        };
        let Some(record) = self.repo.upsert_connector_credential(&record).await? else {
            return Err(AppError::NotFound);
        };

        Ok(ConnectorCredentialResp::from(record))
    }

    async fn list(
        &self,
        resource: CapabilityResource,
        query: CapabilityQuery,
    ) -> Result<PageResult<CapabilityItemResp>, AppError> {
        let page = query.page_query();
        let filter = CapabilityFilter {
            tenant_id: self.tenant_id,
            status: query.status,
            kind: query.kind.as_deref(),
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count(resource, &filter).await?;
        let list = self
            .repo
            .list(resource, &filter)
            .await?
            .into_iter()
            .map(CapabilityItemResp::from)
            .collect();

        Ok(PageResult::new(list, total))
    }
}

pub fn parse_skill_import(
    filename: Option<&str>,
    bytes: &[u8],
) -> Result<SkillImportCommand, AppError> {
    if bytes.is_empty() {
        return Err(AppError::bad_request("Skill 导入文件不能为空"));
    }
    if bytes.len() > MAX_SKILL_IMPORT_BYTES {
        return Err(AppError::bad_request("Skill 导入文件不能超过 512KB"));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| AppError::bad_request("Skill 导入文件必须使用 UTF-8 编码"))?;
    let source_filename = filename
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("SKILL.md");
    let lower_filename = source_filename.to_ascii_lowercase();

    if lower_filename.ends_with(".json") || text.trim_start().starts_with('{') {
        return parse_json_skill_import(Some(source_filename), text);
    }
    parse_markdown_skill_import(Some(source_filename), text)
}

pub fn parse_skill_directory_import(
    source_url: Option<&str>,
    files: Vec<SkillImportFile>,
) -> Result<SkillDirectoryImportCommand, AppError> {
    if files.is_empty() {
        return Err(AppError::bad_request("Skill 目录不能为空"));
    }
    if files.len() > MAX_SKILL_PACKAGE_FILES {
        return Err(AppError::bad_request(format!(
            "Skill 目录文件数不能超过 {}",
            MAX_SKILL_PACKAGE_FILES
        )));
    }

    let mut normalized_files = Vec::with_capacity(files.len());
    let mut total_bytes = 0usize;
    for file in files {
        let relative_path = normalize_skill_package_path(&file.relative_path)?;
        if file.bytes.is_empty() {
            continue;
        }
        total_bytes = total_bytes.saturating_add(file.bytes.len());
        if total_bytes > MAX_SKILL_PACKAGE_BYTES {
            return Err(AppError::bad_request("Skill 目录总大小不能超过 8MB"));
        }
        normalized_files.push(SkillImportFile {
            relative_path,
            bytes: file.bytes,
        });
    }

    let skill_md_index = selected_skill_md_index(&normalized_files)?;
    let skill_md_path = normalized_files[skill_md_index].relative_path.clone();
    let skill_root = skill_root_from_skill_md_path(&skill_md_path);
    let skill_md_text = std::str::from_utf8(&normalized_files[skill_md_index].bytes)
        .map_err(|_| AppError::bad_request("SKILL.md 必须使用 UTF-8 编码"))?;
    let mut skill = parse_markdown_skill_import(Some("SKILL.md"), skill_md_text)?;
    if let Value::Object(metadata) = &mut skill.metadata {
        metadata.insert(
            "sourceType".to_owned(),
            Value::String(source_url.map_or("package".to_owned(), source_type_label)),
        );
        if let Some(source_url) = source_url {
            metadata.insert("sourceUrl".to_owned(), Value::String(source_url.to_owned()));
        }
        metadata.insert("skillRoot".to_owned(), Value::String(skill_root.clone()));
    }

    let mut warnings = Vec::new();
    let mut resources = Vec::new();
    for file in normalized_files {
        let Some(relative_path) = strip_skill_root(&skill_root, &file.relative_path) else {
            continue;
        };
        let resource_kind = skill_resource_kind(&relative_path);
        if resource_kind == SkillResourceKind::Ignored {
            continue;
        }
        let resource = skill_import_resource(resource_kind, &relative_path, &file.bytes)?;
        if resource.resource_type == "script" {
            warnings.push(format!(
                "{} 已保存为脚本资源，但 Novex 不会执行导入脚本",
                resource.relative_path
            ));
        }
        resources.push(resource);
    }

    if !resources
        .iter()
        .any(|resource| resource.relative_path == "SKILL.md")
    {
        return Err(AppError::bad_request("Skill 目录缺少 SKILL.md"));
    }

    Ok(SkillDirectoryImportCommand {
        skill,
        resources,
        warnings,
    })
}

pub fn parse_github_skill_source(value: &str) -> Result<GitHubSkillSource, AppError> {
    let source_url = extract_skill_source_url(value)?;
    let url =
        Url::parse(&source_url).map_err(|_| AppError::bad_request("Skill GitHub 地址格式无效"))?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let segments = url
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>())
        .unwrap_or_default();

    if host == "raw.githubusercontent.com" {
        if segments.len() < 4 {
            return Err(AppError::bad_request("GitHub raw 地址缺少 SKILL.md 路径"));
        }
        let owner = segments[0].to_owned();
        let repo = segments[1].to_owned();
        let reference = segments[2].to_owned();
        let file_path = segments[3..].join("/");
        let path = file_path
            .strip_suffix("/SKILL.md")
            .or_else(|| file_path.strip_suffix("SKILL.md"))
            .unwrap_or(&file_path)
            .trim_matches('/')
            .to_owned();
        return Ok(GitHubSkillSource {
            owner,
            repo,
            reference,
            path,
        });
    }

    if host != "github.com" || segments.len() < 2 {
        return Err(AppError::bad_request(
            "目前仅支持 GitHub 仓库、tree、blob 或 raw SKILL.md 地址",
        ));
    }
    let owner = segments[0].to_owned();
    let repo = segments[1].trim_end_matches(".git").to_owned();
    let mut reference = "main".to_owned();
    let mut path = String::new();

    if segments.len() >= 4 && matches!(segments[2], "tree" | "blob") {
        reference = segments[3].to_owned();
        path = if segments.len() > 4 {
            segments[4..].join("/")
        } else {
            String::new()
        };
        if segments[2] == "blob" {
            path = path
                .strip_suffix("/SKILL.md")
                .or_else(|| path.strip_suffix("SKILL.md"))
                .unwrap_or(&path)
                .trim_matches('/')
                .to_owned();
        }
    }

    Ok(GitHubSkillSource {
        owner,
        repo,
        reference,
        path: normalize_skill_package_path_or_empty(&path)?,
    })
}

fn parse_skill_zip_package(
    filename: Option<&str>,
    bytes: &[u8],
) -> Result<Vec<SkillImportFile>, AppError> {
    if bytes.is_empty() {
        return Err(AppError::bad_request("Skill 压缩包不能为空"));
    }
    if bytes.len() > MAX_SKILL_PACKAGE_BYTES {
        return Err(AppError::bad_request("Skill 压缩包不能超过 8MB"));
    }
    let filename = filename.unwrap_or_default().to_ascii_lowercase();
    if !filename.is_empty() && !filename.ends_with(".zip") {
        return Err(AppError::bad_request("Skill 压缩包必须是 .zip 文件"));
    }

    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|err| AppError::bad_request(format!("Skill 压缩包解析失败: {err}")))?;
    if archive.len() > MAX_SKILL_PACKAGE_FILES {
        return Err(AppError::bad_request(format!(
            "Skill 压缩包文件数不能超过 {}",
            MAX_SKILL_PACKAGE_FILES
        )));
    }

    let mut files = Vec::new();
    let mut total_bytes = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| AppError::bad_request(format!("Skill 压缩包读取失败: {err}")))?;
        if entry.is_dir() {
            continue;
        }
        let relative_path = normalize_skill_package_path(entry.name())?;
        let declared_size = usize::try_from(entry.size()).unwrap_or(usize::MAX);
        if declared_size > MAX_SKILL_PACKAGE_TEXT_BYTES {
            return Err(AppError::bad_request(format!(
                "{relative_path} 不能超过 2MB"
            )));
        }
        let mut content = Vec::new();
        entry
            .read_to_end(&mut content)
            .map_err(|err| AppError::bad_request(format!("Skill 压缩包读取失败: {err}")))?;
        total_bytes = total_bytes.saturating_add(content.len());
        if total_bytes > MAX_SKILL_PACKAGE_BYTES {
            return Err(AppError::bad_request("Skill 压缩包解压后不能超过 8MB"));
        }
        files.push(SkillImportFile {
            relative_path,
            bytes: content,
        });
    }
    Ok(files)
}

fn parse_markdown_skill_import(
    filename: Option<&str>,
    text: &str,
) -> Result<SkillImportCommand, AppError> {
    let normalized = text.strip_prefix('\u{feff}').unwrap_or(text);
    let normalized = normalized.replace("\r\n", "\n");
    let (frontmatter_src, body) = split_skill_md_frontmatter(&normalized)?;
    let (frontmatter, frontmatter_value) = parse_codex_skill_frontmatter(frontmatter_src)?;
    let instruction = body.trim();
    if instruction.is_empty() {
        return Err(AppError::bad_request("SKILL.md instructions 不能为空"));
    }

    let short_description = string_field(
        &frontmatter.metadata,
        &["short-description", "shortDescription"],
    );
    let mut codex = Map::new();
    codex.insert("frontmatter".to_owned(), frontmatter_value);
    if let Some(short_description) = short_description {
        codex.insert(
            "shortDescription".to_owned(),
            Value::String(short_description),
        );
    }
    let metadata = json!({
        "format": "codex.skill.v1",
        "sourceFilename": skill_import_source_filename(filename),
        "skillMd": normalized,
        "instruction": instruction,
        "codex": Value::Object(codex)
    });

    normalize_skill_import_command(SkillImportCommand {
        code: frontmatter.name,
        name: String::new(),
        description: frontmatter.description,
        model_route_policy: default_skill_model_route_policy(),
        capability_refs: default_skill_capability_refs(),
        metadata,
    })
}

fn parse_json_skill_import(
    filename: Option<&str>,
    text: &str,
) -> Result<SkillImportCommand, AppError> {
    let manifest: JsonSkillImport = serde_json::from_str(text)
        .map_err(|err| AppError::bad_request(format!("Skill JSON manifest 解析失败: {err}")))?;
    if let Some(skill_md) = manifest.skill_md.as_deref() {
        return parse_markdown_skill_import(filename, skill_md);
    }

    let mut metadata = match manifest.metadata {
        Value::Object(map) => map,
        Value::Null => Map::new(),
        _ => return Err(AppError::bad_request("Skill metadata 必须是对象")),
    };
    metadata.insert(
        "format".to_owned(),
        Value::String("novex.skill.manifest.v1".to_owned()),
    );
    metadata.insert(
        "sourceFilename".to_owned(),
        Value::String(skill_import_source_filename(filename)),
    );
    if !manifest.instruction.trim().is_empty() && !metadata.contains_key("instruction") {
        metadata.insert(
            "instruction".to_owned(),
            Value::String(manifest.instruction.trim().to_owned()),
        );
    }

    normalize_skill_import_command(SkillImportCommand {
        code: manifest.code,
        name: manifest.name,
        description: manifest.description,
        model_route_policy: manifest.model_route_policy,
        capability_refs: manifest.capability_refs,
        metadata: Value::Object(metadata),
    })
}

fn skill_import_resource(
    resource_kind: SkillResourceKind,
    relative_path: &str,
    bytes: &[u8],
) -> Result<SkillImportResourceCommand, AppError> {
    if bytes.len() > MAX_SKILL_PACKAGE_TEXT_BYTES {
        return Err(AppError::bad_request(format!(
            "{relative_path} 不能超过 2MB"
        )));
    }
    let content_sha256 = sha256_hex(bytes);
    let mime_type = mime_guess::from_path(relative_path)
        .first_or_octet_stream()
        .essence_str()
        .to_owned();
    let content_text = match std::str::from_utf8(bytes) {
        Ok(text) if resource_kind.is_text_resource(&mime_type) => Some(text.to_owned()),
        Ok(_) if resource_kind == SkillResourceKind::Asset => None,
        Ok(text) => Some(text.to_owned()),
        Err(_) if resource_kind == SkillResourceKind::Asset => None,
        Err(_) => {
            return Err(AppError::bad_request(format!(
                "{relative_path} 必须使用 UTF-8 编码"
            )))
        }
    };
    let metadata = match resource_kind {
        SkillResourceKind::Script => json!({ "execution": "disabled" }),
        SkillResourceKind::Asset => json!({ "embedded": false }),
        _ => json!({}),
    };
    Ok(SkillImportResourceCommand {
        resource_type: resource_kind.as_str().to_owned(),
        relative_path: relative_path.to_owned(),
        mime_type,
        content_text,
        storage_ref: None,
        content_sha256,
        size_bytes: bytes.len() as i64,
        metadata,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn normalize_skill_package_path(path: &str) -> Result<String, AppError> {
    normalize_skill_package_path_core(path).map_err(skill_package_error)
}

fn normalize_skill_package_path_or_empty(path: &str) -> Result<String, AppError> {
    normalize_skill_package_path_or_empty_core(path).map_err(skill_package_error)
}

fn selected_skill_md_index(files: &[SkillImportFile]) -> Result<usize, AppError> {
    selected_skill_md_index_core(files).map_err(skill_package_error)
}

fn skill_package_error(error: SkillPackageError) -> AppError {
    match error {
        SkillPackageError::EmptyPath => AppError::bad_request("Skill 文件路径不能为空"),
        SkillPackageError::InvalidPath => AppError::bad_request("Skill 文件路径无效"),
        SkillPackageError::PathTraversal => AppError::bad_request("Skill 文件路径不能包含 .."),
        SkillPackageError::MissingSkillManifest => AppError::bad_request("Skill 目录缺少 SKILL.md"),
        SkillPackageError::MultipleSkillManifests => AppError::bad_request(
            "压缩包包含多个 Skill，请上传单个 Skill 目录或使用 GitHub 预览选择",
        ),
    }
}

fn source_type_label(source_url: &str) -> String {
    if source_url.contains("github.com") || source_url.contains("raw.githubusercontent.com") {
        "github".to_owned()
    } else if source_url.ends_with(".zip") {
        "zip".to_owned()
    } else {
        "package".to_owned()
    }
}

fn extract_skill_source_url(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::bad_request("Skill 导入来源不能为空"));
    }
    let start = value
        .find("https://")
        .or_else(|| value.find("http://"))
        .ok_or_else(|| AppError::bad_request("请粘贴 GitHub Skill 地址或包含地址的需求描述"))?;
    let candidate = &value[start..];
    let end = candidate
        .char_indices()
        .find_map(|(index, ch)| {
            if ch.is_whitespace()
                || matches!(
                    ch,
                    ')' | ']' | '}' | '<' | '>' | '"' | '\'' | '`' | '，' | '。' | ','
                )
            {
                Some(index)
            } else {
                None
            }
        })
        .unwrap_or(candidate.len());
    candidate
        .get(..end)
        .map(str::trim)
        .map(|part| part.trim_end_matches('.').to_owned())
        .filter(|part| !part.is_empty())
        .ok_or_else(|| AppError::bad_request("请粘贴 GitHub Skill 地址或包含地址的需求描述"))
}

fn skill_import_result(
    skill: CapabilityRecord,
    resources: &[SkillImportResourceCommand],
    warnings: Vec<String>,
) -> SkillImportResultResp {
    SkillImportResultResp {
        skill: CapabilityItemResp::from(skill),
        resource_count: resources.len(),
        reference_count: resources
            .iter()
            .filter(|resource| resource.resource_type == "reference")
            .count(),
        script_count: resources
            .iter()
            .filter(|resource| resource.resource_type == "script")
            .count(),
        asset_count: resources
            .iter()
            .filter(|resource| resource.resource_type == "asset")
            .count(),
        warnings,
    }
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubTreeResp {
    tree: Vec<GitHubTreeItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubTreeItem {
    path: String,
    #[serde(default)]
    sha: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubBlobResp {
    content: String,
    encoding: String,
    #[serde(default)]
    size: Option<u64>,
}

async fn fetch_github_skill_tree(
    source: &GitHubSkillSource,
) -> Result<Vec<GitHubTreeItem>, AppError> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/git/trees/{}?recursive=1",
        source.owner, source.repo, source.reference
    );
    let response = github_client()
        .get(&url)
        .send()
        .await
        .map_err(|err| AppError::bad_request(format!("GitHub Skill 目录拉取失败: {err}")))?;
    if !response.status().is_success() {
        return Err(AppError::bad_request(format!(
            "GitHub Skill 目录拉取失败: HTTP {}",
            response.status()
        )));
    }
    let tree = response
        .json::<GitHubTreeResp>()
        .await
        .map_err(|err| AppError::bad_request(format!("GitHub Skill 目录解析失败: {err}")))?;
    Ok(tree.tree)
}

async fn fetch_github_raw_file(
    source: &GitHubSkillSource,
    path: &str,
) -> Result<Vec<u8>, AppError> {
    let path = normalize_skill_package_path(path)?;
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/{}/{}",
        source.owner, source.repo, source.reference, path
    );
    let response = github_client()
        .get(&url)
        .send()
        .await
        .map_err(|err| AppError::bad_request(format!("GitHub Skill 文件拉取失败: {err}")))?;
    if !response.status().is_success() {
        return Err(AppError::bad_request(format!(
            "GitHub Skill 文件拉取失败: HTTP {}",
            response.status()
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| AppError::bad_request(format!("GitHub Skill 文件读取失败: {err}")))?;
    if bytes.len() > MAX_SKILL_PACKAGE_TEXT_BYTES {
        return Err(AppError::bad_request(format!("{path} 不能超过 2MB")));
    }
    Ok(bytes.to_vec())
}

async fn fetch_github_tree_item_file(
    source: &GitHubSkillSource,
    item: &GitHubTreeItem,
) -> Result<Vec<u8>, AppError> {
    if let Some(sha) = item.sha.as_deref() {
        if let Ok(bytes) = fetch_github_blob_file(source, sha, &item.path).await {
            return Ok(bytes);
        }
    }
    fetch_github_raw_file(source, &item.path).await
}

async fn fetch_github_blob_file(
    source: &GitHubSkillSource,
    sha: &str,
    path: &str,
) -> Result<Vec<u8>, AppError> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/git/blobs/{}",
        source.owner, source.repo, sha
    );
    let response = github_client()
        .get(&url)
        .send()
        .await
        .map_err(|err| AppError::bad_request(format!("GitHub Skill 文件拉取失败: {err}")))?;
    if !response.status().is_success() {
        return Err(AppError::bad_request(format!(
            "GitHub Skill 文件拉取失败: HTTP {}",
            response.status()
        )));
    }
    let blob = response
        .json::<GitHubBlobResp>()
        .await
        .map_err(|err| AppError::bad_request(format!("GitHub Skill 文件解析失败: {err}")))?;
    if blob.encoding != "base64" {
        return Err(AppError::bad_request(format!(
            "{path} 使用了不支持的 GitHub blob 编码"
        )));
    }
    if blob.size.unwrap_or_default() as usize > MAX_SKILL_PACKAGE_TEXT_BYTES {
        return Err(AppError::bad_request(format!("{path} 不能超过 2MB")));
    }
    let content = blob
        .content
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let bytes = general_purpose::STANDARD
        .decode(content)
        .map_err(|err| AppError::bad_request(format!("GitHub Skill 文件解码失败: {err}")))?;
    if bytes.len() > MAX_SKILL_PACKAGE_TEXT_BYTES {
        return Err(AppError::bad_request(format!("{path} 不能超过 2MB")));
    }
    Ok(bytes)
}

async fn fetch_github_skill_files(
    source: &GitHubSkillSource,
    tree: &[GitHubTreeItem],
    skill_path: &str,
) -> Result<Vec<SkillImportFile>, AppError> {
    let skill_path = normalize_skill_package_path_or_empty(skill_path)?;
    let mut files = Vec::new();
    let mut total_bytes = 0usize;
    for item in tree {
        if item.kind != "blob" || !path_belongs_to_skill_dir(&item.path, &skill_path) {
            continue;
        }
        let Some(relative_path) = strip_skill_root(&skill_path, &item.path) else {
            continue;
        };
        let resource_kind = skill_resource_kind(&relative_path);
        if resource_kind == SkillResourceKind::Ignored {
            continue;
        }
        if let Some(size) = item.size {
            if size as usize > MAX_SKILL_PACKAGE_TEXT_BYTES {
                return Err(AppError::bad_request(format!("{} 不能超过 2MB", item.path)));
            }
        }
        let bytes = fetch_github_tree_item_file(source, item).await?;
        total_bytes = total_bytes.saturating_add(bytes.len());
        if total_bytes > MAX_SKILL_PACKAGE_BYTES {
            return Err(AppError::bad_request("Skill 目录总大小不能超过 8MB"));
        }
        files.push(SkillImportFile {
            relative_path,
            bytes,
        });
        if files.len() > MAX_SKILL_PACKAGE_FILES {
            return Err(AppError::bad_request(format!(
                "Skill 目录文件数不能超过 {}",
                MAX_SKILL_PACKAGE_FILES
            )));
        }
    }
    Ok(files)
}

fn github_client() -> reqwest::Client {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Novex-Skill-Importer"));
    if let Ok(token) = env::var("GITHUB_TOKEN").or_else(|_| env::var("GH_TOKEN")) {
        if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", token.trim())) {
            headers.insert(AUTHORIZATION, value);
        }
    }
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn discover_skill_paths(source: &GitHubSkillSource, tree: &[GitHubTreeItem]) -> Vec<String> {
    let prefix = source.path.trim_matches('/');
    let mut paths = tree
        .iter()
        .filter(|item| item.kind == "blob")
        .filter(|item| item.path.ends_with("SKILL.md"))
        .filter(|item| prefix.is_empty() || path_belongs_to_skill_dir(&item.path, prefix))
        .map(|item| skill_root_from_skill_md_path(&item.path))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    if !prefix.is_empty() && tree_contains_skill_dir(tree, prefix) {
        paths.retain(|path| path == prefix);
    }
    paths
}

fn tree_contains_skill_dir(tree: &[GitHubTreeItem], skill_path: &str) -> bool {
    let Some(skill_md_path) = skill_md_path_for_dir(skill_path) else {
        return false;
    };
    tree.iter()
        .any(|item| item.kind == "blob" && item.path == skill_md_path)
}

fn tree_item_for_path<'a>(tree: &'a [GitHubTreeItem], path: &str) -> Option<&'a GitHubTreeItem> {
    tree.iter()
        .find(|item| item.kind == "blob" && item.path == path)
}

fn skill_resource_counts(tree: &[GitHubTreeItem], skill_path: &str) -> (usize, usize, usize) {
    let mut reference_count = 0;
    let mut script_count = 0;
    let mut asset_count = 0;
    for item in tree {
        if item.kind != "blob" || !path_belongs_to_skill_dir(&item.path, skill_path) {
            continue;
        }
        let Some(relative_path) = strip_skill_root(skill_path, &item.path) else {
            continue;
        };
        match skill_resource_kind(&relative_path) {
            SkillResourceKind::Reference => reference_count += 1,
            SkillResourceKind::Script => script_count += 1,
            SkillResourceKind::Asset => asset_count += 1,
            _ => {}
        }
    }
    (reference_count, script_count, asset_count)
}

fn skill_md_path_for_dir(skill_path: &str) -> Option<String> {
    let skill_path = skill_path.trim_matches('/');
    if skill_path.is_empty() {
        Some("SKILL.md".to_owned())
    } else {
        Some(format!("{skill_path}/SKILL.md"))
    }
}

fn path_belongs_to_skill_dir(path: &str, skill_path: &str) -> bool {
    let skill_path = skill_path.trim_matches('/');
    if skill_path.is_empty() {
        return true;
    }
    path == skill_path || path.starts_with(&format!("{skill_path}/"))
}

fn normalize_skill_import_command(
    mut command: SkillImportCommand,
) -> Result<SkillImportCommand, AppError> {
    command.code = command.code.trim().to_owned();
    command.name = command.name.trim().to_owned();
    command.description = command.description.trim().to_owned();
    if command.name.is_empty() {
        command.name = command.code.clone();
    }
    if command.model_route_policy.is_null() {
        command.model_route_policy = default_skill_model_route_policy();
    }
    if command.capability_refs.is_null() {
        command.capability_refs = default_skill_capability_refs();
    }

    if command.code.is_empty() {
        return Err(AppError::bad_request("Skill name 不能为空"));
    }
    validate_skill_code(&command.code)?;
    if command.description.is_empty() {
        return Err(AppError::bad_request("Skill description 不能为空"));
    }
    if !command.model_route_policy.is_object() {
        return Err(AppError::bad_request("Skill modelRoutePolicy 必须是对象"));
    }
    if !command.capability_refs.is_array() {
        return Err(AppError::bad_request("Skill capabilityRefs 必须是数组"));
    }
    if !command.metadata.is_object() {
        return Err(AppError::bad_request("Skill metadata 必须是对象"));
    }

    ensure_max_chars("Skill name", &command.code, 128)?;
    ensure_max_chars("Skill display name", &command.name, 128)?;
    ensure_max_chars("Skill description", &command.description, 1024)?;
    Ok(command)
}

fn split_skill_md_frontmatter(text: &str) -> Result<(&str, &str), AppError> {
    let Some(rest) = text.strip_prefix("---\n") else {
        return Err(AppError::bad_request(
            "SKILL.md frontmatter 必须以 --- 开头",
        ));
    };
    let Some((frontmatter, body)) = rest.split_once("\n---\n") else {
        return Err(AppError::bad_request(
            "SKILL.md frontmatter 必须以 --- 结束",
        ));
    };
    Ok((frontmatter, body))
}

fn parse_codex_skill_frontmatter(source: &str) -> Result<(CodexSkillFrontmatter, Value), AppError> {
    let mut root = Map::new();
    let mut metadata = Map::new();
    let mut section: Option<String> = None;
    let lines = source.lines().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < lines.len() {
        let raw_line = lines[index];
        if raw_line.trim().is_empty() || raw_line.trim_start().starts_with('#') {
            index += 1;
            continue;
        }
        let indent = raw_line
            .chars()
            .take_while(|ch| ch.is_ascii_whitespace())
            .count();
        let line = raw_line.trim();

        if indent == 0 {
            let Some((key, value)) = line.split_once(':') else {
                return Err(AppError::bad_request("SKILL.md frontmatter 格式无效"));
            };
            let key = key.trim();
            let value = value.trim();
            if value.is_empty() {
                section = Some(key.to_owned());
                root.insert(key.to_owned(), Value::Object(Map::new()));
                index += 1;
            } else if let Some(folded) = yaml_block_scalar_style(value) {
                let (block, next_index) =
                    collect_yaml_block_scalar(&lines, index + 1, indent, folded);
                section = None;
                root.insert(key.to_owned(), Value::String(block));
                index = next_index;
            } else {
                section = None;
                root.insert(key.to_owned(), yaml_scalar(value));
                index += 1;
            }
            continue;
        }

        if section.as_deref() == Some("metadata") {
            let Some((key, value)) = line.split_once(':') else {
                return Err(AppError::bad_request("SKILL.md metadata 格式无效"));
            };
            let key = key.trim();
            let value = value.trim();
            if let Some(folded) = yaml_block_scalar_style(value) {
                let (block, next_index) =
                    collect_yaml_block_scalar(&lines, index + 1, indent, folded);
                metadata.insert(key.to_owned(), Value::String(block));
                index = next_index;
            } else {
                metadata.insert(key.to_owned(), yaml_scalar(value));
                index += 1;
            }
            continue;
        }
        index += 1;
    }

    if !metadata.is_empty() {
        root.insert("metadata".to_owned(), Value::Object(metadata.clone()));
    }

    let frontmatter = CodexSkillFrontmatter {
        name: root
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        description: root
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        metadata: Value::Object(metadata),
    };
    Ok((frontmatter, Value::Object(root)))
}

fn yaml_block_scalar_style(value: &str) -> Option<bool> {
    let value = value.trim();
    if value.starts_with('|') {
        Some(false)
    } else if value.starts_with('>') {
        Some(true)
    } else {
        None
    }
}

fn collect_yaml_block_scalar(
    lines: &[&str],
    start_index: usize,
    parent_indent: usize,
    folded: bool,
) -> (String, usize) {
    let mut index = start_index;
    let mut block_lines = Vec::new();
    while index < lines.len() {
        let line = lines[index];
        if !line.trim().is_empty() {
            let indent = line
                .chars()
                .take_while(|ch| ch.is_ascii_whitespace())
                .count();
            if indent <= parent_indent {
                break;
            }
        }
        block_lines.push(line);
        index += 1;
    }

    let min_indent = block_lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.chars()
                .take_while(|ch| ch.is_ascii_whitespace())
                .count()
        })
        .min()
        .unwrap_or(parent_indent + 2);
    let normalized = block_lines
        .into_iter()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                line.chars().skip(min_indent).collect::<String>()
            }
        })
        .collect::<Vec<_>>();

    let value = if folded {
        normalized.join(" ")
    } else {
        normalized.join("\n")
    };
    (value.trim().to_owned(), index)
}

fn yaml_scalar(value: &str) -> Value {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    if let Some(unquoted) = trimmed
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
    {
        return Value::String(unquoted.to_owned());
    }
    Value::String(trimmed.to_owned())
}

fn validate_skill_code(code: &str) -> Result<(), AppError> {
    let mut chars = code.chars();
    let Some(first) = chars.next() else {
        return Err(AppError::bad_request("Skill name 不能为空"));
    };
    if !first.is_ascii_alphanumeric() {
        return Err(AppError::bad_request("Skill name 必须以英文字母或数字开头"));
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':')) {
        return Err(AppError::bad_request(
            "Skill name 只能包含字母、数字、下划线、中划线、点或冒号",
        ));
    }
    Ok(())
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn skill_import_source_filename(filename: Option<&str>) -> String {
    filename
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("SKILL.md")
        .to_owned()
}

fn default_skill_model_route_policy() -> Value {
    json!({
        "answerModel": "runtime.llm.rag_answer",
        "embeddingModel": "runtime.embedding.default",
        "rerankModel": "runtime.rerank.default"
    })
}

fn default_skill_capability_refs() -> Value {
    json!([{ "kind": "tool", "code": "rag.search" }])
}

pub fn normalize_tool_dry_run_command(
    mut command: ToolDryRunCommand,
) -> Result<ToolDryRunCommand, AppError> {
    command.tool_code = command.tool_code.trim().to_owned();
    if command.tool_code.is_empty() {
        return Err(AppError::bad_request("工具编码不能为空"));
    }
    ensure_max_chars("工具编码", &command.tool_code, 128)?;
    Ok(command)
}

pub fn normalize_connector_credential_command(
    mut command: ConnectorCredentialCommand,
) -> Result<ConnectorCredentialCommand, AppError> {
    command.connector_code = command.connector_code.trim().to_owned();
    command.scope_type = command.scope_type.trim().to_ascii_lowercase();
    command.scope_id = command.scope_id.trim().to_owned();
    command.auth_type = command.auth_type.trim().to_ascii_lowercase();
    command.secret_ref = command.secret_ref.trim().to_owned();

    if command.connector_code.is_empty() {
        return Err(AppError::bad_request("连接器编码不能为空"));
    }
    if !matches!(command.scope_type.as_str(), "tenant" | "user" | "app") {
        return Err(AppError::bad_request("连接器凭据作用域无效"));
    }
    if command.scope_id.is_empty() {
        return Err(AppError::bad_request("连接器凭据作用域ID不能为空"));
    }
    if command.auth_type.is_empty() {
        return Err(AppError::bad_request("连接器凭据认证类型不能为空"));
    }
    if !command.secret_ref.starts_with("env:") {
        return Err(AppError::bad_request(
            "连接器凭据 secretRef 必须使用 env: 前缀",
        ));
    }
    if !(0..=1).contains(&command.status) {
        return Err(AppError::bad_request("连接器凭据状态无效"));
    }

    ensure_max_chars("连接器编码", &command.connector_code, 128)?;
    ensure_max_chars("连接器凭据作用域", &command.scope_type, 32)?;
    ensure_max_chars("连接器凭据作用域ID", &command.scope_id, 128)?;
    ensure_max_chars("连接器凭据认证类型", &command.auth_type, 64)?;
    ensure_max_chars("连接器凭据 secretRef", &command.secret_ref, 255)?;
    Ok(command)
}

pub fn normalize_plugin_install_command(
    mut command: PluginInstallCommand,
) -> Result<PluginInstallCommand, AppError> {
    command.plugin_code = command.plugin_code.trim().to_owned();
    command.version = command.version.trim().to_owned();
    if command.permission_grants.is_null() {
        command.permission_grants = Value::Array(vec![]);
    }
    if command.config.is_null() {
        command.config = Value::Object(Default::default());
    }

    if command.plugin_code.is_empty() {
        return Err(AppError::bad_request("插件编码不能为空"));
    }
    if command.version.is_empty() {
        return Err(AppError::bad_request("插件版本不能为空"));
    }
    if !command.permission_grants.is_array() {
        return Err(AppError::bad_request("插件授权必须是数组"));
    }
    if !command.config.is_object() {
        return Err(AppError::bad_request("插件配置必须是对象"));
    }

    ensure_max_chars("插件编码", &command.plugin_code, 128)?;
    ensure_max_chars("插件版本", &command.version, 64)?;
    Ok(command)
}

pub fn normalize_mcp_server_command(
    mut command: McpServerCommand,
) -> Result<McpServerCommand, AppError> {
    command.code = command.code.trim().to_owned();
    command.name = command.name.trim().to_owned();
    command.transport_kind = command
        .transport_kind
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_");
    command.auth_scope = command.auth_scope.trim().to_ascii_lowercase();
    command.auth_type = command
        .auth_type
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_");
    command.endpoint_url = trim_optional(command.endpoint_url);
    command.secret_ref = trim_optional(command.secret_ref);
    if command.network_allowlist.is_null() {
        command.network_allowlist = Value::Array(vec![]);
    }
    if command.tool_allowlist.is_null() {
        command.tool_allowlist = Value::Array(vec![]);
    }
    if command.discovered_tools.is_null() {
        command.discovered_tools = Value::Array(vec![]);
    }

    if command.code.is_empty() {
        return Err(AppError::bad_request("MCP Server 编码不能为空"));
    }
    if command.name.is_empty() {
        return Err(AppError::bad_request("MCP Server 名称不能为空"));
    }
    if command.transport_kind.is_empty() {
        command.transport_kind = "streamable_http".to_owned();
    }
    if command.auth_scope.is_empty() {
        command.auth_scope = "tenant".to_owned();
    }
    if command.auth_type.is_empty() {
        command.auth_type = "none".to_owned();
    }
    if !matches!(
        command.transport_kind.as_str(),
        "builtin" | "stdio" | "sse" | "streamable_http"
    ) {
        return Err(AppError::bad_request("MCP Server transport 无效"));
    }
    if !matches!(command.auth_scope.as_str(), "tenant" | "user" | "app") {
        return Err(AppError::bad_request("MCP Server 授权作用域无效"));
    }
    if !matches!(
        command.auth_type.as_str(),
        "none" | "bearer_env" | "oauth" | "headers"
    ) {
        return Err(AppError::bad_request("MCP Server 认证类型无效"));
    }
    if !command.network_allowlist.is_array() {
        return Err(AppError::bad_request(
            "MCP Server network allow-list 必须是数组",
        ));
    }
    if !command.tool_allowlist.is_array() {
        return Err(AppError::bad_request(
            "MCP Server tool allow-list 必须是数组",
        ));
    }
    if !command.discovered_tools.is_array() {
        return Err(AppError::bad_request(
            "MCP Server discoveredTools 必须是数组",
        ));
    }
    let network_allowlist = string_array_values(
        "MCP Server network allow-list",
        &command.network_allowlist,
        512,
    )?;
    let tool_allowlist =
        string_array_values("MCP Server tool allow-list", &command.tool_allowlist, 128)?;
    validate_mcp_registration_policy(&McpRegistrationPolicy {
        server_code: command.code.clone(),
        endpoint_url: command.endpoint_url.clone(),
        transport_kind: mcp_transport_kind(&command.transport_kind),
        auth_scope: mcp_auth_scope(&command.auth_scope),
        auth_type: mcp_auth_type(&command.auth_type),
        secret_ref: command.secret_ref.clone(),
        network_allowlist,
        tool_allowlist,
    })
    .map_err(|err| AppError::bad_request(err.message))?;
    ensure_max_chars("MCP Server 编码", &command.code, 128)?;
    ensure_max_chars("MCP Server 名称", &command.name, 128)?;
    ensure_max_chars("MCP Server transport", &command.transport_kind, 32)?;
    ensure_max_chars("MCP Server 授权作用域", &command.auth_scope, 64)?;
    ensure_max_chars("MCP Server 认证类型", &command.auth_type, 64)?;
    if let Some(endpoint_url) = command.endpoint_url.as_deref() {
        ensure_max_chars("MCP Server endpointUrl", endpoint_url, 512)?;
    }
    if let Some(secret_ref) = command.secret_ref.as_deref() {
        ensure_max_chars("MCP Server secretRef", secret_ref, 255)?;
    }
    Ok(command)
}

fn build_mcp_discovery_save_plan(
    tenant_id: i64,
    user_id: i64,
    now: chrono::NaiveDateTime,
    server: &McpServerRecord,
    command: McpDiscoveryCommand,
) -> Result<McpDiscoverySavePlan, AppError> {
    if command.tools.is_empty() {
        return Err(AppError::bad_request("MCP discovery tools 不能为空"));
    }
    let allowed_tools =
        string_array_values("MCP Server tool allow-list", &server.tool_allowlist, 128)?
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();

    let mut mcp_tools = Vec::with_capacity(command.tools.len());
    let mut tools = Vec::with_capacity(command.tools.len());
    let mut discovered_tools = Vec::with_capacity(command.tools.len());

    for mut tool in command.tools {
        tool.tool_name = tool.tool_name.trim().to_owned();
        tool.description = tool.description.trim().to_owned();
        if tool.tool_name.is_empty() {
            return Err(AppError::bad_request("MCP Tool 名称不能为空"));
        }
        ensure_max_chars("MCP Tool 名称", &tool.tool_name, 128)?;
        ensure_max_chars("MCP Tool 描述", &tool.description, 512)?;

        let input_schema = normalize_json_object("MCP Tool inputSchema", tool.input_schema)?;
        let output_schema = normalize_json_object("MCP Tool outputSchema", tool.output_schema)?;
        let metadata = normalize_json_object("MCP Tool metadata", tool.metadata)?;
        let risk_level = mcp_tool_risk_level(tool.risk_level)?;
        let tool_code = mcp_tool_code(&server.code, &tool.tool_name);
        let permission_code = mcp_permission_code(&server.code, &tool.tool_name);

        if !mcp_tool_allowed(&server.code, &allowed_tools, &tool.tool_name, &tool_code) {
            return Err(AppError::bad_request(format!(
                "MCP Tool {} 不在 tool allow-list 中",
                tool.tool_name
            )));
        }
        ensure_max_chars("MCP Tool 编码", &tool_code, 128)?;
        ensure_max_chars("MCP Tool permissionCode", &permission_code, 128)?;

        let discovered = McpDiscoveredTool {
            server_code: server.code.clone(),
            tool_name: tool.tool_name.clone(),
            description: tool.description.clone(),
            input_schema: input_schema.clone(),
            output_schema: Some(output_schema.clone()),
            risk_level,
        };
        let definition = discovered.to_tool_definition(permission_code.clone());
        let metadata = mcp_tool_metadata(server, &tool.tool_name, &metadata);
        let status = if tool.enabled { ENABLED_STATUS } else { 0 };

        mcp_tools.push(McpToolSaveRecord {
            id: next_id(),
            tenant_id,
            server_id: server.id,
            tool_name: tool.tool_name.clone(),
            tool_code: definition.code.clone(),
            description: definition.description.clone(),
            input_schema: definition.input_schema.clone(),
            output_schema: definition
                .output_schema
                .clone()
                .unwrap_or_else(default_json_object),
            risk_level: tool.risk_level,
            permission_code: definition.permission_code.clone(),
            status,
            metadata: metadata.clone(),
            user_id,
            now,
        });
        tools.push(ToolSaveRecord {
            id: next_id(),
            tenant_id,
            code: definition.code.clone(),
            name: definition.name.clone(),
            description: definition.description.clone(),
            tool_kind: "mcp".to_owned(),
            risk_level: tool.risk_level,
            approval_policy: 1,
            permission_code: definition.permission_code.clone(),
            executor_kind: "mcp".to_owned(),
            input_schema: definition.input_schema.clone(),
            output_schema: definition.output_schema.unwrap_or_else(default_json_object),
            status,
            metadata: metadata.clone(),
            user_id,
            now,
        });
        discovered_tools.push(json!({
            "serverId": server.id,
            "serverCode": server.code,
            "toolName": tool.tool_name,
            "toolCode": definition.code,
            "riskLevel": tool.risk_level,
            "permissionCode": permission_code,
            "enabled": tool.enabled,
            "metadata": metadata,
        }));
    }

    Ok(McpDiscoverySavePlan {
        mcp_tools,
        tools,
        discovered_tools: Value::Array(discovered_tools),
    })
}

fn normalize_json_object(label: &str, value: Value) -> Result<Value, AppError> {
    if value.is_null() {
        return Ok(default_json_object());
    }
    if !value.is_object() {
        return Err(AppError::bad_request(format!("{label} 必须是对象")));
    }
    Ok(value)
}

fn mcp_tool_risk_level(value: i16) -> Result<ToolRiskLevel, AppError> {
    match value {
        1 => Ok(ToolRiskLevel::Low),
        2 => Ok(ToolRiskLevel::Medium),
        3 => Ok(ToolRiskLevel::High),
        _ => Err(AppError::bad_request("MCP Tool riskLevel 必须是 1、2 或 3")),
    }
}

fn mcp_permission_code(server_code: &str, tool_name: &str) -> String {
    format!(
        "ai:mcp:{}:{}",
        normalize_permission_code_segment(server_code),
        normalize_permission_code_segment(tool_name)
    )
}

fn normalize_permission_code_segment(value: &str) -> String {
    let normalized = value
        .trim()
        .chars()
        .map(|ch| match ch {
            ch if ch.is_ascii_alphanumeric() => ch.to_ascii_lowercase(),
            '.' | '_' => ch,
            '-' | '/' | ':' | ' ' => '_',
            _ => '_',
        })
        .collect::<String>();
    let collapsed = normalized
        .split('_')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if collapsed.is_empty() {
        "unknown".to_owned()
    } else {
        collapsed
    }
}

fn mcp_tool_allowed(
    server_code: &str,
    allowed_tools: &[String],
    tool_name: &str,
    tool_code: &str,
) -> bool {
    let server_scoped_name = format!("{}.{}", server_code.trim(), tool_name.trim());
    allowed_tools.iter().any(|allowed| {
        let allowed = allowed.trim();
        allowed == "*"
            || allowed.eq_ignore_ascii_case(tool_name)
            || allowed.eq_ignore_ascii_case(tool_code)
            || allowed.eq_ignore_ascii_case(&server_scoped_name)
    })
}

fn mcp_tool_metadata(server: &McpServerRecord, tool_name: &str, source: &Value) -> Value {
    let mut metadata = source.as_object().cloned().unwrap_or_else(Map::new);
    metadata.insert("source".to_owned(), json!("mcp_discovery"));
    metadata.insert("serverId".to_owned(), json!(server.id));
    metadata.insert("serverCode".to_owned(), json!(server.code));
    metadata.insert("toolName".to_owned(), json!(tool_name));
    Value::Object(metadata)
}

fn masked_secret_ref(secret_ref: &str) -> String {
    let secret_ref = secret_ref.trim();
    if let Some(env_name) = secret_ref.strip_prefix("env:") {
        let env_name = env_name.trim();
        if env_name.len() <= 4 {
            "env:****".to_owned()
        } else {
            format!("env:{}****", &env_name[..env_name.len().min(4)])
        }
    } else {
        "****".to_owned()
    }
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn string_array_values(
    label: &str,
    value: &Value,
    max_len: usize,
) -> Result<Vec<String>, AppError> {
    let Some(values) = value.as_array() else {
        return Err(AppError::bad_request(format!("{label} 必须是数组")));
    };
    let mut strings = Vec::with_capacity(values.len());
    for item in values {
        let Some(item) = item.as_str() else {
            return Err(AppError::bad_request(format!("{label} 只能包含字符串")));
        };
        ensure_max_chars(label, item, max_len)?;
        strings.push(item.trim().to_owned());
    }
    Ok(strings)
}

fn mcp_transport_kind(value: &str) -> McpTransportKind {
    match value {
        "builtin" => McpTransportKind::Builtin,
        "stdio" => McpTransportKind::Stdio,
        "sse" => McpTransportKind::Sse,
        _ => McpTransportKind::StreamableHttp,
    }
}

fn mcp_auth_scope(value: &str) -> McpAuthScope {
    match value {
        "user" => McpAuthScope::User,
        "app" => McpAuthScope::App,
        _ => McpAuthScope::Tenant,
    }
}

fn mcp_auth_type(value: &str) -> McpAuthType {
    match value {
        "bearer_env" => McpAuthType::BearerEnv,
        "oauth" => McpAuthType::OAuth,
        "headers" => McpAuthType::Headers,
        _ => McpAuthType::None,
    }
}

fn tool_dry_run_response(audit_id: i64, tool_code: &str, response: Value) -> ToolDryRunResp {
    ToolDryRunResp {
        audit_id,
        tool_code: tool_code.to_owned(),
        status: "succeeded".to_owned(),
        dry_run: true,
        response,
    }
}

impl From<CapabilityRecord> for CapabilityItemResp {
    fn from(record: CapabilityRecord) -> Self {
        Self {
            id: record.id,
            code: record.code,
            name: record.name,
            description: record.description,
            kind: record.kind,
            status: record.status,
            risk_level: record.risk_level,
            metadata: record.metadata,
            create_time: format_datetime(record.create_time),
        }
    }
}

impl From<ToolAuditRecord> for ToolCallAuditResp {
    fn from(record: ToolAuditRecord) -> Self {
        Self {
            id: record.id,
            tool_code: record.tool_code,
            status: record.status,
            dry_run: record.dry_run,
            risk_level: record.risk_level,
            permission_code: record.permission_code,
            create_time: format_datetime(record.create_time),
        }
    }
}

impl From<ConnectorCredentialRecord> for ConnectorCredentialResp {
    fn from(record: ConnectorCredentialRecord) -> Self {
        Self {
            id: record.id,
            connector_id: record.connector_id,
            connector_code: record.connector_code,
            scope_type: record.scope_type,
            scope_id: record.scope_id,
            auth_type: record.auth_type,
            masked_value: masked_secret_ref(&record.secret_ref),
            secret_ref: record.secret_ref,
            scopes: record.scopes,
            status: record.status,
            create_time: format_datetime(record.create_time),
            update_time: record.update_time.map(format_datetime),
        }
    }
}

impl From<PluginInstallationRecord> for PluginInstallationResp {
    fn from(record: PluginInstallationRecord) -> Self {
        Self {
            id: record.id,
            plugin_id: record.plugin_id,
            plugin_code: record.plugin_code,
            plugin_name: record.plugin_name,
            version: record.version,
            enabled: record.enabled,
            permission_grants: record.permission_grants,
            capabilities: record.capabilities,
            config: record.config,
            install_source: record.install_source,
            create_time: format_datetime(record.create_time),
            update_time: record.update_time.map(format_datetime),
        }
    }
}

impl From<McpServerRecord> for McpServerResp {
    fn from(record: McpServerRecord) -> Self {
        let masked_secret_ref = record
            .secret_ref
            .as_deref()
            .map(masked_secret_ref)
            .unwrap_or_default();
        Self {
            id: record.id,
            code: record.code,
            name: record.name,
            endpoint_url: record.endpoint_url,
            transport_kind: record.transport_kind,
            auth_scope: record.auth_scope,
            auth_type: record.auth_type,
            secret_ref: record.secret_ref,
            masked_secret_ref,
            network_allowlist: record.network_allowlist,
            tool_allowlist: record.tool_allowlist,
            discovered_tools: record.discovered_tools,
            enabled: record.status == ENABLED_STATUS,
            create_time: format_datetime(record.create_time),
            update_time: record.update_time.map(format_datetime),
        }
    }
}

impl From<McpToolRecord> for McpToolResp {
    fn from(record: McpToolRecord) -> Self {
        Self {
            id: record.id,
            server_id: record.server_id,
            server_code: record.server_code,
            tool_name: record.tool_name,
            tool_code: record.tool_code,
            description: record.description,
            input_schema: record.input_schema,
            output_schema: record.output_schema,
            risk_level: record.risk_level,
            permission_code: record.permission_code,
            status: record.status,
            metadata: record.metadata,
            create_time: format_datetime(record.create_time),
            update_time: record.update_time.map(format_datetime),
        }
    }
}

fn summary_filter<'a>(tenant_id: i64) -> CapabilityFilter<'a> {
    CapabilityFilter {
        tenant_id,
        status: Some(ENABLED_STATUS),
        kind: None,
        limit: DEFAULT_CAPABILITY_PAGE_SIZE as i64,
        offset: 0,
    }
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_capability_size() -> u64 {
    DEFAULT_CAPABILITY_PAGE_SIZE
}

fn default_enabled_status() -> Option<i16> {
    Some(ENABLED_STATUS)
}

fn default_enabled_status_i16() -> i16 {
    ENABLED_STATUS
}

fn default_true() -> bool {
    true
}

fn default_low_risk_level() -> i16 {
    1
}

fn default_json_object() -> Value {
    Value::Object(Default::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use std::io::Write;
    use zip::{write::SimpleFileOptions, ZipWriter};

    #[tokio::test]
    async fn capability_service_can_be_bound_to_request_tenant() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let service = CapabilityService::for_tenant(db, 42);

        assert_eq!(service.tenant_id, 42);
    }

    #[test]
    fn capability_query_defaults_to_enabled_status_and_page_size() {
        let query = CapabilityQuery::default();

        assert_eq!(query.status, Some(1));
        assert_eq!(query.page_query().limit(), 20);
    }

    #[test]
    fn skill_import_parses_codex_skill_md_frontmatter_and_instructions() {
        let source = br#"---
name: knowledge-table-summarizer
description: Summarize cited knowledge tables and preserve row evidence.
metadata:
  short-description: Summarize tables
---
# Table Summarizer

Always cite the source rows.
"#;

        let command = parse_skill_import(Some("SKILL.md"), source).unwrap();

        assert_eq!(command.code, "knowledge-table-summarizer");
        assert_eq!(command.name, "knowledge-table-summarizer");
        assert_eq!(
            command.description,
            "Summarize cited knowledge tables and preserve row evidence."
        );
        assert_eq!(
            command.capability_refs,
            json!([{ "kind": "tool", "code": "rag.search" }])
        );
        assert_eq!(
            command.model_route_policy["answerModel"],
            "runtime.llm.rag_answer"
        );
        assert_eq!(command.metadata["format"], "codex.skill.v1");
        assert_eq!(
            command.metadata["codex"]["shortDescription"],
            "Summarize tables"
        );
        assert!(command.metadata["instruction"]
            .as_str()
            .unwrap()
            .contains("Always cite the source rows."));
    }

    #[test]
    fn skill_import_parses_yaml_block_scalar_description() {
        let source = r#"---
name: khazix-writer
description: |
  数字生命卡兹克的公众号长文写作skill。
  当用户需要撰写公众号文章时使用。
---
# 卡兹克公众号长文写作

按作者风格输出长文。
"#;

        let command = parse_skill_import(Some("SKILL.md"), source.as_bytes()).unwrap();

        assert_eq!(command.code, "khazix-writer");
        assert!(command.description.contains("数字生命卡兹克"));
        assert!(command.description.contains("公众号文章"));
        assert_ne!(command.description, "|");
    }

    #[test]
    fn github_skill_source_parses_tree_skill_directory() {
        let source = parse_github_skill_source(
            "https://github.com/KKKKhazix/khazix-skills/tree/main/khazix-writer",
        )
        .unwrap();

        assert_eq!(source.owner, "KKKKhazix");
        assert_eq!(source.repo, "khazix-skills");
        assert_eq!(source.reference, "main");
        assert_eq!(source.path, "khazix-writer");
    }

    #[test]
    fn github_skill_source_accepts_markdown_link_text() {
        let source = parse_github_skill_source(
            "[https://github.com/KKKKhazix/khazix-skills/tree/main/khazix-writer](https://github.com/KKKKhazix/khazix-skills/tree/main/khazix-writer)",
        )
        .unwrap();

        assert_eq!(source.owner, "KKKKhazix");
        assert_eq!(source.repo, "khazix-skills");
        assert_eq!(source.reference, "main");
        assert_eq!(source.path, "khazix-writer");
    }

    #[test]
    fn skill_directory_import_keeps_references_and_disables_scripts() {
        let source = vec![
            SkillImportFile {
                relative_path: "SKILL.md".to_owned(),
                bytes: br#"---
name: khazix-writer
description: Generate cited long-form content.
---
# Khazix Writer

Use the referenced methodology before drafting.
"#
                .to_vec(),
            },
            SkillImportFile {
                relative_path: "references/content_methodology.md".to_owned(),
                bytes: b"# Content Methodology\nPrefer evidence-first outlines.".to_vec(),
            },
            SkillImportFile {
                relative_path: "scripts/md_to_pdf.py".to_owned(),
                bytes: b"print('disabled by Novex')\n".to_vec(),
            },
        ];

        let bundle =
            parse_skill_directory_import(Some("https://github.com/x/y/tree/main/skill"), source)
                .unwrap();

        assert_eq!(bundle.skill.code, "khazix-writer");
        assert!(bundle
            .resources
            .iter()
            .any(|resource| resource.resource_type == "reference"
                && resource.relative_path == "references/content_methodology.md"
                && resource
                    .content_text
                    .as_deref()
                    .unwrap_or_default()
                    .contains("evidence-first")));
        let script = bundle
            .resources
            .iter()
            .find(|resource| resource.relative_path == "scripts/md_to_pdf.py")
            .unwrap();
        assert_eq!(script.resource_type, "script");
        assert_eq!(script.metadata["execution"], "disabled");
    }

    #[test]
    fn skill_zip_package_extracts_single_skill_directory() {
        let package = skill_zip_bytes(&[
            (
                "khazix-writer/SKILL.md",
                b"---\nname: khazix-writer\ndescription: Imported writer.\n---\n# Writer\nUse references.",
            ),
            (
                "khazix-writer/references/style_examples.md",
                b"# Style\nUse evidence first.",
            ),
        ]);

        let files = parse_skill_zip_package(Some("khazix-writer.zip"), &package).unwrap();
        let bundle = parse_skill_directory_import(Some("khazix-writer.zip"), files).unwrap();

        assert_eq!(bundle.skill.code, "khazix-writer");
        assert!(bundle.resources.iter().any(|resource| {
            resource.resource_type == "reference"
                && resource.relative_path == "references/style_examples.md"
        }));
    }

    #[test]
    fn skill_zip_package_rejects_path_traversal_entries() {
        let package = skill_zip_bytes(&[(
            "../SKILL.md",
            b"---\nname: unsafe\ndescription: Unsafe.\n---\n# Unsafe",
        )]);

        let err = parse_skill_zip_package(Some("unsafe.zip"), &package).unwrap_err();

        assert!(err.to_string().contains(".."));
    }

    #[test]
    fn skill_import_rejects_markdown_without_codex_frontmatter() {
        let err = parse_skill_import(Some("SKILL.md"), b"# Missing metadata").unwrap_err();

        assert!(err.to_string().contains("SKILL.md frontmatter"));
    }

    #[test]
    fn tool_dry_run_rejects_blank_tool_code() {
        let command = ToolDryRunCommand {
            tool_code: "   ".to_owned(),
            input: Value::Null,
        };

        let err = normalize_tool_dry_run_command(command).unwrap_err();

        assert!(err.to_string().contains("工具编码不能为空"));
    }

    #[test]
    fn tool_dry_run_response_includes_audit_id_and_status() {
        let resp = tool_dry_run_response(99, "rag.search", Value::Null);

        assert_eq!(resp.audit_id, 99);
        assert_eq!(resp.tool_code, "rag.search");
        assert_eq!(resp.status, "succeeded");
        assert!(resp.dry_run);
    }

    #[test]
    fn feishu_tool_registry_marks_agent_connector_executor() {
        let seed =
            include_str!("../../../migrations/202606050006_create_ai_capability_registry.sql");
        let promotion =
            include_str!("../../../migrations/202606060009_promote_feishu_tool_connector.sql");

        assert!(seed.contains("'feishu.message.send'"));
        assert!(seed.contains("'ai:agent:run', 'connector'"));
        assert!(seed.contains("\"liveCapable\":true"));
        assert!(!seed.contains("'feishu.message.send', 'Feishu Message', 'Dry-run metadata"));

        for needle in [
            "code = 'feishu.message.send'",
            "permission_code = 'ai:agent:run'",
            "executor_kind = 'connector'",
            "\"dryRunFallback\":\"missing_webhook_env\"",
        ] {
            assert!(promotion.contains(needle), "{needle} missing");
        }
    }

    #[test]
    fn customer_service_tool_seed_contains_tool_contracts() {
        let seed_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/202606160005_seed_customer_service_tools.sql"
        );
        let seed = std::fs::read_to_string(seed_path)
            .expect("missing customer service tool seed migration");

        for needle in [
            "'faq.search'",
            "'customer.lookup'",
            "'ticket.create'",
            "'handoff.request'",
            "'ai:customer-service:read'",
            "'ai:customer-service:ticket'",
            "'ai:customer-service:handoff'",
            "'ticket.create', 'Create Support Ticket'",
            "'handoff.request', 'Request Human Handoff'",
        ] {
            assert!(seed.contains(needle), "{needle} missing");
        }
    }

    #[test]
    fn skill_registry_seed_contains_every_template_skill_manifest() {
        let seed =
            include_str!("../../../migrations/202606050006_create_ai_capability_registry.sql");
        let templates = crate::application::ai::template_service::delivery_templates().unwrap();

        for template in templates {
            for skill in template.skills {
                assert!(
                    seed.contains(&format!("'{}'", skill.code)),
                    "skill seed missing template skill {} from {}",
                    skill.code,
                    template.code
                );
            }
        }
    }

    #[test]
    fn capability_registry_seed_contains_every_template_connector_plugin_and_trigger() {
        let seed =
            include_str!("../../../migrations/202606050006_create_ai_capability_registry.sql");
        let templates = crate::application::ai::template_service::delivery_templates().unwrap();

        for template in templates {
            for connector in template.connectors {
                assert!(
                    seed.contains(&format!("'{}'", connector.code)),
                    "connector seed missing template connector {} from {}",
                    connector.code,
                    template.code
                );
            }
            for plugin in template.plugins {
                assert!(
                    seed.contains(&format!("'{}'", plugin.code)),
                    "plugin seed missing template plugin {} from {}",
                    plugin.code,
                    template.code
                );
            }
            for trigger in template.triggers {
                assert!(
                    seed.contains(&format!("'{}'", trigger.code)),
                    "trigger seed missing template trigger {} from {}",
                    trigger.code,
                    template.code
                );
            }
        }
    }

    #[test]
    fn capability_registry_seed_plugin_manifests_match_builtin_contracts() {
        let seeds = [
            (
                "202606050006_create_ai_capability_registry",
                include_str!("../../../migrations/202606050006_create_ai_capability_registry.sql"),
            ),
            (
                "202606060012_seed_template_capability_aliases",
                include_str!(
                    "../../../migrations/202606060012_seed_template_capability_aliases.sql"
                ),
            ),
        ];

        for (seed_name, seed) in seeds {
            for plugin_code in ["builtin.agent-tools", "builtin.training-pack"] {
                if !seed.contains(plugin_code) {
                    continue;
                }
                let manifest = novex_plugin::builtin_plugin_manifest(plugin_code).unwrap();
                let fragment = plugin_seed_fragment(seed, plugin_code);

                for capability in &manifest.capabilities {
                    assert!(
                        fragment.contains(&format!("\"code\":\"{}\"", capability.code)),
                        "{seed_name} {plugin_code} manifest missing capability {}",
                        capability.code
                    );
                }
                for permission in novex_plugin::required_plugin_permissions(&manifest) {
                    assert!(
                        fragment.contains(&permission),
                        "{seed_name} {plugin_code} manifest missing permission {permission}"
                    );
                }
            }
        }
    }

    #[test]
    fn connector_credential_command_normalizes_scope_and_env_secret_ref() {
        let command = normalize_connector_credential_command(ConnectorCredentialCommand {
            connector_code: " github.default ".to_owned(),
            scope_type: " Tenant ".to_owned(),
            scope_id: " 1 ".to_owned(),
            auth_type: " OAuth_App ".to_owned(),
            secret_ref: " env:GITHUB_CONNECTOR_TOKEN ".to_owned(),
            scopes: json!(["repo"]),
            status: 1,
        })
        .expect("connector credential command should be valid");

        assert_eq!(command.connector_code, "github.default");
        assert_eq!(command.scope_type, "tenant");
        assert_eq!(command.scope_id, "1");
        assert_eq!(command.auth_type, "oauth_app");
        assert_eq!(command.secret_ref, "env:GITHUB_CONNECTOR_TOKEN");
    }

    #[test]
    fn connector_credential_command_rejects_plain_secret_values() {
        let err = normalize_connector_credential_command(ConnectorCredentialCommand {
            connector_code: "github.default".to_owned(),
            scope_type: "tenant".to_owned(),
            scope_id: "1".to_owned(),
            auth_type: "oauth_app".to_owned(),
            secret_ref: "github_pat_plaintext".to_owned(),
            scopes: json!([]),
            status: 1,
        })
        .unwrap_err();

        assert!(err.to_string().contains("secretRef"));
    }

    #[test]
    fn connector_credential_response_masks_secret_ref() {
        assert_eq!(
            masked_secret_ref("env:GITHUB_CONNECTOR_TOKEN"),
            "env:GITH****"
        );
        assert_eq!(masked_secret_ref("env:KEY"), "env:****");
    }

    #[test]
    fn plugin_install_command_normalizes_builtin_manifest_installation() {
        let command = normalize_plugin_install_command(PluginInstallCommand {
            plugin_code: " builtin.github-basic ".to_owned(),
            version: " 0.1.0 ".to_owned(),
            enabled: true,
            permission_grants: json!(["ai:connector:list", "ai:tool:dryRun"]),
            config: Value::Null,
        })
        .expect("plugin install command should normalize");

        assert_eq!(command.plugin_code, "builtin.github-basic");
        assert_eq!(command.version, "0.1.0");
        assert_eq!(command.config, json!({}));

        let response = serde_json::to_value(PluginInstallationResp {
            id: 1,
            plugin_id: 3230001,
            plugin_code: command.plugin_code,
            plugin_name: "GitHub Basic".to_owned(),
            version: command.version,
            enabled: true,
            permission_grants: command.permission_grants,
            capabilities: json!([{"kind":"tool","code":"github.repo.search"}]),
            config: command.config,
            install_source: "builtin".to_owned(),
            create_time: "2026-06-06 12:00:00".to_owned(),
            update_time: None,
        })
        .unwrap();

        assert_eq!(response["pluginCode"], "builtin.github-basic");
        assert_eq!(response["permissionGrants"][0], "ai:connector:list");
    }

    #[test]
    fn mcp_server_command_normalizes_http_registration_policy() {
        let command = normalize_mcp_server_command(McpServerCommand {
            code: " docs.search ".to_owned(),
            name: " Docs Search ".to_owned(),
            endpoint_url: Some(" https://mcp.example.com/sse ".to_owned()),
            transport_kind: " Streamable_HTTP ".to_owned(),
            auth_scope: " Tenant ".to_owned(),
            auth_type: " Bearer_Env ".to_owned(),
            secret_ref: Some(" env:DOCS_MCP_TOKEN ".to_owned()),
            network_allowlist: json!(["mcp.example.com"]),
            tool_allowlist: json!(["docs.search"]),
            discovered_tools: Value::Null,
            enabled: true,
        })
        .expect("mcp server command should normalize");

        assert_eq!(command.code, "docs.search");
        assert_eq!(command.name, "Docs Search");
        assert_eq!(command.transport_kind, "streamable_http");
        assert_eq!(command.auth_scope, "tenant");
        assert_eq!(command.auth_type, "bearer_env");
        assert_eq!(command.secret_ref.as_deref(), Some("env:DOCS_MCP_TOKEN"));
        assert_eq!(command.discovered_tools, json!([]));
    }

    #[test]
    fn mcp_server_command_requires_endpoint_host_in_allowlist() {
        let err = normalize_mcp_server_command(McpServerCommand {
            code: "docs.search".to_owned(),
            name: "Docs Search".to_owned(),
            endpoint_url: Some("https://mcp.example.com/sse".to_owned()),
            transport_kind: "streamable_http".to_owned(),
            auth_scope: "tenant".to_owned(),
            auth_type: "bearer_env".to_owned(),
            secret_ref: Some("env:DOCS_MCP_TOKEN".to_owned()),
            network_allowlist: json!(["api.example.com"]),
            tool_allowlist: json!(["docs.search"]),
            discovered_tools: json!([]),
            enabled: true,
        })
        .unwrap_err();

        assert!(err.to_string().contains("allow-list"));
    }

    #[test]
    fn mcp_discovery_persists_allowed_tools_as_ai_tools() {
        let now = Utc::now().naive_utc();
        let server = mcp_server_record(now, json!(["search"]));

        let plan = build_mcp_discovery_save_plan(
            1,
            7,
            now,
            &server,
            McpDiscoveryCommand {
                tools: vec![McpDiscoveryToolCommand {
                    tool_name: " search ".to_owned(),
                    description: " Search docs ".to_owned(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string"
                            }
                        }
                    }),
                    output_schema: Value::Null,
                    risk_level: 1,
                    metadata: Value::Null,
                    enabled: true,
                }],
            },
        )
        .expect("allowed MCP discovery should build save records");

        assert_eq!(plan.mcp_tools[0].tool_name, "search");
        assert_eq!(plan.mcp_tools[0].tool_code, "mcp.docs.search");
        assert_eq!(
            plan.mcp_tools[0].permission_code.as_deref(),
            Some("ai:mcp:docs:search")
        );
        assert_eq!(plan.tools[0].code, "mcp.docs.search");
        assert_eq!(plan.tools[0].tool_kind, "mcp");
        assert_eq!(plan.tools[0].executor_kind, "mcp");
        assert_eq!(
            plan.tools[0].input_schema["properties"]["query"]["type"],
            "string"
        );
    }

    #[test]
    fn mcp_discovery_rejects_unallowlisted_tool() {
        let now = Utc::now().naive_utc();
        let server = mcp_server_record(now, json!(["search"]));

        let err = build_mcp_discovery_save_plan(
            1,
            7,
            now,
            &server,
            McpDiscoveryCommand {
                tools: vec![McpDiscoveryToolCommand {
                    tool_name: "write".to_owned(),
                    description: "Write docs".to_owned(),
                    input_schema: json!({"type":"object"}),
                    output_schema: Value::Null,
                    risk_level: 2,
                    metadata: Value::Null,
                    enabled: true,
                }],
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("allow-list"));
    }

    fn mcp_server_record(now: chrono::NaiveDateTime, tool_allowlist: Value) -> McpServerRecord {
        McpServerRecord {
            id: 42,
            code: "docs".to_owned(),
            name: "Docs".to_owned(),
            endpoint_url: Some("https://mcp.example.com/mcp".to_owned()),
            transport_kind: "streamable_http".to_owned(),
            auth_scope: "tenant".to_owned(),
            auth_type: "bearer_env".to_owned(),
            secret_ref: Some("env:DOCS_MCP_TOKEN".to_owned()),
            network_allowlist: json!(["mcp.example.com"]),
            tool_allowlist,
            discovered_tools: json!([]),
            status: 1,
            create_time: now,
            update_time: None,
        }
    }

    fn plugin_seed_fragment<'a>(seed: &'a str, plugin_code: &str) -> &'a str {
        let start = seed
            .find(plugin_code)
            .unwrap_or_else(|| panic!("{plugin_code} seed missing"));
        let end = (start + 1_600).min(seed.len());
        &seed[start..end]
    }

    fn skill_zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        for (path, content) in entries {
            writer.start_file(*path, options).unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
}
