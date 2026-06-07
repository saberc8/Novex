use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};

use crate::shared::error::AppError;

#[derive(Debug, Clone)]
pub struct AiIntegrationRepository {
    db: PgPool,
}

#[derive(Debug, Clone)]
pub struct IntegrationFilter<'a> {
    pub tenant_id: i64,
    pub app_id: Option<&'a str>,
    pub status: Option<i16>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct ApiKeySaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub app_id: String,
    pub name: String,
    pub key_prefix: String,
    pub key_hash: String,
    pub masked_key: String,
    pub permission_scope: Value,
    pub qps_limit: i32,
    pub quota_limit: i64,
    pub expires_at: Option<NaiveDateTime>,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct ApiKeyRecord {
    pub id: i64,
    pub app_id: String,
    pub name: String,
    pub key_prefix: String,
    pub masked_key: String,
    pub permission_scope: Value,
    pub qps_limit: i32,
    pub quota_limit: i64,
    pub expires_at: Option<NaiveDateTime>,
    pub last_used_at: Option<NaiveDateTime>,
    pub status: i16,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, FromRow)]
pub struct RuntimeApiKeyRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub create_user: i64,
    pub app_id: String,
    pub name: String,
    pub masked_key: String,
    pub permission_scope: Value,
    pub qps_limit: i32,
    pub quota_limit: i64,
    pub expires_at: Option<NaiveDateTime>,
    pub last_used_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct PublicLinkSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub app_id: String,
    pub name: String,
    pub path: String,
    pub token_hash: String,
    pub masked_token: String,
    pub public_url: String,
    pub permission_scope: Value,
    pub qps_limit: i32,
    pub quota_limit: i64,
    pub expires_at: Option<NaiveDateTime>,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct PublicLinkRecord {
    pub id: i64,
    pub app_id: String,
    pub name: String,
    pub path: String,
    pub public_url: String,
    pub masked_token: String,
    pub permission_scope: Value,
    pub qps_limit: i32,
    pub quota_limit: i64,
    pub expires_at: Option<NaiveDateTime>,
    pub last_used_at: Option<NaiveDateTime>,
    pub status: i16,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, FromRow)]
pub struct RuntimePublicLinkRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub create_user: i64,
    pub app_id: String,
    pub name: String,
    pub path: String,
    pub public_url: String,
    pub masked_token: String,
    pub permission_scope: Value,
    pub qps_limit: i32,
    pub quota_limit: i64,
    pub expires_at: Option<NaiveDateTime>,
    pub last_used_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct UsageMeterIncrementRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub scope_type: String,
    pub scope_id: String,
    pub resource_type: String,
    pub usage_unit: String,
    pub window_start: NaiveDateTime,
    pub window_end: NaiveDateTime,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct UsageMeterSummaryFilter {
    pub tenant_id: i64,
    pub scope_type: String,
    pub scope_ids: Vec<String>,
    pub qps_resource_type: String,
    pub qps_window_start: NaiveDateTime,
    pub qps_window_end: NaiveDateTime,
    pub quota_resource_type: String,
    pub quota_window_start: NaiveDateTime,
    pub quota_window_end: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct UsageMeterSummaryRecord {
    pub scope_id: String,
    pub resource_type: String,
    pub usage_value: i64,
    pub window_start: NaiveDateTime,
}

impl AiIntegrationRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn count_api_keys(&self, filter: &IntegrationFilter<'_>) -> Result<i64, AppError> {
        let mut query =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ai_api_key AS k WHERE 1 = 1");
        push_integration_filters(&mut query, "k", filter);
        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_api_keys(
        &self,
        filter: &IntegrationFilter<'_>,
    ) -> Result<Vec<ApiKeyRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
SELECT
    k.id,
    k.app_id,
    k.name,
    k.key_prefix,
    k.masked_key,
    k.permission_scope,
    k.qps_limit,
    k.quota_limit,
    k.expires_at,
    k.last_used_at,
    k.status,
    k.create_time,
    k.update_time
FROM ai_api_key AS k
WHERE 1 = 1"#,
        );
        push_integration_filters(&mut query, "k", filter);
        query
            .push(" ORDER BY k.create_time DESC, k.id DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);

        Ok(query
            .build_query_as::<ApiKeyRecord>()
            .fetch_all(&self.db)
            .await?)
    }

    pub async fn create_api_key(
        &self,
        record: &ApiKeySaveRecord,
    ) -> Result<ApiKeyRecord, AppError> {
        Ok(sqlx::query_as::<_, ApiKeyRecord>(
            r#"
INSERT INTO ai_api_key (
    id, tenant_id, app_id, name, key_prefix, key_hash, masked_key,
    permission_scope, qps_limit, quota_limit, expires_at, metadata,
    status, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 1, $13, $14)
RETURNING
    id, app_id, name, key_prefix, masked_key, permission_scope, qps_limit,
    quota_limit, expires_at, last_used_at, status, create_time, update_time;
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(&record.app_id)
        .bind(&record.name)
        .bind(&record.key_prefix)
        .bind(&record.key_hash)
        .bind(&record.masked_key)
        .bind(&record.permission_scope)
        .bind(record.qps_limit)
        .bind(record.quota_limit)
        .bind(record.expires_at)
        .bind(&record.metadata)
        .bind(record.user_id)
        .bind(record.now)
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn revoke_api_key(
        &self,
        tenant_id: i64,
        id: i64,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<bool, AppError> {
        let affected = sqlx::query(
            r#"
UPDATE ai_api_key
SET status = 0, revoked_at = $4, update_user = $3, update_time = $4
WHERE tenant_id = $1 AND id = $2 AND status = 1;
"#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?
        .rows_affected();

        Ok(affected > 0)
    }

    pub async fn find_runtime_api_key_by_hash(
        &self,
        key_hash: &str,
        now: NaiveDateTime,
    ) -> Result<Option<RuntimeApiKeyRecord>, AppError> {
        Ok(sqlx::query_as::<_, RuntimeApiKeyRecord>(
            r#"
SELECT
    id, tenant_id, create_user, app_id, name, masked_key, permission_scope, qps_limit,
    quota_limit, expires_at, last_used_at
FROM ai_api_key
WHERE key_hash = $1
  AND status = 1
  AND revoked_at IS NULL
  AND (expires_at IS NULL OR expires_at > $2);
"#,
        )
        .bind(key_hash)
        .bind(now)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn touch_api_key_last_used(
        &self,
        id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_api_key
SET last_used_at = $2, update_time = $2
WHERE id = $1;
"#,
        )
        .bind(id)
        .bind(now)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn count_public_links(
        &self,
        filter: &IntegrationFilter<'_>,
    ) -> Result<i64, AppError> {
        let mut query =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ai_public_link AS l WHERE 1 = 1");
        push_integration_filters(&mut query, "l", filter);
        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_public_links(
        &self,
        filter: &IntegrationFilter<'_>,
    ) -> Result<Vec<PublicLinkRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
SELECT
    l.id,
    l.app_id,
    l.name,
    l.path,
    l.public_url,
    l.masked_token,
    l.permission_scope,
    l.qps_limit,
    l.quota_limit,
    l.expires_at,
    l.last_used_at,
    l.status,
    l.create_time,
    l.update_time
FROM ai_public_link AS l
WHERE 1 = 1"#,
        );
        push_integration_filters(&mut query, "l", filter);
        query
            .push(" ORDER BY l.create_time DESC, l.id DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);

        Ok(query
            .build_query_as::<PublicLinkRecord>()
            .fetch_all(&self.db)
            .await?)
    }

    pub async fn create_public_link(
        &self,
        record: &PublicLinkSaveRecord,
    ) -> Result<PublicLinkRecord, AppError> {
        Ok(sqlx::query_as::<_, PublicLinkRecord>(
            r#"
INSERT INTO ai_public_link (
    id, tenant_id, app_id, name, path, token_hash, masked_token, public_url,
    permission_scope, qps_limit, quota_limit, expires_at, metadata,
    status, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 1, $14, $15)
RETURNING
    id, app_id, name, path, public_url, masked_token, permission_scope,
    qps_limit, quota_limit, expires_at, last_used_at, status, create_time, update_time;
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(&record.app_id)
        .bind(&record.name)
        .bind(&record.path)
        .bind(&record.token_hash)
        .bind(&record.masked_token)
        .bind(&record.public_url)
        .bind(&record.permission_scope)
        .bind(record.qps_limit)
        .bind(record.quota_limit)
        .bind(record.expires_at)
        .bind(&record.metadata)
        .bind(record.user_id)
        .bind(record.now)
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn revoke_public_link(
        &self,
        tenant_id: i64,
        id: i64,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<bool, AppError> {
        let affected = sqlx::query(
            r#"
UPDATE ai_public_link
SET status = 0, revoked_at = $4, update_user = $3, update_time = $4
WHERE tenant_id = $1 AND id = $2 AND status = 1;
"#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?
        .rows_affected();

        Ok(affected > 0)
    }

    pub async fn find_runtime_public_link_by_token_hash(
        &self,
        token_hash: &str,
        now: NaiveDateTime,
    ) -> Result<Option<RuntimePublicLinkRecord>, AppError> {
        Ok(sqlx::query_as::<_, RuntimePublicLinkRecord>(
            r#"
SELECT
    id, tenant_id, create_user, app_id, name, path, public_url, masked_token,
    permission_scope, qps_limit, quota_limit, expires_at, last_used_at
FROM ai_public_link
WHERE token_hash = $1
  AND status = 1
  AND revoked_at IS NULL
  AND (expires_at IS NULL OR expires_at > $2);
"#,
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn touch_public_link_last_used(
        &self,
        id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_public_link
SET last_used_at = $2, update_time = $2
WHERE id = $1;
"#,
        )
        .bind(id)
        .bind(now)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn increment_usage_meter(
        &self,
        record: &UsageMeterIncrementRecord,
    ) -> Result<i64, AppError> {
        Ok(sqlx::query_scalar::<_, i64>(
            r#"
INSERT INTO sys_usage_meter (
    id, tenant_id, scope_type, scope_id, resource_type, usage_value,
    usage_unit, window_start, window_end, metadata, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, 1, $6, $7, $8, $9, $10, $11)
ON CONFLICT (tenant_id, scope_type, scope_id, resource_type, usage_unit, window_start, window_end)
DO UPDATE SET
    usage_value = sys_usage_meter.usage_value + 1,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time
RETURNING usage_value::BIGINT;
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(&record.scope_type)
        .bind(&record.scope_id)
        .bind(&record.resource_type)
        .bind(&record.usage_unit)
        .bind(record.window_start)
        .bind(record.window_end)
        .bind(&record.metadata)
        .bind(record.user_id)
        .bind(record.now)
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn list_usage_meter_summaries(
        &self,
        filter: &UsageMeterSummaryFilter,
    ) -> Result<Vec<UsageMeterSummaryRecord>, AppError> {
        if filter.scope_ids.is_empty() {
            return Ok(Vec::new());
        }

        Ok(sqlx::query_as::<_, UsageMeterSummaryRecord>(
            r#"
SELECT
    scope_id,
    resource_type,
    usage_value::BIGINT AS usage_value,
    window_start
FROM sys_usage_meter
WHERE tenant_id = $1
  AND scope_type = $2
  AND scope_id = ANY($3)
  AND (
      (resource_type = $4 AND window_start = $5 AND window_end = $6)
      OR
      (resource_type = $7 AND window_start = $8 AND window_end = $9)
  )
ORDER BY scope_id ASC, resource_type ASC;
"#,
        )
        .bind(filter.tenant_id)
        .bind(&filter.scope_type)
        .bind(&filter.scope_ids)
        .bind(&filter.qps_resource_type)
        .bind(filter.qps_window_start)
        .bind(filter.qps_window_end)
        .bind(&filter.quota_resource_type)
        .bind(filter.quota_window_start)
        .bind(filter.quota_window_end)
        .fetch_all(&self.db)
        .await?)
    }
}

fn push_integration_filters(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    filter: &IntegrationFilter<'_>,
) {
    query
        .push(" AND ")
        .push(alias)
        .push(".tenant_id = ")
        .push_bind(filter.tenant_id);
    if let Some(status) = filter.status {
        query
            .push(" AND ")
            .push(alias)
            .push(".status = ")
            .push_bind(status);
    }
    if let Some(app_id) = non_empty(filter.app_id) {
        query
            .push(" AND ")
            .push(alias)
            .push(".app_id = ")
            .push_bind(app_id.to_owned());
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    #[test]
    fn integration_migration_defines_api_key_and_public_link_contracts() {
        let migration =
            include_str!("../../../migrations/202606050014_create_foundation_control_plane.sql");

        for required in [
            "CREATE TABLE IF NOT EXISTS ai_api_key",
            "CREATE TABLE IF NOT EXISTS ai_public_link",
            "key_hash",
            "masked_key",
            "token_hash",
            "masked_token",
            "permission_scope",
            "qps_limit",
            "quota_limit",
            "revoked_at",
        ] {
            assert!(migration.contains(required), "missing {required}");
        }
    }

    #[test]
    fn integration_migration_defines_usage_meter_contract_for_runtime_limits() {
        let migration =
            include_str!("../../../migrations/202606050014_create_foundation_control_plane.sql");

        for required in [
            "CREATE TABLE IF NOT EXISTS sys_usage_meter",
            "scope_type",
            "scope_id",
            "resource_type",
            "usage_value",
            "usage_unit",
            "window_start",
            "window_end",
            "uk_sys_usage_meter_window",
        ] {
            assert!(migration.contains(required), "missing {required}");
        }
    }
}
