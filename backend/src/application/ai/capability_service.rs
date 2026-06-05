use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::{
    application::system::{ensure_max_chars, format_datetime},
    infrastructure::persistence::ai_capability_repository::{
        AiCapabilityRepository, CapabilityFilter, CapabilityRecord, CapabilityResource,
        ToolAuditFilter, ToolAuditRecord, ToolAuditSaveRecord,
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

#[derive(Debug, Clone)]
pub struct CapabilityService {
    repo: AiCapabilityRepository,
}

impl CapabilityService {
    pub fn new(db: PgPool) -> Self {
        Self {
            repo: AiCapabilityRepository::new(db),
        }
    }

    pub async fn summary(&self) -> Result<CapabilitySummaryResp, AppError> {
        let filter = summary_filter();
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

    pub async fn dry_run_tool(
        &self,
        user_id: i64,
        command: ToolDryRunCommand,
    ) -> Result<ToolDryRunResp, AppError> {
        let command = normalize_tool_dry_run_command(command)?;
        let Some(tool) = self
            .repo
            .find_tool_by_code(DEFAULT_TENANT_ID, &command.tool_code)
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
            tenant_id: DEFAULT_TENANT_ID,
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
            tenant_id: DEFAULT_TENANT_ID,
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

    async fn list(
        &self,
        resource: CapabilityResource,
        query: CapabilityQuery,
    ) -> Result<PageResult<CapabilityItemResp>, AppError> {
        let page = query.page_query();
        let filter = CapabilityFilter {
            tenant_id: DEFAULT_TENANT_ID,
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

fn summary_filter<'a>() -> CapabilityFilter<'a> {
    CapabilityFilter {
        tenant_id: DEFAULT_TENANT_ID,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
