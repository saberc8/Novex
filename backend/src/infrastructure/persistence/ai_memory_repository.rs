use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};

use crate::shared::error::AppError;

#[derive(Debug, Clone)]
pub struct AiMemoryRepository {
    db: PgPool,
}

#[derive(Debug, Clone)]
pub struct MemoryFilter<'a> {
    pub tenant_id: i64,
    pub scope_type: Option<&'a str>,
    pub scope_id: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct MemorySaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub scope_type: String,
    pub scope_id: String,
    pub owner_user_id: Option<i64>,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub content: String,
    pub summary: String,
    pub sensitivity: String,
    pub write_policy: String,
    pub ttl_days: Option<i32>,
    pub expires_at: Option<NaiveDateTime>,
    pub metadata: Value,
    pub status: i16,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct MemoryRecord {
    pub id: i64,
    pub scope_type: String,
    pub scope_id: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub content: String,
    pub summary: String,
    pub sensitivity: String,
    pub write_policy: String,
    pub ttl_days: Option<i32>,
    pub expires_at: Option<NaiveDateTime>,
    pub metadata: Value,
    pub status: i16,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

impl AiMemoryRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn count_memories(&self, filter: &MemoryFilter<'_>) -> Result<i64, AppError> {
        let mut query =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ai_memory AS m WHERE 1 = 1");
        push_memory_filters(&mut query, filter);
        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_memories(
        &self,
        filter: &MemoryFilter<'_>,
    ) -> Result<Vec<MemoryRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
SELECT
    m.id,
    m.scope_type,
    m.scope_id,
    m.source_kind,
    m.source_id,
    m.content,
    m.summary,
    m.sensitivity,
    m.write_policy,
    m.ttl_days,
    m.expires_at,
    m.metadata,
    m.status,
    m.create_time,
    m.update_time
FROM ai_memory AS m
WHERE 1 = 1"#,
        );
        push_memory_filters(&mut query, filter);
        query
            .push(" ORDER BY m.create_time DESC, m.id DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);

        Ok(query
            .build_query_as::<MemoryRecord>()
            .fetch_all(&self.db)
            .await?)
    }

    pub async fn upsert_memory(&self, record: &MemorySaveRecord) -> Result<MemoryRecord, AppError> {
        Ok(sqlx::query_as::<_, MemoryRecord>(
            r#"
INSERT INTO ai_memory (
    id, tenant_id, scope_type, scope_id, owner_user_id, source_kind, source_id,
    content, summary, sensitivity, write_policy, ttl_days, expires_at, metadata,
    status, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6, $7,
    $8, $9, $10, $11, $12, $13, $14,
    $15, $16, $17
)
ON CONFLICT (id)
DO UPDATE SET
    scope_type = EXCLUDED.scope_type,
    scope_id = EXCLUDED.scope_id,
    owner_user_id = EXCLUDED.owner_user_id,
    source_kind = EXCLUDED.source_kind,
    source_id = EXCLUDED.source_id,
    content = EXCLUDED.content,
    summary = EXCLUDED.summary,
    sensitivity = EXCLUDED.sensitivity,
    write_policy = EXCLUDED.write_policy,
    ttl_days = EXCLUDED.ttl_days,
    expires_at = EXCLUDED.expires_at,
    metadata = EXCLUDED.metadata,
    status = EXCLUDED.status,
    deleted_at = NULL,
    update_user = $16,
    update_time = $17
RETURNING
    id,
    scope_type,
    scope_id,
    source_kind,
    source_id,
    content,
    summary,
    sensitivity,
    write_policy,
    ttl_days,
    expires_at,
    metadata,
    status,
    create_time,
    update_time;
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(&record.scope_type)
        .bind(&record.scope_id)
        .bind(record.owner_user_id)
        .bind(&record.source_kind)
        .bind(&record.source_id)
        .bind(&record.content)
        .bind(&record.summary)
        .bind(&record.sensitivity)
        .bind(&record.write_policy)
        .bind(record.ttl_days)
        .bind(record.expires_at)
        .bind(&record.metadata)
        .bind(record.status)
        .bind(record.user_id)
        .bind(record.now)
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn soft_delete_memory(
        &self,
        tenant_id: i64,
        memory_id: i64,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<bool, AppError> {
        let affected = sqlx::query(
            r#"
UPDATE ai_memory
SET status = 0, deleted_at = $4, update_user = $3, update_time = $4
WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL;
"#,
        )
        .bind(tenant_id)
        .bind(memory_id)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?
        .rows_affected();

        Ok(affected > 0)
    }
}

fn push_memory_filters(query: &mut QueryBuilder<'_, Postgres>, filter: &MemoryFilter<'_>) {
    query
        .push(" AND m.tenant_id = ")
        .push_bind(filter.tenant_id)
        .push(
            " AND m.deleted_at IS NULL AND m.status = 1 AND (m.expires_at IS NULL OR m.expires_at > NOW())",
        );
    if let Some(scope_type) = non_empty(filter.scope_type) {
        query
            .push(" AND m.scope_type = ")
            .push_bind(scope_type.to_owned());
    }
    if let Some(scope_id) = non_empty(filter.scope_id) {
        query
            .push(" AND m.scope_id = ")
            .push_bind(scope_id.to_owned());
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Execute;

    #[test]
    fn memory_migration_defines_policy_entry_and_retention_contract() {
        let migration = include_str!("../../../migrations/202606060008_create_ai_memory.sql");

        assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_memory_policy"));
        assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_memory"));
        assert!(migration.contains("scope_type"));
        assert!(migration.contains("write_policy"));
        assert!(migration.contains("ttl_days"));
        assert!(migration.contains("expires_at"));
        assert!(migration.contains("redaction_rules"));
        assert!(migration.contains("deleted_at"));
    }

    #[test]
    fn memory_list_filters_exclude_disabled_and_expired_entries() {
        let mut query =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ai_memory AS m WHERE 1 = 1");
        push_memory_filters(
            &mut query,
            &MemoryFilter {
                tenant_id: 1,
                scope_type: None,
                scope_id: None,
                limit: 20,
                offset: 0,
            },
        );

        let sql = query.build().sql().to_owned();

        assert!(sql.contains("m.status = 1"));
        assert!(sql.contains("m.expires_at IS NULL OR m.expires_at > NOW()"));
    }
}
