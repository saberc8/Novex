use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};

use crate::shared::error::AppError;

#[derive(Debug, Clone)]
pub struct AiCapabilityRepository {
    db: PgPool,
}

#[derive(Debug, Clone, Copy)]
pub enum CapabilityResource {
    Skill,
    Tool,
    Connector,
    Plugin,
    Trigger,
    McpServer,
}

#[derive(Debug, Clone)]
pub struct CapabilityFilter<'a> {
    pub tenant_id: i64,
    pub status: Option<i16>,
    pub kind: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct CapabilityRecord {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub status: i16,
    pub risk_level: Option<i16>,
    pub metadata: Value,
    pub create_time: NaiveDateTime,
}

impl AiCapabilityRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn count(
        &self,
        resource: CapabilityResource,
        filter: &CapabilityFilter<'_>,
    ) -> Result<i64, AppError> {
        let mut query = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ");
        query.push(resource.table_name()).push(" AS c WHERE 1 = 1");
        push_filters(&mut query, resource, filter);
        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list(
        &self,
        resource: CapabilityResource,
        filter: &CapabilityFilter<'_>,
    ) -> Result<Vec<CapabilityRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(resource.select_sql());
        query.push(" WHERE 1 = 1");
        push_filters(&mut query, resource, filter);
        query
            .push(" ORDER BY c.create_time DESC, c.id DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);
        Ok(query
            .build_query_as::<CapabilityRecord>()
            .fetch_all(&self.db)
            .await?)
    }
}

impl CapabilityResource {
    fn table_name(self) -> &'static str {
        match self {
            Self::Skill => "ai_skill",
            Self::Tool => "ai_tool",
            Self::Connector => "ai_connector",
            Self::Plugin => "ai_plugin",
            Self::Trigger => "ai_trigger",
            Self::McpServer => "ai_mcp_server",
        }
    }

    fn kind_column(self) -> &'static str {
        match self {
            Self::Skill => "code",
            Self::Tool => "tool_kind",
            Self::Connector => "connector_kind",
            Self::Plugin => "runtime",
            Self::Trigger => "trigger_kind",
            Self::McpServer => "auth_scope",
        }
    }

    fn select_sql(self) -> &'static str {
        match self {
            Self::Skill => {
                r#"
SELECT
    c.id,
    c.code,
    c.name,
    COALESCE(c.description, '') AS description,
    c.code AS kind,
    c.status,
    NULL::smallint AS risk_level,
    c.metadata,
    c.create_time
FROM ai_skill AS c
"#
            }
            Self::Tool => {
                r#"
SELECT
    c.id,
    c.code,
    c.name,
    COALESCE(c.description, '') AS description,
    c.tool_kind AS kind,
    c.status,
    c.risk_level,
    c.metadata,
    c.create_time
FROM ai_tool AS c
"#
            }
            Self::Connector => {
                r#"
SELECT
    c.id,
    c.code,
    c.name,
    COALESCE(c.description, '') AS description,
    c.connector_kind AS kind,
    c.status,
    NULL::smallint AS risk_level,
    c.metadata,
    c.create_time
FROM ai_connector AS c
"#
            }
            Self::Plugin => {
                r#"
SELECT
    c.id,
    c.code,
    c.name,
    '' AS description,
    c.runtime AS kind,
    c.status,
    NULL::smallint AS risk_level,
    c.metadata,
    c.create_time
FROM ai_plugin AS c
"#
            }
            Self::Trigger => {
                r#"
SELECT
    c.id,
    c.code,
    c.name,
    COALESCE(c.description, '') AS description,
    c.trigger_kind AS kind,
    c.status,
    NULL::smallint AS risk_level,
    c.metadata,
    c.create_time
FROM ai_trigger AS c
"#
            }
            Self::McpServer => {
                r#"
SELECT
    c.id,
    c.code,
    c.name,
    COALESCE(c.endpoint_url, '') AS description,
    c.auth_scope AS kind,
    c.status,
    NULL::smallint AS risk_level,
    c.metadata,
    c.create_time
FROM ai_mcp_server AS c
"#
            }
        }
    }
}

fn push_filters(
    query: &mut QueryBuilder<'_, Postgres>,
    resource: CapabilityResource,
    filter: &CapabilityFilter<'_>,
) {
    query
        .push(" AND c.tenant_id = ")
        .push_bind(filter.tenant_id);
    if let Some(status) = filter.status.filter(|value| *value > 0) {
        query.push(" AND c.status = ").push_bind(status);
    }
    if let Some(kind) = non_empty(filter.kind) {
        query
            .push(" AND c.")
            .push(resource.kind_column())
            .push(" = ")
            .push_bind(kind.to_owned());
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
