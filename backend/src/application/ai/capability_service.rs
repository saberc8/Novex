use chrono::Utc;
use novex_mcp::{
    validate_mcp_registration_policy, McpAuthScope, McpAuthType, McpRegistrationPolicy,
    McpTransportKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::{
    application::system::{ensure_max_chars, format_datetime},
    infrastructure::persistence::ai_capability_repository::{
        AiCapabilityRepository, CapabilityFilter, CapabilityRecord, CapabilityResource,
        ConnectorCredentialFilter, ConnectorCredentialRecord, ConnectorCredentialSaveRecord,
        McpServerRecord, McpServerSaveRecord, PluginInstallationFilter, PluginInstallationRecord,
        PluginInstallationSaveRecord, ToolAuditFilter, ToolAuditRecord, ToolAuditSaveRecord,
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

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

    fn plugin_seed_fragment<'a>(seed: &'a str, plugin_code: &str) -> &'a str {
        let start = seed
            .find(plugin_code)
            .unwrap_or_else(|| panic!("{plugin_code} seed missing"));
        let end = (start + 1_600).min(seed.len());
        &seed[start..end]
    }
}
