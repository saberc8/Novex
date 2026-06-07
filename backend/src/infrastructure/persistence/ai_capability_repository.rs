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
    pub approval_policy: i16,
    pub permission_code: Option<String>,
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
SELECT id, code, risk_level, approval_policy, permission_code
FROM ai_tool
WHERE tenant_id = $1 AND code = $2 AND status = 1;
"#,
        )
        .bind(tenant_id)
        .bind(tool_code)
        .fetch_optional(&self.db)
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
}
