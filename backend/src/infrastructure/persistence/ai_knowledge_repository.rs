use chrono::NaiveDateTime;
use serde_json::Value;
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

#[derive(Debug, Clone, FromRow)]
pub struct ChunkRecord {
    pub id: i64,
    pub document_id: i64,
    pub chunk_uid: String,
    pub chunk_index: i32,
    pub content: String,
    pub semantic_search_text: String,
    pub token_count: i32,
    pub citation: Value,
    pub segment_type: String,
    pub segment_index: i32,
    pub page_no: Option<i32>,
    pub section_path: Value,
    pub content_role: String,
    pub display_capability: String,
    pub metadata: Value,
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

#[derive(Debug, Clone)]
pub struct DocumentSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub name: String,
    pub source_uri: Option<String>,
    pub file_id: Option<i64>,
    pub content_type: Option<String>,
    pub owner_id: i64,
    pub visibility: i16,
    pub parse_status: i16,
    pub ingestion_status: i16,
    pub chunk_count: i32,
    pub source_hash: Option<String>,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct ParserJobSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub document_id: i64,
    pub job_type: i16,
    pub status: i16,
    pub result_summary: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct BlockSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub document_id: i64,
    pub block_uid: String,
    pub block_index: i32,
    pub block_type: String,
    pub text: String,
    pub page_no: Option<i32>,
    pub section_path: Value,
    pub bbox: Value,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct ChunkSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub document_id: i64,
    pub chunk_uid: String,
    pub chunk_index: i32,
    pub content: String,
    pub semantic_search_text: String,
    pub token_count: i32,
    pub citation: Value,
    pub segment_type: String,
    pub segment_index: i32,
    pub page_no: Option<i32>,
    pub section_path: Value,
    pub content_role: String,
    pub display_capability: String,
    pub metadata: Value,
    pub embedding_status: i16,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct RagTraceSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub question: String,
    pub answer: String,
    pub answer_strategy: String,
    pub retrieval_mode: i16,
    pub embedding_model_route: Option<String>,
    pub rerank_model_route: Option<String>,
    pub answer_model_route: Option<String>,
    pub retrieval_hit_count: i32,
    pub context_token_count: i32,
    pub output_token_count: i32,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct RagTraceHitSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub trace_id: i64,
    pub dataset_id: i64,
    pub document_id: i64,
    pub chunk_id: i64,
    pub rank: i32,
    pub score: f32,
    pub citation: Value,
    pub content_preview: String,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct FeedbackSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub resource_type: String,
    pub resource_id: String,
    pub trace_id: Option<String>,
    pub rating: String,
    pub reason: String,
    pub metadata: Value,
    pub status: i16,
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

    pub async fn create_document_ingestion(
        &self,
        document: &DocumentSaveRecord,
        parser_job: &ParserJobSaveRecord,
        blocks: &[BlockSaveRecord],
        chunks: &[ChunkSaveRecord],
    ) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
INSERT INTO ai_document (
    id, tenant_id, dataset_id, name, source_uri, file_id, content_type, owner_id,
    visibility, parse_status, ingestion_status, chunk_count, source_hash, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15);
"#,
        )
        .bind(document.id)
        .bind(document.tenant_id)
        .bind(document.dataset_id)
        .bind(&document.name)
        .bind(&document.source_uri)
        .bind(document.file_id)
        .bind(&document.content_type)
        .bind(document.owner_id)
        .bind(document.visibility)
        .bind(document.parse_status)
        .bind(document.ingestion_status)
        .bind(document.chunk_count)
        .bind(&document.source_hash)
        .bind(document.user_id)
        .bind(document.now)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
INSERT INTO ai_parser_job (
    id, tenant_id, dataset_id, document_id, job_type, status, result_summary, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9);
"#,
        )
        .bind(parser_job.id)
        .bind(parser_job.tenant_id)
        .bind(parser_job.dataset_id)
        .bind(parser_job.document_id)
        .bind(parser_job.job_type)
        .bind(parser_job.status)
        .bind(&parser_job.result_summary)
        .bind(parser_job.user_id)
        .bind(parser_job.now)
        .execute(&mut *tx)
        .await?;

        for block in blocks {
            sqlx::query(
                r#"
INSERT INTO ai_document_block (
    id, tenant_id, dataset_id, document_id, block_uid, block_index, block_type,
    text, page_no, section_path, bbox, metadata, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14);
"#,
            )
            .bind(block.id)
            .bind(block.tenant_id)
            .bind(block.dataset_id)
            .bind(block.document_id)
            .bind(&block.block_uid)
            .bind(block.block_index)
            .bind(&block.block_type)
            .bind(&block.text)
            .bind(block.page_no)
            .bind(&block.section_path)
            .bind(&block.bbox)
            .bind(&block.metadata)
            .bind(block.user_id)
            .bind(block.now)
            .execute(&mut *tx)
            .await?;
        }

        for chunk in chunks {
            sqlx::query(
                r#"
INSERT INTO ai_document_chunk (
    id, tenant_id, dataset_id, document_id, chunk_uid, chunk_index, content,
    semantic_search_text, token_count, citation, segment_type, segment_index, page_no,
    section_path, content_role, display_capability, metadata, embedding_status,
    create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
    $11, $12, $13, $14, $15, $16, $17, $18, $19, $20
);
"#,
            )
            .bind(chunk.id)
            .bind(chunk.tenant_id)
            .bind(chunk.dataset_id)
            .bind(chunk.document_id)
            .bind(&chunk.chunk_uid)
            .bind(chunk.chunk_index)
            .bind(&chunk.content)
            .bind(&chunk.semantic_search_text)
            .bind(chunk.token_count)
            .bind(&chunk.citation)
            .bind(&chunk.segment_type)
            .bind(chunk.segment_index)
            .bind(chunk.page_no)
            .bind(&chunk.section_path)
            .bind(&chunk.content_role)
            .bind(&chunk.display_capability)
            .bind(&chunk.metadata)
            .bind(chunk.embedding_status)
            .bind(chunk.user_id)
            .bind(chunk.now)
            .execute(&mut *tx)
            .await?;
        }

        let result = sqlx::query(
            r#"
UPDATE ai_dataset
SET document_count = document_count + 1,
    chunk_count = chunk_count + $1,
    update_user = $2,
    update_time = $3
WHERE tenant_id = $4 AND id = $5;
"#,
        )
        .bind(document.chunk_count)
        .bind(document.user_id)
        .bind(document.now)
        .bind(document.tenant_id)
        .bind(document.dataset_id)
        .execute(&mut *tx)
        .await?;
        ensure_affected(result.rows_affected())?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_indexed_chunks(
        &self,
        tenant_id: i64,
        dataset_id: i64,
        limit: i64,
    ) -> Result<Vec<ChunkRecord>, AppError> {
        Ok(sqlx::query_as::<_, ChunkRecord>(
            r#"
SELECT
    id,
    document_id,
    chunk_uid,
    chunk_index,
    content,
    semantic_search_text,
    token_count,
    citation,
    segment_type,
    segment_index,
    page_no,
    section_path,
    content_role,
    display_capability,
    metadata
FROM ai_document_chunk
WHERE tenant_id = $1
  AND dataset_id = $2
  AND embedding_status = 4
ORDER BY document_id ASC, chunk_index ASC
LIMIT $3;
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .bind(limit)
        .fetch_all(&self.db)
        .await?)
    }

    pub async fn create_rag_trace(
        &self,
        trace: &RagTraceSaveRecord,
        hits: &[RagTraceHitSaveRecord],
    ) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
INSERT INTO ai_rag_trace (
    id, tenant_id, dataset_id, question, answer, answer_strategy, retrieval_mode,
    embedding_model_route, rerank_model_route, answer_model_route,
    retrieval_hit_count, context_token_count, output_token_count, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15);
"#,
        )
        .bind(trace.id)
        .bind(trace.tenant_id)
        .bind(trace.dataset_id)
        .bind(&trace.question)
        .bind(&trace.answer)
        .bind(&trace.answer_strategy)
        .bind(trace.retrieval_mode)
        .bind(&trace.embedding_model_route)
        .bind(&trace.rerank_model_route)
        .bind(&trace.answer_model_route)
        .bind(trace.retrieval_hit_count)
        .bind(trace.context_token_count)
        .bind(trace.output_token_count)
        .bind(trace.user_id)
        .bind(trace.now)
        .execute(&mut *tx)
        .await?;

        for hit in hits {
            sqlx::query(
                r#"
INSERT INTO ai_rag_trace_hit (
    id, tenant_id, trace_id, dataset_id, document_id, chunk_id, rank, score,
    citation, content_preview, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11);
"#,
            )
            .bind(hit.id)
            .bind(hit.tenant_id)
            .bind(hit.trace_id)
            .bind(hit.dataset_id)
            .bind(hit.document_id)
            .bind(hit.chunk_id)
            .bind(hit.rank)
            .bind(hit.score)
            .bind(&hit.citation)
            .bind(&hit.content_preview)
            .bind(hit.now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn create_feedback(&self, record: &FeedbackSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_feedback (
    id, tenant_id, resource_type, resource_id, trace_id, rating, reason,
    metadata, status, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(&record.resource_type)
        .bind(&record.resource_id)
        .bind(&record.trace_id)
        .bind(&record.rating)
        .bind(&record.reason)
        .bind(&record.metadata)
        .bind(record.status)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
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

fn ensure_affected(rows_affected: u64) -> Result<(), AppError> {
    if rows_affected == 0 {
        Err(AppError::NotFound)
    } else {
        Ok(())
    }
}
