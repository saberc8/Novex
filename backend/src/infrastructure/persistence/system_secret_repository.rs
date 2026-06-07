use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};

use crate::shared::error::AppError;

#[derive(Debug, Clone)]
pub struct SystemSecretRepository {
    db: PgPool,
}

#[derive(Debug, Clone)]
pub struct SecretFilter<'a> {
    pub tenant_id: i64,
    pub scope_type: Option<&'a str>,
    pub scope_id: Option<&'a str>,
    pub code: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct SecretSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub scope_type: String,
    pub scope_id: String,
    pub code: String,
    pub key_version: i32,
    pub ciphertext: String,
    pub masked_value: String,
    pub metadata: Value,
    pub status: i16,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct SecretRecord {
    pub id: i64,
    pub scope_type: String,
    pub scope_id: String,
    pub code: String,
    pub key_version: i32,
    pub masked_value: String,
    pub expires_at: Option<NaiveDateTime>,
    pub rotated_at: Option<NaiveDateTime>,
    pub last_used_at: Option<NaiveDateTime>,
    pub metadata: Value,
    pub status: i16,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

impl SystemSecretRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn count(&self, filter: &SecretFilter<'_>) -> Result<i64, AppError> {
        let mut query = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM sys_secret AS s");
        query.push(" WHERE 1 = 1");
        push_secret_filters(&mut query, filter);

        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list(&self, filter: &SecretFilter<'_>) -> Result<Vec<SecretRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
SELECT
    s.id,
    s.scope_type,
    s.scope_id,
    s.code,
    s.key_version,
    s.masked_value,
    s.expires_at,
    s.rotated_at,
    s.last_used_at,
    s.metadata,
    s.status,
    s.create_time,
    s.update_time
FROM sys_secret AS s
"#,
        );
        query.push(" WHERE 1 = 1");
        push_secret_filters(&mut query, filter);
        query
            .push(" ORDER BY s.code ASC, s.key_version DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);

        Ok(query
            .build_query_as::<SecretRecord>()
            .fetch_all(&self.db)
            .await?)
    }

    pub async fn latest_key_version(
        &self,
        tenant_id: i64,
        scope_type: &str,
        scope_id: &str,
        code: &str,
    ) -> Result<i32, AppError> {
        Ok(sqlx::query_scalar::<_, Option<i32>>(
            r#"
SELECT MAX(key_version)
FROM sys_secret
WHERE tenant_id = $1 AND scope_type = $2 AND scope_id = $3 AND code = $4;
"#,
        )
        .bind(tenant_id)
        .bind(scope_type)
        .bind(scope_id)
        .bind(code)
        .fetch_one(&self.db)
        .await?
        .unwrap_or(0))
    }

    pub async fn create_version(
        &self,
        record: &SecretSaveRecord,
    ) -> Result<SecretRecord, AppError> {
        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
UPDATE sys_secret
SET status = 0, update_user = $5, update_time = $6
WHERE tenant_id = $1 AND scope_type = $2 AND scope_id = $3 AND code = $4 AND status = 1;
"#,
        )
        .bind(record.tenant_id)
        .bind(&record.scope_type)
        .bind(&record.scope_id)
        .bind(&record.code)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&mut *tx)
        .await?;

        let inserted = sqlx::query_as::<_, SecretRecord>(
            r#"
INSERT INTO sys_secret (
    id, tenant_id, scope_type, scope_id, code, key_version, ciphertext,
    masked_value, rotated_at, metadata, status, create_user, create_time, update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $12, $9, $10, $11, $12, $11, $12)
RETURNING id, scope_type, scope_id, code, key_version, masked_value, expires_at,
          rotated_at, last_used_at, metadata, status, create_time, update_time;
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(&record.scope_type)
        .bind(&record.scope_id)
        .bind(&record.code)
        .bind(record.key_version)
        .bind(&record.ciphertext)
        .bind(&record.masked_value)
        .bind(&record.metadata)
        .bind(record.status)
        .bind(record.user_id)
        .bind(record.now)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(inserted)
    }
}

fn push_secret_filters(query: &mut QueryBuilder<'_, Postgres>, filter: &SecretFilter<'_>) {
    query
        .push(" AND s.tenant_id = ")
        .push_bind(filter.tenant_id);
    if let Some(scope_type) = non_empty(filter.scope_type) {
        query
            .push(" AND s.scope_type = ")
            .push_bind(scope_type.to_owned());
    }
    if let Some(scope_id) = non_empty(filter.scope_id) {
        query
            .push(" AND s.scope_id = ")
            .push_bind(scope_id.to_owned());
    }
    if let Some(code) = non_empty(filter.code) {
        query.push(" AND s.code = ").push_bind(code.to_owned());
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
