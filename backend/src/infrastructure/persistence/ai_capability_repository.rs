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

#[derive(Debug, Clone, FromRow)]
pub struct ToolLookupRecord {
    pub id: i64,
    pub code: String,
    pub risk_level: i16,
    pub permission_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolAuditFilter<'a> {
    pub tenant_id: i64,
    pub tool_code: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct ToolAuditSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub tool_id: i64,
    pub tool_code: String,
    pub caller_kind: String,
    pub caller_id: Option<i64>,
    pub request_payload: Value,
    pub response_payload: Value,
    pub status: String,
    pub dry_run: bool,
    pub risk_level: i16,
    pub permission_code: Option<String>,
    pub error_message: Option<String>,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct ToolAuditRecord {
    pub id: i64,
    pub tool_code: String,
    pub status: String,
    pub dry_run: bool,
    pub risk_level: i16,
    pub permission_code: String,
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

    pub async fn find_tool_by_code(
        &self,
        tenant_id: i64,
        tool_code: &str,
    ) -> Result<Option<ToolLookupRecord>, AppError> {
        Ok(sqlx::query_as::<_, ToolLookupRecord>(
            r#"
SELECT id, code, risk_level, permission_code
FROM ai_tool
WHERE tenant_id = $1 AND code = $2 AND status = 1;
"#,
        )
        .bind(tenant_id)
        .bind(tool_code)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn create_tool_call_audit(
        &self,
        record: &ToolAuditSaveRecord,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_tool_call_audit (
    id, tenant_id, tool_id, tool_code, caller_kind, caller_id, request_payload,
    response_payload, status, dry_run, risk_level, permission_code, error_message,
    create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.tool_id)
        .bind(&record.tool_code)
        .bind(&record.caller_kind)
        .bind(record.caller_id)
        .bind(&record.request_payload)
        .bind(&record.response_payload)
        .bind(&record.status)
        .bind(record.dry_run)
        .bind(record.risk_level)
        .bind(&record.permission_code)
        .bind(&record.error_message)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn count_tool_call_audits(
        &self,
        filter: &ToolAuditFilter<'_>,
    ) -> Result<i64, AppError> {
        let mut query =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ai_tool_call_audit AS a");
        query.push(" WHERE 1 = 1");
        push_tool_audit_filters(&mut query, filter);
        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_tool_call_audits(
        &self,
        filter: &ToolAuditFilter<'_>,
    ) -> Result<Vec<ToolAuditRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
SELECT
    a.id,
    a.tool_code,
    a.status,
    a.dry_run,
    a.risk_level,
    COALESCE(a.permission_code, '') AS permission_code,
    a.create_time
FROM ai_tool_call_audit AS a
"#,
        );
        query.push(" WHERE 1 = 1");
        push_tool_audit_filters(&mut query, filter);
        query
            .push(" ORDER BY a.create_time DESC, a.id DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);
        Ok(query
            .build_query_as::<ToolAuditRecord>()
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

fn push_tool_audit_filters(query: &mut QueryBuilder<'_, Postgres>, filter: &ToolAuditFilter<'_>) {
    query
        .push(" AND a.tenant_id = ")
        .push_bind(filter.tenant_id);
    if let Some(tool_code) = non_empty(filter.tool_code) {
        query
            .push(" AND a.tool_code = ")
            .push_bind(tool_code.to_owned());
    }
}
