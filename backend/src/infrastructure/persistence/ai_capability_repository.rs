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

#[derive(Debug, Clone)]
pub struct SkillSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub status: i16,
    pub model_route_policy: Value,
    pub capability_refs: Value,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct SkillResourceSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub skill_id: i64,
    pub resource_type: String,
    pub relative_path: String,
    pub mime_type: String,
    pub content_text: Option<String>,
    pub storage_ref: Option<String>,
    pub content_sha256: String,
    pub size_bytes: i64,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct SkillResourceRecord {
    pub id: i64,
    pub skill_id: i64,
    pub resource_type: String,
    pub relative_path: String,
    pub mime_type: String,
    pub content_text: Option<String>,
    pub storage_ref: Option<String>,
    pub content_sha256: String,
    pub size_bytes: i64,
    pub metadata: Value,
}

#[derive(Debug, Clone, FromRow)]
pub struct ToolLookupRecord {
    pub id: i64,
    pub code: String,
    pub tool_kind: String,
    pub executor_kind: String,
    pub risk_level: i16,
    pub approval_policy: i16,
    pub permission_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub tool_kind: String,
    pub risk_level: i16,
    pub approval_policy: i16,
    pub permission_code: Option<String>,
    pub executor_kind: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub status: i16,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct ConnectorCredentialLookupRecord {
    pub id: i64,
    pub connector_id: i64,
    pub connector_code: String,
    pub scope_type: String,
    pub scope_id: String,
    pub auth_type: String,
    pub secret_ref: String,
    pub scopes: Value,
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct ConnectorCredentialFilter<'a> {
    pub tenant_id: i64,
    pub connector_code: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct ConnectorCredentialSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub connector_code: String,
    pub scope_type: String,
    pub scope_id: String,
    pub auth_type: String,
    pub secret_ref: String,
    pub scopes: Value,
    pub status: i16,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct ConnectorCredentialRecord {
    pub id: i64,
    pub connector_id: i64,
    pub connector_code: String,
    pub scope_type: String,
    pub scope_id: String,
    pub auth_type: String,
    pub secret_ref: String,
    pub scopes: Value,
    pub status: i16,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct PluginInstallationFilter<'a> {
    pub tenant_id: i64,
    pub plugin_code: Option<&'a str>,
    pub enabled: Option<bool>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct PluginInstallationSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub plugin_code: String,
    pub version: String,
    pub enabled: bool,
    pub permission_grants: Value,
    pub config: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct PluginInstallationRecord {
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
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct McpServerSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub code: String,
    pub name: String,
    pub endpoint_url: Option<String>,
    pub transport_kind: String,
    pub auth_scope: String,
    pub auth_type: String,
    pub secret_ref: Option<String>,
    pub network_allowlist: Value,
    pub tool_allowlist: Value,
    pub discovered_tools: Value,
    pub status: i16,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct McpServerRecord {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub endpoint_url: Option<String>,
    pub transport_kind: String,
    pub auth_scope: String,
    pub auth_type: String,
    pub secret_ref: Option<String>,
    pub network_allowlist: Value,
    pub tool_allowlist: Value,
    pub discovered_tools: Value,
    pub status: i16,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct McpToolSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub server_id: i64,
    pub tool_name: String,
    pub tool_code: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub risk_level: i16,
    pub permission_code: Option<String>,
    pub status: i16,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct McpToolRecord {
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
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, FromRow)]
pub struct McpToolExecutionRecord {
    pub id: i64,
    pub server_id: i64,
    pub server_code: String,
    pub server_name: String,
    pub endpoint_url: Option<String>,
    pub transport_kind: String,
    pub auth_type: String,
    pub secret_ref: Option<String>,
    pub tool_name: String,
    pub tool_code: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub risk_level: i16,
    pub permission_code: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct McpOAuthStateSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub server_id: i64,
    pub server_code: String,
    pub scope_type: String,
    pub scope_id: String,
    pub state_hash: String,
    pub redirect_uri: String,
    pub requested_scopes: Value,
    pub code_verifier_secret_ref: String,
    pub client_auth: Value,
    pub token_endpoint: String,
    pub client_id: String,
    pub access_token_secret_ref: String,
    pub refresh_token_secret_ref: Option<String>,
    pub expires_at: NaiveDateTime,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct McpOAuthStateRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub server_id: i64,
    pub server_code: String,
    pub scope_type: String,
    pub scope_id: String,
    pub state_hash: String,
    pub redirect_uri: String,
    pub requested_scopes: Value,
    pub code_verifier_secret_ref: String,
    pub client_auth: Value,
    pub token_endpoint: String,
    pub client_id: String,
    pub access_token_secret_ref: String,
    pub refresh_token_secret_ref: Option<String>,
    pub expires_at: NaiveDateTime,
    pub consumed_at: Option<NaiveDateTime>,
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct McpOAuthSessionSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub server_id: i64,
    pub server_code: String,
    pub scope_type: String,
    pub scope_id: String,
    pub access_token_secret_ref: String,
    pub refresh_token_secret_ref: Option<String>,
    pub token_type: String,
    pub scopes: Value,
    pub expires_at: Option<NaiveDateTime>,
    pub refresh_needed_after: Option<NaiveDateTime>,
    pub metadata: Value,
    pub status: i16,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct McpOAuthSessionRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub server_id: i64,
    pub server_code: String,
    pub scope_type: String,
    pub scope_id: String,
    pub access_token_secret_ref: String,
    pub refresh_token_secret_ref: Option<String>,
    pub token_type: String,
    pub scopes: Value,
    pub expires_at: Option<NaiveDateTime>,
    pub refresh_needed_after: Option<NaiveDateTime>,
    pub last_refreshed_at: Option<NaiveDateTime>,
    pub revoked_at: Option<NaiveDateTime>,
    pub status: i16,
    pub metadata: Value,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, FromRow)]
pub struct TriggerLookupRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub code: String,
    pub target_kind: String,
    pub signature_secret_ref: Option<String>,
    pub route_config: Value,
}

#[derive(Debug, Clone)]
pub struct TriggerEventSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub trigger_id: i64,
    pub trigger_code: String,
    pub source_type: String,
    pub target_kind: String,
    pub idempotency_key: String,
    pub signature_header: String,
    pub event_payload: Value,
    pub route_snapshot: Value,
    pub status: String,
    pub trace_id: Option<i64>,
    pub error_message: Option<String>,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct TriggerEventRecord {
    pub id: i64,
    pub trigger_code: String,
    pub target_kind: String,
    pub idempotency_key: String,
    pub status: String,
    pub trace_id: Option<i64>,
    pub route_snapshot: Value,
}

#[derive(Debug, Clone)]
pub struct TriggerEventFilter<'a> {
    pub tenant_id: i64,
    pub trigger_code: Option<&'a str>,
    pub status: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct TriggerEventListRecord {
    pub id: i64,
    pub trigger_code: String,
    pub source_type: String,
    pub target_kind: String,
    pub idempotency_key: String,
    pub event_payload: Value,
    pub route_snapshot: Value,
    pub status: String,
    pub trace_id: Option<i64>,
    pub error_message: Option<String>,
    pub create_time: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct TriggerEventInsertOutcome {
    pub record: TriggerEventRecord,
    pub duplicate: bool,
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

const MCP_OAUTH_STATE_INSERT_SQL: &str = r#"
INSERT INTO ai_mcp_oauth_state (
    id, tenant_id, server_id, server_code, scope_type, scope_id, state_hash,
    redirect_uri, requested_scopes, code_verifier_secret_ref, client_auth,
    token_endpoint, client_id, access_token_secret_ref, refresh_token_secret_ref,
    expires_at, status, metadata, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6, $7,
    $8, $9, $10, $11,
    $12, $13, $14, $15,
    $16, 1, $17, $18, $19
)
ON CONFLICT (state_hash) DO NOTHING;
"#;

const MCP_OAUTH_STATE_CONSUME_SQL: &str = r#"
UPDATE ai_mcp_oauth_state
SET
    consumed_at = NOW(),
    update_time = NOW()
WHERE tenant_id = $1
  AND server_id = $2
  AND state_hash = $3
  AND redirect_uri = $4
  AND status = 1
  AND consumed_at IS NULL
  AND expires_at > NOW()
RETURNING
    id,
    tenant_id,
    server_id,
    server_code,
    scope_type,
    scope_id,
    state_hash,
    redirect_uri,
    requested_scopes,
    code_verifier_secret_ref,
    client_auth,
    token_endpoint,
    client_id,
    access_token_secret_ref,
    refresh_token_secret_ref,
    expires_at,
    consumed_at,
    metadata;
"#;

const MCP_OAUTH_SESSION_UPSERT_SQL: &str = r#"
INSERT INTO ai_mcp_oauth_session (
    id, tenant_id, server_id, server_code, scope_type, scope_id,
    access_token_secret_ref, refresh_token_secret_ref, token_type, scopes,
    expires_at, refresh_needed_after, last_refreshed_at, revoked_at, status,
    metadata, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6,
    $7, $8, $9, $10,
    $11, $12, $13, NULL, $14,
    $15, $16, $17
)
ON CONFLICT (tenant_id, server_id, scope_type, scope_id)
DO UPDATE SET
    server_code = EXCLUDED.server_code,
    access_token_secret_ref = EXCLUDED.access_token_secret_ref,
    refresh_token_secret_ref = EXCLUDED.refresh_token_secret_ref,
    token_type = EXCLUDED.token_type,
    scopes = EXCLUDED.scopes,
    expires_at = EXCLUDED.expires_at,
    refresh_needed_after = EXCLUDED.refresh_needed_after,
    last_refreshed_at = EXCLUDED.last_refreshed_at,
    revoked_at = NULL,
    status = EXCLUDED.status,
    metadata = EXCLUDED.metadata,
    update_user = $16,
    update_time = $17
RETURNING
    id,
    tenant_id,
    server_id,
    server_code,
    scope_type,
    scope_id,
    access_token_secret_ref,
    refresh_token_secret_ref,
    token_type,
    scopes,
    expires_at,
    refresh_needed_after,
    last_refreshed_at,
    revoked_at,
    status,
    metadata,
    create_time,
    update_time;
"#;

const MCP_OAUTH_SESSION_LOOKUP_SQL: &str = r#"
SELECT
    id,
    tenant_id,
    server_id,
    server_code,
    scope_type,
    scope_id,
    access_token_secret_ref,
    refresh_token_secret_ref,
    token_type,
    scopes,
    expires_at,
    refresh_needed_after,
    last_refreshed_at,
    revoked_at,
    status,
    metadata,
    create_time,
    update_time
FROM ai_mcp_oauth_session
WHERE tenant_id = $1
  AND server_id = $2
  AND scope_type = $3
  AND scope_id = $4
  AND status = 1
  AND revoked_at IS NULL
LIMIT 1;
"#;

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
SELECT id, code, tool_kind, executor_kind, risk_level, approval_policy, permission_code
FROM ai_tool
WHERE tenant_id = $1 AND code = $2 AND status = 1;
"#,
        )
        .bind(tenant_id)
        .bind(tool_code)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn upsert_tool(&self, record: &ToolSaveRecord) -> Result<CapabilityRecord, AppError> {
        Ok(sqlx::query_as::<_, CapabilityRecord>(
            r#"
INSERT INTO ai_tool (
    id, tenant_id, code, name, description, tool_kind, risk_level,
    approval_policy, permission_code, executor_kind, input_schema, output_schema,
    status, metadata, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6, $7,
    $8, $9, $10, $11, $12,
    $13, $14, $15, $16
)
ON CONFLICT (tenant_id, code)
DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    tool_kind = EXCLUDED.tool_kind,
    risk_level = EXCLUDED.risk_level,
    approval_policy = EXCLUDED.approval_policy,
    permission_code = EXCLUDED.permission_code,
    executor_kind = EXCLUDED.executor_kind,
    input_schema = EXCLUDED.input_schema,
    output_schema = EXCLUDED.output_schema,
    status = EXCLUDED.status,
    metadata = EXCLUDED.metadata,
    update_user = $15,
    update_time = $16
RETURNING
    id,
    code,
    name,
    COALESCE(description, '') AS description,
    tool_kind AS kind,
    status,
    risk_level,
    metadata,
    create_time;
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(&record.code)
        .bind(&record.name)
        .bind(&record.description)
        .bind(&record.tool_kind)
        .bind(record.risk_level)
        .bind(record.approval_policy)
        .bind(&record.permission_code)
        .bind(&record.executor_kind)
        .bind(&record.input_schema)
        .bind(&record.output_schema)
        .bind(record.status)
        .bind(&record.metadata)
        .bind(record.user_id)
        .bind(record.now)
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn upsert_skill(
        &self,
        record: &SkillSaveRecord,
    ) -> Result<CapabilityRecord, AppError> {
        Ok(sqlx::query_as::<_, CapabilityRecord>(
            r#"
INSERT INTO ai_skill (
    id, tenant_id, code, name, description, status, model_route_policy,
    capability_refs, metadata, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
ON CONFLICT (tenant_id, code)
DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    status = EXCLUDED.status,
    model_route_policy = EXCLUDED.model_route_policy,
    capability_refs = EXCLUDED.capability_refs,
    metadata = EXCLUDED.metadata,
    update_user = $10,
    update_time = $11
RETURNING
    id,
    code,
    name,
    COALESCE(description, '') AS description,
    code AS kind,
    status,
    NULL::smallint AS risk_level,
    metadata || jsonb_build_object(
        'modelRoutePolicy', model_route_policy,
        'capabilityRefs', capability_refs
    ) AS metadata,
    create_time;
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(&record.code)
        .bind(&record.name)
        .bind(&record.description)
        .bind(record.status)
        .bind(&record.model_route_policy)
        .bind(&record.capability_refs)
        .bind(&record.metadata)
        .bind(record.user_id)
        .bind(record.now)
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn replace_skill_resources(
        &self,
        tenant_id: i64,
        skill_id: i64,
        records: &[SkillResourceSaveRecord],
    ) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
DELETE FROM ai_skill_resource
WHERE tenant_id = $1 AND skill_id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(skill_id)
        .execute(&mut *tx)
        .await?;

        for record in records {
            sqlx::query(
                r#"
INSERT INTO ai_skill_resource (
    id, tenant_id, skill_id, resource_type, relative_path, mime_type,
    content_text, storage_ref, content_sha256, size_bytes, metadata,
    create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13);
"#,
            )
            .bind(record.id)
            .bind(record.tenant_id)
            .bind(record.skill_id)
            .bind(&record.resource_type)
            .bind(&record.relative_path)
            .bind(&record.mime_type)
            .bind(&record.content_text)
            .bind(&record.storage_ref)
            .bind(&record.content_sha256)
            .bind(record.size_bytes)
            .bind(&record.metadata)
            .bind(record.user_id)
            .bind(record.now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn list_skill_resources(
        &self,
        tenant_id: i64,
        skill_id: i64,
        resource_type: Option<&str>,
    ) -> Result<Vec<SkillResourceRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
SELECT
    id,
    skill_id,
    resource_type,
    relative_path,
    mime_type,
    content_text,
    storage_ref,
    content_sha256,
    size_bytes,
    metadata
FROM ai_skill_resource
WHERE tenant_id = "#,
        );
        query
            .push_bind(tenant_id)
            .push(" AND skill_id = ")
            .push_bind(skill_id)
            .push(" AND status = 1");
        if let Some(resource_type) = resource_type {
            query.push(" AND resource_type = ").push_bind(resource_type);
        }
        query.push(" ORDER BY relative_path ASC, id ASC");

        Ok(query
            .build_query_as::<SkillResourceRecord>()
            .fetch_all(&self.db)
            .await?)
    }

    pub async fn find_connector_credential(
        &self,
        tenant_id: i64,
        connector_code: &str,
        user_id: i64,
    ) -> Result<Option<ConnectorCredentialLookupRecord>, AppError> {
        let user_scope_id = user_id.to_string();
        let tenant_scope_id = tenant_id.to_string();
        Ok(sqlx::query_as::<_, ConnectorCredentialLookupRecord>(
            r#"
SELECT
    cc.id,
    cc.connector_id,
    c.code AS connector_code,
    cc.scope_type,
    cc.scope_id,
    cc.auth_type,
    cc.secret_ref,
    cc.scopes,
    cc.metadata
FROM ai_connector_credential AS cc
JOIN ai_connector AS c ON c.id = cc.connector_id
WHERE cc.tenant_id = $1
  AND c.tenant_id = $1
  AND c.code = $2
  AND c.status = 1
  AND cc.status = 1
  AND (cc.expires_at IS NULL OR cc.expires_at > NOW())
  AND (
      (cc.scope_type = 'user' AND cc.scope_id = $3)
      OR (cc.scope_type = 'tenant' AND cc.scope_id = $4)
      OR (cc.scope_type = 'app' AND cc.scope_id = $2)
  )
ORDER BY
  CASE
    WHEN cc.scope_type = 'user' AND cc.scope_id = $3 THEN 0
    WHEN cc.scope_type = 'tenant' AND cc.scope_id = $4 THEN 1
    WHEN cc.scope_type = 'app' AND cc.scope_id = $2 THEN 2
    ELSE 3
  END ASC,
  COALESCE(cc.update_time, cc.create_time) DESC,
  cc.id DESC
LIMIT 1;
"#,
        )
        .bind(tenant_id)
        .bind(connector_code)
        .bind(user_scope_id)
        .bind(tenant_scope_id)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn count_connector_credentials(
        &self,
        filter: &ConnectorCredentialFilter<'_>,
    ) -> Result<i64, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT COUNT(*) FROM ai_connector_credential AS cc \
             JOIN ai_connector AS c ON c.id = cc.connector_id \
             WHERE cc.tenant_id = ",
        );
        query.push_bind(filter.tenant_id);
        query
            .push(" AND c.tenant_id = ")
            .push_bind(filter.tenant_id);
        if let Some(connector_code) = filter.connector_code {
            query.push(" AND c.code = ").push_bind(connector_code);
        }

        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_connector_credentials(
        &self,
        filter: &ConnectorCredentialFilter<'_>,
    ) -> Result<Vec<ConnectorCredentialRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
SELECT
    cc.id,
    cc.connector_id,
    c.code AS connector_code,
    cc.scope_type,
    cc.scope_id,
    cc.auth_type,
    cc.secret_ref,
    cc.scopes,
    cc.status,
    cc.create_time,
    cc.update_time
FROM ai_connector_credential AS cc
JOIN ai_connector AS c ON c.id = cc.connector_id
WHERE cc.tenant_id = "#,
        );
        query.push_bind(filter.tenant_id);
        query
            .push(" AND c.tenant_id = ")
            .push_bind(filter.tenant_id);
        if let Some(connector_code) = filter.connector_code {
            query.push(" AND c.code = ").push_bind(connector_code);
        }
        query
            .push(" ORDER BY c.code ASC, cc.scope_type ASC, cc.scope_id ASC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);

        Ok(query
            .build_query_as::<ConnectorCredentialRecord>()
            .fetch_all(&self.db)
            .await?)
    }

    pub async fn upsert_connector_credential(
        &self,
        record: &ConnectorCredentialSaveRecord,
    ) -> Result<Option<ConnectorCredentialRecord>, AppError> {
        Ok(sqlx::query_as::<_, ConnectorCredentialRecord>(
            r#"
WITH connector AS (
    SELECT id, tenant_id, code
    FROM ai_connector
    WHERE tenant_id = $2
      AND code = $3
      AND status = 1
    LIMIT 1
),
upserted AS (
    INSERT INTO ai_connector_credential (
        id, tenant_id, connector_id, scope_type, scope_id, auth_type, secret_ref,
        scopes, status, metadata, create_user, create_time
    )
    SELECT
        $1, c.tenant_id, c.id, $4, $5, $6, $7, $8, $9,
        jsonb_build_object('configuredBy', $10::BIGINT, 'source', 'admin'),
        $10, $11
    FROM connector AS c
    ON CONFLICT (tenant_id, connector_id, scope_type, scope_id, auth_type)
    DO UPDATE SET
        secret_ref = EXCLUDED.secret_ref,
        scopes = EXCLUDED.scopes,
        status = EXCLUDED.status,
        update_user = $10,
        update_time = $11
    RETURNING id, tenant_id, connector_id, scope_type, scope_id, auth_type,
              secret_ref, scopes, status, create_time, update_time
)
SELECT
    u.id,
    u.connector_id,
    c.code AS connector_code,
    u.scope_type,
    u.scope_id,
    u.auth_type,
    u.secret_ref,
    u.scopes,
    u.status,
    u.create_time,
    u.update_time
FROM upserted AS u
JOIN ai_connector AS c ON c.id = u.connector_id AND c.tenant_id = u.tenant_id;
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(&record.connector_code)
        .bind(&record.scope_type)
        .bind(&record.scope_id)
        .bind(&record.auth_type)
        .bind(&record.secret_ref)
        .bind(&record.scopes)
        .bind(record.status)
        .bind(record.user_id)
        .bind(record.now)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn count_plugin_installations(
        &self,
        filter: &PluginInstallationFilter<'_>,
    ) -> Result<i64, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT COUNT(*) FROM ai_plugin_installation AS i \
             JOIN ai_plugin AS p ON p.id = i.plugin_id AND p.tenant_id = i.tenant_id \
             WHERE i.tenant_id = ",
        );
        query.push_bind(filter.tenant_id);
        push_plugin_installation_filters(&mut query, filter);

        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_plugin_installations(
        &self,
        filter: &PluginInstallationFilter<'_>,
    ) -> Result<Vec<PluginInstallationRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
SELECT
    i.id,
    i.plugin_id,
    p.code AS plugin_code,
    p.name AS plugin_name,
    v.version,
    i.enabled,
    i.permission_grants,
    COALESCE((
        SELECT jsonb_agg(
            jsonb_build_object(
                'kind', c.capability_kind,
                'code', c.capability_code,
                'permissionCode', COALESCE(c.permission_code, ''),
                'metadata', c.metadata
            )
            ORDER BY c.capability_kind ASC, c.capability_code ASC
        )
        FROM ai_plugin_capability AS c
        WHERE c.tenant_id = i.tenant_id
          AND c.plugin_version_id = i.plugin_version_id
          AND c.status = 1
    ), '[]'::jsonb) AS capabilities,
    i.config,
    i.install_source,
    i.create_time,
    i.update_time
FROM ai_plugin_installation AS i
JOIN ai_plugin AS p ON p.id = i.plugin_id AND p.tenant_id = i.tenant_id
JOIN ai_plugin_version AS v ON v.id = i.plugin_version_id AND v.tenant_id = i.tenant_id
WHERE i.tenant_id = "#,
        );
        query.push_bind(filter.tenant_id);
        push_plugin_installation_filters(&mut query, filter);
        query
            .push(" ORDER BY p.code ASC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);

        Ok(query
            .build_query_as::<PluginInstallationRecord>()
            .fetch_all(&self.db)
            .await?)
    }

    pub async fn upsert_plugin_installation(
        &self,
        record: &PluginInstallationSaveRecord,
    ) -> Result<Option<PluginInstallationRecord>, AppError> {
        Ok(sqlx::query_as::<_, PluginInstallationRecord>(
            r#"
WITH selected AS (
    SELECT
        p.id AS plugin_id,
        p.tenant_id,
        p.code AS plugin_code,
        p.name AS plugin_name,
        v.id AS plugin_version_id,
        v.version
    FROM ai_plugin AS p
    JOIN ai_plugin_version AS v ON v.plugin_id = p.id AND v.tenant_id = p.tenant_id
    WHERE p.tenant_id = $2
      AND p.code = $3
      AND v.version = $4
      AND p.status = 1
      AND v.status = 1
    LIMIT 1
),
upserted AS (
    INSERT INTO ai_plugin_installation (
        id, tenant_id, plugin_id, plugin_version_id, enabled, install_source,
        permission_grants, config, installed_by, installed_at, status, create_user, create_time
    )
    SELECT
        $1, s.tenant_id, s.plugin_id, s.plugin_version_id, $5, 'builtin',
        $6, $7, $8, $9, 1, $8, $9
    FROM selected AS s
    ON CONFLICT (tenant_id, plugin_id)
    DO UPDATE SET
        plugin_version_id = EXCLUDED.plugin_version_id,
        enabled = EXCLUDED.enabled,
        install_source = EXCLUDED.install_source,
        permission_grants = EXCLUDED.permission_grants,
        config = EXCLUDED.config,
        installed_by = EXCLUDED.installed_by,
        installed_at = EXCLUDED.installed_at,
        status = 1,
        update_user = $8,
        update_time = $9
    RETURNING id, tenant_id, plugin_id, plugin_version_id, enabled,
              permission_grants, config, install_source, create_time, update_time
)
SELECT
    u.id,
    u.plugin_id,
    p.code AS plugin_code,
    p.name AS plugin_name,
    v.version,
    u.enabled,
    u.permission_grants,
    COALESCE((
        SELECT jsonb_agg(
            jsonb_build_object(
                'kind', c.capability_kind,
                'code', c.capability_code,
                'permissionCode', COALESCE(c.permission_code, ''),
                'metadata', c.metadata
            )
            ORDER BY c.capability_kind ASC, c.capability_code ASC
        )
        FROM ai_plugin_capability AS c
        WHERE c.tenant_id = u.tenant_id
          AND c.plugin_version_id = u.plugin_version_id
          AND c.status = 1
    ), '[]'::jsonb) AS capabilities,
    u.config,
    u.install_source,
    u.create_time,
    u.update_time
FROM upserted AS u
JOIN ai_plugin AS p ON p.id = u.plugin_id AND p.tenant_id = u.tenant_id
JOIN ai_plugin_version AS v ON v.id = u.plugin_version_id AND v.tenant_id = u.tenant_id;
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(&record.plugin_code)
        .bind(&record.version)
        .bind(record.enabled)
        .bind(&record.permission_grants)
        .bind(&record.config)
        .bind(record.user_id)
        .bind(record.now)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn upsert_mcp_server(
        &self,
        record: &McpServerSaveRecord,
    ) -> Result<McpServerRecord, AppError> {
        Ok(sqlx::query_as::<_, McpServerRecord>(
            r#"
INSERT INTO ai_mcp_server (
    id, tenant_id, code, name, endpoint_url, transport_kind, status, auth_scope,
    auth_type, secret_ref, network_allowlist, tool_allowlist, discovered_tools,
    metadata, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8,
    $9, $10, $11, $12, $13,
    jsonb_build_object('source', 'admin', 'registeredBy', $14::BIGINT),
    $14, $15
)
ON CONFLICT (tenant_id, code)
DO UPDATE SET
    name = EXCLUDED.name,
    endpoint_url = EXCLUDED.endpoint_url,
    transport_kind = EXCLUDED.transport_kind,
    status = EXCLUDED.status,
    auth_scope = EXCLUDED.auth_scope,
    auth_type = EXCLUDED.auth_type,
    secret_ref = EXCLUDED.secret_ref,
    network_allowlist = EXCLUDED.network_allowlist,
    tool_allowlist = EXCLUDED.tool_allowlist,
    discovered_tools = EXCLUDED.discovered_tools,
    metadata = ai_mcp_server.metadata || jsonb_build_object('source', 'admin', 'registeredBy', $14::BIGINT),
    update_user = $14,
    update_time = $15
RETURNING
    id,
    code,
    name,
    endpoint_url,
    transport_kind,
    auth_scope,
    auth_type,
    secret_ref,
    network_allowlist,
    tool_allowlist,
    discovered_tools,
    status,
    create_time,
    update_time;
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(&record.code)
        .bind(&record.name)
        .bind(&record.endpoint_url)
        .bind(&record.transport_kind)
        .bind(record.status)
        .bind(&record.auth_scope)
        .bind(&record.auth_type)
        .bind(&record.secret_ref)
        .bind(&record.network_allowlist)
        .bind(&record.tool_allowlist)
        .bind(&record.discovered_tools)
        .bind(record.user_id)
        .bind(record.now)
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn find_mcp_server_by_id(
        &self,
        tenant_id: i64,
        server_id: i64,
    ) -> Result<Option<McpServerRecord>, AppError> {
        Ok(sqlx::query_as::<_, McpServerRecord>(
            r#"
SELECT
    id,
    code,
    name,
    endpoint_url,
    transport_kind,
    auth_scope,
    auth_type,
    secret_ref,
    network_allowlist,
    tool_allowlist,
    discovered_tools,
    status,
    create_time,
    update_time
FROM ai_mcp_server
WHERE tenant_id = $1
  AND id = $2
  AND status = 1
LIMIT 1;
"#,
        )
        .bind(tenant_id)
        .bind(server_id)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn update_mcp_server_discovered_tools(
        &self,
        tenant_id: i64,
        server_id: i64,
        discovered_tools: &Value,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_mcp_server
SET
    discovered_tools = $3,
    update_user = $4,
    update_time = $5
WHERE tenant_id = $1
  AND id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(server_id)
        .bind(discovered_tools)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn save_discovered_mcp_tools(
        &self,
        records: &[McpToolSaveRecord],
    ) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;
        for record in records {
            sqlx::query(
                r#"
INSERT INTO ai_mcp_tool (
    id, tenant_id, server_id, tool_name, tool_code, description,
    input_schema, output_schema, risk_level, permission_code, status,
    metadata, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6,
    $7, $8, $9, $10, $11,
    $12, $13, $14
)
ON CONFLICT (tenant_id, server_id, tool_name)
DO UPDATE SET
    tool_code = EXCLUDED.tool_code,
    description = EXCLUDED.description,
    input_schema = EXCLUDED.input_schema,
    output_schema = EXCLUDED.output_schema,
    risk_level = EXCLUDED.risk_level,
    permission_code = EXCLUDED.permission_code,
    status = EXCLUDED.status,
    metadata = EXCLUDED.metadata,
    update_user = $13,
    update_time = $14;
"#,
            )
            .bind(record.id)
            .bind(record.tenant_id)
            .bind(record.server_id)
            .bind(&record.tool_name)
            .bind(&record.tool_code)
            .bind(&record.description)
            .bind(&record.input_schema)
            .bind(&record.output_schema)
            .bind(record.risk_level)
            .bind(&record.permission_code)
            .bind(record.status)
            .bind(&record.metadata)
            .bind(record.user_id)
            .bind(record.now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_mcp_tools_by_server(
        &self,
        tenant_id: i64,
        server_id: i64,
    ) -> Result<Vec<McpToolRecord>, AppError> {
        Ok(sqlx::query_as::<_, McpToolRecord>(mcp_tool_select_sql())
            .bind(tenant_id)
            .bind(server_id)
            .fetch_all(&self.db)
            .await?)
    }

    pub async fn find_mcp_tool_by_tool_code(
        &self,
        tenant_id: i64,
        tool_code: &str,
    ) -> Result<Option<McpToolRecord>, AppError> {
        Ok(sqlx::query_as::<_, McpToolRecord>(
            r#"
SELECT
    t.id,
    t.server_id,
    s.code AS server_code,
    t.tool_name,
    t.tool_code,
    COALESCE(t.description, '') AS description,
    t.input_schema,
    t.output_schema,
    t.risk_level,
    t.permission_code,
    t.status,
    t.metadata,
    t.create_time,
    t.update_time
FROM ai_mcp_tool AS t
JOIN ai_mcp_server AS s ON s.id = t.server_id AND s.tenant_id = t.tenant_id
WHERE t.tenant_id = $1
  AND t.tool_code = $2
  AND t.status = 1
  AND s.status = 1
LIMIT 1;
"#,
        )
        .bind(tenant_id)
        .bind(tool_code)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn find_mcp_tool_for_execution(
        &self,
        tenant_id: i64,
        tool_code: &str,
    ) -> Result<Option<McpToolExecutionRecord>, AppError> {
        Ok(sqlx::query_as::<_, McpToolExecutionRecord>(
            r#"
SELECT
    t.id,
    t.server_id,
    s.code AS server_code,
    s.name AS server_name,
    s.endpoint_url,
    s.transport_kind,
    s.auth_type,
    s.secret_ref,
    t.tool_name,
    t.tool_code,
    COALESCE(t.description, '') AS description,
    t.input_schema,
    t.output_schema,
    t.risk_level,
    t.permission_code,
    t.metadata
FROM ai_mcp_tool AS t
JOIN ai_mcp_server AS s ON s.id = t.server_id AND s.tenant_id = t.tenant_id
WHERE t.tenant_id = $1
  AND t.tool_code = $2
  AND t.status = 1
  AND s.status = 1
LIMIT 1;
"#,
        )
        .bind(tenant_id)
        .bind(tool_code)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn save_mcp_oauth_state(
        &self,
        record: &McpOAuthStateSaveRecord,
    ) -> Result<(), AppError> {
        sqlx::query(MCP_OAUTH_STATE_INSERT_SQL)
            .bind(record.id)
            .bind(record.tenant_id)
            .bind(record.server_id)
            .bind(&record.server_code)
            .bind(&record.scope_type)
            .bind(&record.scope_id)
            .bind(&record.state_hash)
            .bind(&record.redirect_uri)
            .bind(&record.requested_scopes)
            .bind(&record.code_verifier_secret_ref)
            .bind(&record.client_auth)
            .bind(&record.token_endpoint)
            .bind(&record.client_id)
            .bind(&record.access_token_secret_ref)
            .bind(&record.refresh_token_secret_ref)
            .bind(record.expires_at)
            .bind(&record.metadata)
            .bind(record.user_id)
            .bind(record.now)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn consume_mcp_oauth_state(
        &self,
        tenant_id: i64,
        server_id: i64,
        state_hash: &str,
        redirect_uri: &str,
    ) -> Result<Option<McpOAuthStateRecord>, AppError> {
        Ok(
            sqlx::query_as::<_, McpOAuthStateRecord>(MCP_OAUTH_STATE_CONSUME_SQL)
                .bind(tenant_id)
                .bind(server_id)
                .bind(state_hash)
                .bind(redirect_uri)
                .fetch_optional(&self.db)
                .await?,
        )
    }

    pub async fn upsert_mcp_oauth_session(
        &self,
        record: &McpOAuthSessionSaveRecord,
    ) -> Result<McpOAuthSessionRecord, AppError> {
        Ok(
            sqlx::query_as::<_, McpOAuthSessionRecord>(MCP_OAUTH_SESSION_UPSERT_SQL)
                .bind(record.id)
                .bind(record.tenant_id)
                .bind(record.server_id)
                .bind(&record.server_code)
                .bind(&record.scope_type)
                .bind(&record.scope_id)
                .bind(&record.access_token_secret_ref)
                .bind(&record.refresh_token_secret_ref)
                .bind(&record.token_type)
                .bind(&record.scopes)
                .bind(record.expires_at)
                .bind(record.refresh_needed_after)
                .bind(record.now)
                .bind(record.status)
                .bind(&record.metadata)
                .bind(record.user_id)
                .bind(record.now)
                .fetch_one(&self.db)
                .await?,
        )
    }

    pub async fn find_mcp_oauth_session_for_scope(
        &self,
        tenant_id: i64,
        server_id: i64,
        scope_type: &str,
        scope_id: &str,
    ) -> Result<Option<McpOAuthSessionRecord>, AppError> {
        Ok(
            sqlx::query_as::<_, McpOAuthSessionRecord>(MCP_OAUTH_SESSION_LOOKUP_SQL)
                .bind(tenant_id)
                .bind(server_id)
                .bind(scope_type)
                .bind(scope_id)
                .fetch_optional(&self.db)
                .await?,
        )
    }

    pub async fn find_webhook_trigger(
        &self,
        tenant_id: i64,
        trigger_key: &str,
    ) -> Result<Option<TriggerLookupRecord>, AppError> {
        let route_path = format!("/ai/triggers/webhook/{trigger_key}");
        Ok(sqlx::query_as::<_, TriggerLookupRecord>(
            r#"
SELECT
    id,
    tenant_id,
    code,
    target_kind,
    COALESCE(signature_secret_ref, route_config ->> 'signatureSecretRef') AS signature_secret_ref,
    route_config
FROM ai_trigger
WHERE tenant_id = $1
  AND trigger_kind = 'webhook'
  AND status = 1
  AND (code = $2 OR route_config ->> 'path' = $3);
"#,
        )
        .bind(tenant_id)
        .bind(trigger_key)
        .bind(route_path)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn find_webhook_trigger_by_public_key(
        &self,
        trigger_key: &str,
    ) -> Result<Option<TriggerLookupRecord>, AppError> {
        let route_path = format!("/ai/triggers/webhook/{trigger_key}");
        let rows = sqlx::query_as::<_, TriggerLookupRecord>(
            r#"
SELECT
    id,
    tenant_id,
    code,
    target_kind,
    COALESCE(signature_secret_ref, route_config ->> 'signatureSecretRef') AS signature_secret_ref,
    route_config
FROM ai_trigger
WHERE trigger_kind = 'webhook'
  AND status = 1
  AND (code = $1 OR route_config ->> 'path' = $2)
ORDER BY tenant_id, id
LIMIT 2;
"#,
        )
        .bind(trigger_key)
        .bind(route_path)
        .fetch_all(&self.db)
        .await?;

        if rows.len() > 1 {
            return Err(AppError::conflict(
                "Webhook 触发器编码或路径不唯一，请使用唯一 public path",
            ));
        }

        Ok(rows.into_iter().next())
    }

    pub async fn create_trigger_event(
        &self,
        record: &TriggerEventSaveRecord,
    ) -> Result<TriggerEventInsertOutcome, AppError> {
        let inserted = sqlx::query_as::<_, TriggerEventRecord>(
            r#"
INSERT INTO ai_trigger_event (
    id, tenant_id, trigger_id, trigger_code, source_type, target_kind,
    idempotency_key, signature_header, event_payload, route_snapshot, status,
    trace_id, error_message, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
ON CONFLICT (tenant_id, trigger_id, idempotency_key) DO NOTHING
RETURNING id, trigger_code, target_kind, idempotency_key, status, trace_id, route_snapshot;
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.trigger_id)
        .bind(&record.trigger_code)
        .bind(&record.source_type)
        .bind(&record.target_kind)
        .bind(&record.idempotency_key)
        .bind(&record.signature_header)
        .bind(&record.event_payload)
        .bind(&record.route_snapshot)
        .bind(&record.status)
        .bind(record.trace_id)
        .bind(&record.error_message)
        .bind(record.user_id)
        .bind(record.now)
        .fetch_optional(&self.db)
        .await?;

        if let Some(record) = inserted {
            return Ok(TriggerEventInsertOutcome {
                record,
                duplicate: false,
            });
        }

        let existing = sqlx::query_as::<_, TriggerEventRecord>(
            r#"
SELECT id, trigger_code, target_kind, idempotency_key, status, trace_id, route_snapshot
FROM ai_trigger_event
WHERE tenant_id = $1 AND trigger_id = $2 AND idempotency_key = $3;
"#,
        )
        .bind(record.tenant_id)
        .bind(record.trigger_id)
        .bind(&record.idempotency_key)
        .fetch_one(&self.db)
        .await?;

        Ok(TriggerEventInsertOutcome {
            record: existing,
            duplicate: true,
        })
    }

    pub async fn count_trigger_events(
        &self,
        filter: &TriggerEventFilter<'_>,
    ) -> Result<i64, AppError> {
        let mut query = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ai_trigger_event AS e");
        query.push(" WHERE 1 = 1");
        push_trigger_event_filters(&mut query, filter);
        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_trigger_events(
        &self,
        filter: &TriggerEventFilter<'_>,
    ) -> Result<Vec<TriggerEventListRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
SELECT
    e.id,
    e.trigger_code,
    e.source_type,
    e.target_kind,
    e.idempotency_key,
    e.event_payload,
    e.route_snapshot,
    e.status,
    e.trace_id,
    e.error_message,
    e.create_time
FROM ai_trigger_event AS e
"#,
        );
        query.push(" WHERE 1 = 1");
        push_trigger_event_filters(&mut query, filter);
        query
            .push(" ORDER BY e.create_time DESC, e.id DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);
        Ok(query
            .build_query_as::<TriggerEventListRecord>()
            .fetch_all(&self.db)
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
    c.metadata || jsonb_build_object(
        'modelRoutePolicy', c.model_route_policy,
        'capabilityRefs', c.capability_refs
    ) AS metadata,
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
    c.metadata || jsonb_build_object('version', c.version, 'manifest', c.manifest) AS metadata,
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
    c.metadata || jsonb_build_object(
        'transportKind', c.transport_kind,
        'authScope', c.auth_scope,
        'authType', c.auth_type,
        'networkAllowlist', c.network_allowlist,
        'toolAllowlist', c.tool_allowlist,
        'discoveredTools', c.discovered_tools
    ) AS metadata,
    c.create_time
FROM ai_mcp_server AS c
"#
            }
        }
    }
}

fn mcp_tool_select_sql() -> &'static str {
    r#"
SELECT
    t.id,
    t.server_id,
    s.code AS server_code,
    t.tool_name,
    t.tool_code,
    COALESCE(t.description, '') AS description,
    t.input_schema,
    t.output_schema,
    t.risk_level,
    t.permission_code,
    t.status,
    t.metadata,
    t.create_time,
    t.update_time
FROM ai_mcp_tool AS t
JOIN ai_mcp_server AS s ON s.id = t.server_id AND s.tenant_id = t.tenant_id
WHERE t.tenant_id = $1
  AND t.server_id = $2
  AND t.status = 1
  AND s.status = 1
ORDER BY t.tool_name ASC, t.id ASC;
"#
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

fn push_trigger_event_filters(
    query: &mut QueryBuilder<'_, Postgres>,
    filter: &TriggerEventFilter<'_>,
) {
    query
        .push(" AND e.tenant_id = ")
        .push_bind(filter.tenant_id);
    if let Some(trigger_code) = non_empty(filter.trigger_code) {
        query
            .push(" AND e.trigger_code = ")
            .push_bind(trigger_code.to_owned());
    }
    if let Some(status) = non_empty(filter.status) {
        query.push(" AND e.status = ").push_bind(status.to_owned());
    }
}

fn push_plugin_installation_filters(
    query: &mut QueryBuilder<'_, Postgres>,
    filter: &PluginInstallationFilter<'_>,
) {
    if let Some(plugin_code) = non_empty(filter.plugin_code) {
        query
            .push(" AND p.code = ")
            .push_bind(plugin_code.to_owned());
    }
    if let Some(enabled) = filter.enabled {
        query.push(" AND i.enabled = ").push_bind(enabled);
    }
    query.push(" AND i.status = 1");
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use serde_json::json;

    use super::*;

    #[test]
    fn trigger_lookup_record_keeps_signature_secret_ref_and_route() {
        let record = TriggerLookupRecord {
            id: 1,
            tenant_id: 1,
            code: "webhook.training.event".to_owned(),
            target_kind: "run_graph".to_owned(),
            signature_secret_ref: Some("env:NOVEX_TRAINING_WEBHOOK_SECRET".to_owned()),
            route_config: json!({
                "path": "/ai/triggers/webhook/training",
                "signatureSecretRef": "env:NOVEX_TRAINING_WEBHOOK_SECRET"
            }),
        };

        assert_eq!(
            record.signature_secret_ref.as_deref(),
            Some("env:NOVEX_TRAINING_WEBHOOK_SECRET")
        );
        assert_eq!(record.route_config["path"], "/ai/triggers/webhook/training");
    }

    #[test]
    fn public_webhook_lookup_detects_ambiguous_trigger_keys() {
        let source = include_str!("ai_capability_repository.rs");
        let public_lookup = ["find_webhook_trigger", "by_public_key"].join("_");
        let limit_two = ["LIMIT", "2"].join(" ");
        let route_path_filter = ["route_config ->> 'path'", " = $2"].concat();

        assert!(source.contains(&format!("pub async fn {public_lookup}")));
        assert!(source.contains(&limit_two));
        assert!(source.contains(&route_path_filter));
    }

    #[test]
    fn trigger_event_save_record_preserves_idempotency_and_route_snapshot() {
        let now = NaiveDate::from_ymd_opt(2026, 6, 6)
            .unwrap()
            .and_hms_opt(1, 2, 3)
            .unwrap();
        let record = TriggerEventSaveRecord {
            id: 10,
            tenant_id: 1,
            trigger_id: 1,
            trigger_code: "webhook.training.event".to_owned(),
            source_type: "webhook".to_owned(),
            target_kind: "run_graph".to_owned(),
            idempotency_key: "tenant-1:event-7".to_owned(),
            signature_header: "sha256=abc".to_owned(),
            event_payload: json!({"event":"training.completed"}),
            route_snapshot: json!({"targetKind":"run_graph"}),
            status: "accepted".to_owned(),
            trace_id: Some(10),
            error_message: None,
            user_id: 1,
            now,
        };

        assert_eq!(record.idempotency_key, "tenant-1:event-7");
        assert_eq!(record.route_snapshot["targetKind"], "run_graph");
        assert_eq!(record.trace_id, Some(10));
    }

    #[test]
    fn plugin_installation_migration_defines_version_installation_and_capability_contract() {
        let migration =
            include_str!("../../../migrations/202606050006_create_ai_capability_registry.sql");

        assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_plugin_version"));
        assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_plugin_installation"));
        assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_plugin_capability"));
        assert!(migration.contains("permission_grants"));
        assert!(migration.contains("installed_by"));
    }

    #[test]
    fn mcp_server_migration_defines_registration_policy_contract() {
        let migration =
            include_str!("../../../migrations/202606050006_create_ai_capability_registry.sql");

        assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_mcp_server"));
        assert!(migration.contains("transport_kind"));
        assert!(migration.contains("auth_type"));
        assert!(migration.contains("secret_ref"));
        assert!(migration.contains("network_allowlist"));
        assert!(migration.contains("tool_allowlist"));
        assert!(migration.contains("discovered_tools"));
    }

    #[test]
    fn mcp_gateway_migration_defines_discovered_tool_table() {
        let migration = include_str!("../../../migrations/202606160001_create_ai_mcp_gateway.sql");

        assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_mcp_tool"));
        assert!(migration.contains("server_id"));
        assert!(migration.contains("tool_name"));
        assert!(migration.contains("tool_code"));
        assert!(migration.contains("input_schema"));
        assert!(migration.contains("output_schema"));
        assert!(migration.contains("uk_ai_mcp_tool_tenant_tool_code"));
    }

    #[test]
    fn mcp_oauth_persistence_migration_defines_state_and_session_contract() {
        let migration =
            include_str!("../../../migrations/202606180001_create_ai_mcp_oauth_persistence.sql");

        for required in [
            "CREATE TABLE IF NOT EXISTS ai_mcp_oauth_state",
            "CREATE TABLE IF NOT EXISTS ai_mcp_oauth_session",
            "server_id",
            "server_code",
            "scope_type",
            "scope_id",
            "state_hash",
            "redirect_uri",
            "requested_scopes",
            "code_verifier_secret_ref",
            "client_auth",
            "token_endpoint",
            "client_id",
            "access_token_secret_ref",
            "refresh_token_secret_ref",
            "expires_at",
            "consumed_at",
            "uk_ai_mcp_oauth_state_hash",
            "uk_ai_mcp_oauth_session_scope",
        ] {
            assert!(migration.contains(required), "missing {required}");
        }
        for forbidden in [
            "authorization_code",
            "access_token_value",
            "refresh_token_value",
            "plain_state",
        ] {
            assert!(
                !migration.contains(forbidden),
                "migration must not persist {forbidden}"
            );
        }
    }

    #[test]
    fn mcp_oauth_persistence_repository_consumes_state_once() {
        assert!(MCP_OAUTH_STATE_INSERT_SQL.contains("INSERT INTO ai_mcp_oauth_state"));
        assert!(MCP_OAUTH_STATE_INSERT_SQL.contains("ON CONFLICT (state_hash) DO NOTHING"));
        assert!(MCP_OAUTH_STATE_CONSUME_SQL.contains("UPDATE ai_mcp_oauth_state"));
        assert!(MCP_OAUTH_STATE_CONSUME_SQL.contains("consumed_at = NOW()"));
        assert!(MCP_OAUTH_STATE_CONSUME_SQL.contains("consumed_at IS NULL"));
        assert!(MCP_OAUTH_STATE_CONSUME_SQL.contains("expires_at > NOW()"));
        assert!(MCP_OAUTH_STATE_CONSUME_SQL.contains("state_hash = $3"));
        assert!(!MCP_OAUTH_STATE_CONSUME_SQL.contains("authorization_code"));
    }

    #[test]
    fn mcp_oauth_persistence_repository_upserts_secret_ref_session_scope() {
        assert!(MCP_OAUTH_SESSION_UPSERT_SQL.contains("INSERT INTO ai_mcp_oauth_session"));
        assert!(MCP_OAUTH_SESSION_UPSERT_SQL
            .contains("ON CONFLICT (tenant_id, server_id, scope_type, scope_id)"));
        assert!(MCP_OAUTH_SESSION_UPSERT_SQL.contains("access_token_secret_ref"));
        assert!(MCP_OAUTH_SESSION_UPSERT_SQL.contains("refresh_token_secret_ref"));
        assert!(MCP_OAUTH_SESSION_LOOKUP_SQL.contains("revoked_at IS NULL"));
        assert!(MCP_OAUTH_SESSION_LOOKUP_SQL.contains("scope_type = $3"));
        assert!(MCP_OAUTH_SESSION_LOOKUP_SQL.contains("scope_id = $4"));
        assert!(!MCP_OAUTH_SESSION_UPSERT_SQL.contains("access_token_value"));
        assert!(!MCP_OAUTH_SESSION_UPSERT_SQL.contains("refresh_token_value"));
    }
}
