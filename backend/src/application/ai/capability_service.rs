use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

use crate::{
    application::system::format_datetime,
    infrastructure::persistence::ai_capability_repository::{
        AiCapabilityRepository, CapabilityFilter, CapabilityRecord, CapabilityResource,
    },
    shared::{
        error::AppError,
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
}
