use chrono::NaiveDateTime;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};

use crate::shared::error::AppError;

#[derive(Debug, Clone)]
pub struct AiKnowledgeRepository {
    db: PgPool,
}

#[derive(Debug, Clone, FromRow)]
pub struct DatasetRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub name: String,
    pub description: String,
    pub owner_id: i64,
    pub visibility: i16,
    pub status: i16,
    pub retrieval_mode: i16,
    pub document_count: i32,
    pub chunk_count: i32,
    pub create_time: NaiveDateTime,
    pub create_user_string: String,
    pub update_time: Option<NaiveDateTime>,
    pub update_user_string: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct DocumentRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub name: String,
    pub source_uri: String,
    pub file_id: Option<i64>,
    pub content_type: String,
    pub owner_id: i64,
    pub visibility: i16,
    pub parse_status: i16,
    pub ingestion_status: i16,
    pub chunk_count: i32,
    pub source_hash: String,
    pub create_time: NaiveDateTime,
    pub create_user_string: String,
    pub update_time: Option<NaiveDateTime>,
    pub update_user_string: String,
}

#[derive(Debug, Clone)]
pub struct DatasetFilter<'a> {
    pub tenant_id: i64,
    pub name: Option<&'a str>,
    pub status: Option<i16>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct DocumentFilter {
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct DatasetSaveRecord<'a> {
    pub id: i64,
    pub tenant_id: i64,
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub owner_id: i64,
    pub visibility: i16,
    pub status: i16,
    pub retrieval_mode: i16,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

impl AiKnowledgeRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn count_datasets(&self, filter: &DatasetFilter<'_>) -> Result<i64, AppError> {
        let mut query = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ai_dataset AS d");
        query.push(" WHERE 1 = 1");
        push_dataset_filters(&mut query, filter);
        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_datasets(
        &self,
        filter: &DatasetFilter<'_>,
    ) -> Result<Vec<DatasetRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(dataset_select_sql());
        query.push(" WHERE 1 = 1");
        push_dataset_filters(&mut query, filter);
        query
            .push(" ORDER BY d.create_time DESC, d.id DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);
        Ok(query
            .build_query_as::<DatasetRecord>()
            .fetch_all(&self.db)
            .await?)
    }

    pub async fn create_dataset(&self, record: &DatasetSaveRecord<'_>) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_dataset (
    id, tenant_id, name, description, owner_id, visibility, status, retrieval_mode,
    create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.name)
        .bind(record.description)
        .bind(record.owner_id)
        .bind(record.visibility)
        .bind(record.status)
        .bind(record.retrieval_mode)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn dataset_exists(&self, tenant_id: i64, dataset_id: i64) -> Result<bool, AppError> {
        Ok(sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM ai_dataset WHERE tenant_id = $1 AND id = $2);",
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn count_documents(&self, filter: &DocumentFilter) -> Result<i64, AppError> {
        let mut query = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ai_document AS d");
        query.push(" WHERE 1 = 1");
        push_document_filters(&mut query, filter);
        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_documents(
        &self,
        filter: &DocumentFilter,
    ) -> Result<Vec<DocumentRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(document_select_sql());
        query.push(" WHERE 1 = 1");
        push_document_filters(&mut query, filter);
        query
            .push(" ORDER BY d.create_time DESC, d.id DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);
        Ok(query
            .build_query_as::<DocumentRecord>()
            .fetch_all(&self.db)
            .await?)
    }
}

fn push_dataset_filters(query: &mut QueryBuilder<'_, Postgres>, filter: &DatasetFilter<'_>) {
    query
        .push(" AND d.tenant_id = ")
        .push_bind(filter.tenant_id);
    if let Some(name) = non_empty(filter.name) {
        query
            .push(" AND d.name ILIKE ")
            .push_bind(format!("%{name}%"));
    }
    if let Some(status) = filter.status.filter(|value| *value > 0) {
        query.push(" AND d.status = ").push_bind(status);
    }
}

fn push_document_filters(query: &mut QueryBuilder<'_, Postgres>, filter: &DocumentFilter) {
    query
        .push(" AND d.tenant_id = ")
        .push_bind(filter.tenant_id)
        .push(" AND d.dataset_id = ")
        .push_bind(filter.dataset_id);
}

fn dataset_select_sql() -> &'static str {
    r#"
SELECT
    d.id,
    d.tenant_id,
    d.name,
    COALESCE(d.description, '') AS description,
    d.owner_id,
    d.visibility,
    d.status,
    d.retrieval_mode,
    d.document_count,
    d.chunk_count,
    d.create_time,
    COALESCE(cu.nickname, '') AS create_user_string,
    d.update_time,
    COALESCE(uu.nickname, '') AS update_user_string
FROM ai_dataset AS d
LEFT JOIN sys_user AS cu ON cu.id = d.create_user
LEFT JOIN sys_user AS uu ON uu.id = d.update_user
"#
}

fn document_select_sql() -> &'static str {
    r#"
SELECT
    d.id,
    d.tenant_id,
    d.dataset_id,
    d.name,
    COALESCE(d.source_uri, '') AS source_uri,
    d.file_id,
    COALESCE(d.content_type, '') AS content_type,
    d.owner_id,
    d.visibility,
    d.parse_status,
    d.ingestion_status,
    d.chunk_count,
    COALESCE(d.source_hash, '') AS source_hash,
    d.create_time,
    COALESCE(cu.nickname, '') AS create_user_string,
    d.update_time,
    COALESCE(uu.nickname, '') AS update_user_string
FROM ai_document AS d
LEFT JOIN sys_user AS cu ON cu.id = d.create_user
LEFT JOIN sys_user AS uu ON uu.id = d.update_user
"#
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
