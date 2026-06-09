use chrono::NaiveDateTime;
use serde_json::{json, Value};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};

use crate::shared::{error::AppError, id::next_id};

const DEFAULT_VECTOR_BACKEND: &str = "milvus";
const DEFAULT_EMBEDDING_MODEL_ROUTE: &str = "local-keyword";
const DEFAULT_VECTOR_DIMENSION: i32 = 64;
const VECTOR_COLLECTION_STATUS_READY: i16 = 1;

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

#[derive(Debug, Clone, FromRow)]
pub struct VectorCollectionRecord {
    pub id: i64,
    pub vector_backend: String,
    pub provider_collection: String,
    pub dimension: i32,
    pub metric_type: String,
    pub status: i16,
}

#[derive(Debug, Clone, FromRow)]
pub struct ParserJobRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub document_id: i64,
    pub job_type: i16,
    pub status: i16,
    pub attempt_count: i32,
    pub error_message: String,
    pub result_summary: Value,
    pub document_name: String,
    pub source_uri: String,
    pub file_id: Option<i64>,
    pub content_type: String,
    pub parse_status: i16,
    pub ingestion_status: i16,
    pub chunk_count: i32,
    pub create_time: NaiveDateTime,
    pub create_user_string: String,
    pub update_time: Option<NaiveDateTime>,
    pub update_user_string: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct ParserOutboxRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub document_id: i64,
    pub parser_job_id: i64,
    pub event_type: String,
    pub payload: Value,
    pub status: i16,
    pub attempt_count: i32,
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
pub struct DatasetAccessFilter<'a> {
    pub tenant_id: i64,
    pub user_id: i64,
    pub role_ids: &'a [i64],
    pub is_admin: bool,
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
pub struct ParserJobFilter {
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub job_id: i64,
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
pub struct ParserOutboxSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub document_id: i64,
    pub parser_job_id: i64,
    pub event_type: String,
    pub payload: Value,
    pub status: i16,
    pub attempt_count: i32,
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
    pub embedding_model: Option<String>,
    pub embedding_ref: Option<String>,
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

#[derive(Debug, Clone, FromRow)]
pub struct TrainingLearningSummaryRecord {
    pub rag_trace_count: i64,
    pub feedback_count: i64,
    pub weak_signal_count: i64,
    pub quiz_wrong_count: i64,
    pub latest_eval_average_score: Option<f64>,
    pub latest_eval_total_cases: Option<i32>,
    pub latest_eval_passed_cases: Option<i32>,
}

#[derive(Debug, Clone, FromRow)]
pub struct TrainingLearningActivityRecord {
    pub id: i64,
    pub learner_id: i64,
    pub learner_name: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub status: String,
    pub score: Option<f64>,
    pub create_time: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct TrainingWeakPointRecord {
    pub topic: String,
    pub evidence: String,
    pub count: i64,
    pub last_seen_at: NaiveDateTime,
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

    pub async fn count_accessible_datasets(
        &self,
        filter: &DatasetAccessFilter<'_>,
    ) -> Result<i64, AppError> {
        let mut query = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ai_dataset AS d");
        query.push(" WHERE 1 = 1");
        push_dataset_access_filters(&mut query, filter);
        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_accessible_datasets(
        &self,
        filter: &DatasetAccessFilter<'_>,
    ) -> Result<Vec<DatasetRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(dataset_select_sql());
        query.push(" WHERE 1 = 1");
        push_dataset_access_filters(&mut query, filter);
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

    pub async fn dataset_readable(
        &self,
        filter: &DatasetAccessFilter<'_>,
        dataset_id: i64,
    ) -> Result<bool, AppError> {
        let mut query =
            QueryBuilder::<Postgres>::new("SELECT EXISTS(SELECT 1 FROM ai_dataset AS d");
        query.push(" WHERE 1 = 1");
        push_dataset_access_filters(&mut query, filter);
        query.push(" AND d.id = ").push_bind(dataset_id).push(")");
        Ok(query
            .build_query_scalar::<bool>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn create_dataset(&self, record: &DatasetSaveRecord<'_>) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;
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
        .execute(&mut *tx)
        .await?;
        insert_dataset_vector_collection(&mut tx, record).await?;
        tx.commit().await?;
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

    pub async fn delete_dataset_cascade(
        &self,
        tenant_id: i64,
        dataset_id: i64,
    ) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;

        sqlx::query(
            r#"
DELETE FROM ai_feedback AS f
USING ai_rag_trace AS t
WHERE f.tenant_id = $1
  AND t.tenant_id = $1
  AND t.dataset_id = $2
  AND (f.trace_id = t.id::TEXT OR (f.resource_type = 'rag_trace' AND f.resource_id = t.id::TEXT));
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
DELETE FROM ai_chat_flow_message AS m
WHERE m.tenant_id = $1
  AND (
      EXISTS (
          SELECT 1
          FROM ai_chat_flow_session AS s
          WHERE s.tenant_id = m.tenant_id
            AND s.id = m.session_id
            AND s.dataset_id = $2
      )
      OR EXISTS (
          SELECT 1
          FROM ai_rag_trace AS t
          WHERE t.tenant_id = m.tenant_id
            AND t.id = m.rag_trace_id
            AND t.dataset_id = $2
      )
  );
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
DELETE FROM ai_chat_flow_session
WHERE tenant_id = $1 AND dataset_id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
DELETE FROM ai_rag_trace_hit
WHERE tenant_id = $1 AND dataset_id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
DELETE FROM ai_rag_trace
WHERE tenant_id = $1 AND dataset_id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .execute(&mut *tx)
        .await?;

        if parser_outbox_table_exists(&mut tx).await? {
            sqlx::query(
                r#"
DELETE FROM ai_parser_outbox
WHERE tenant_id = $1 AND dataset_id = $2;
"#,
            )
            .bind(tenant_id)
            .bind(dataset_id)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            r#"
DELETE FROM ai_parser_job
WHERE tenant_id = $1 AND dataset_id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
DELETE FROM ai_embedding
WHERE tenant_id = $1 AND dataset_id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
DELETE FROM ai_vector_collection
WHERE tenant_id = $1 AND dataset_id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
DELETE FROM ai_document_block
WHERE tenant_id = $1 AND dataset_id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
DELETE FROM ai_document_chunk
WHERE tenant_id = $1 AND dataset_id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
DELETE FROM ai_document
WHERE tenant_id = $1 AND dataset_id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
DELETE FROM sys_resource_permission
WHERE tenant_id = $1
  AND resource_type = 'ai_dataset'
  AND resource_id = $2::TEXT;
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .execute(&mut *tx)
        .await?;

        let result = sqlx::query(
            r#"
DELETE FROM ai_dataset
WHERE tenant_id = $1 AND id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .execute(&mut *tx)
        .await?;
        ensure_affected(result.rows_affected())?;

        tx.commit().await?;
        Ok(())
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
        ensure_dataset_vector_collection(
            &mut tx,
            document.tenant_id,
            document.dataset_id,
            None,
            document.user_id,
            document.now,
        )
        .await?;
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
    section_path, content_role, display_capability, metadata, embedding_model, embedding_ref,
    embedding_status, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
    $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22
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
            .bind(&chunk.embedding_model)
            .bind(&chunk.embedding_ref)
            .bind(chunk.embedding_status)
            .bind(chunk.user_id)
            .bind(chunk.now)
            .execute(&mut *tx)
            .await?;
            insert_chunk_embedding(&mut tx, chunk).await?;
        }

        update_vector_collection_embedding_shape(
            &mut tx,
            document.tenant_id,
            document.dataset_id,
            chunks,
        )
        .await?;

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

    pub async fn create_document_parse_job(
        &self,
        document: &DocumentSaveRecord,
        parser_job: &ParserJobSaveRecord,
        parser_outbox: &ParserOutboxSaveRecord,
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

        sqlx::query(
            r#"
INSERT INTO ai_parser_outbox (
    id, tenant_id, dataset_id, document_id, parser_job_id, event_type, payload,
    status, attempt_count, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
ON CONFLICT (tenant_id, parser_job_id, event_type) DO UPDATE
SET payload = EXCLUDED.payload,
    status = EXCLUDED.status,
    attempt_count = EXCLUDED.attempt_count,
    last_error = NULL,
    published_time = NULL,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
        )
        .bind(parser_outbox.id)
        .bind(parser_outbox.tenant_id)
        .bind(parser_outbox.dataset_id)
        .bind(parser_outbox.document_id)
        .bind(parser_outbox.parser_job_id)
        .bind(&parser_outbox.event_type)
        .bind(&parser_outbox.payload)
        .bind(parser_outbox.status)
        .bind(parser_outbox.attempt_count)
        .bind(parser_outbox.user_id)
        .bind(parser_outbox.now)
        .execute(&mut *tx)
        .await?;

        let result = sqlx::query(
            r#"
UPDATE ai_dataset
SET document_count = document_count + 1,
    update_user = $1,
    update_time = $2
WHERE tenant_id = $3 AND id = $4;
"#,
        )
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

    pub async fn complete_document_parse_job(
        &self,
        document: &DocumentSaveRecord,
        parser_job: &ParserJobSaveRecord,
        blocks: &[BlockSaveRecord],
        chunks: &[ChunkSaveRecord],
    ) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;
        ensure_dataset_vector_collection(
            &mut tx,
            document.tenant_id,
            document.dataset_id,
            None,
            document.user_id,
            document.now,
        )
        .await?;
        let result = sqlx::query(
            r#"
UPDATE ai_document
SET name = $1,
    content_type = $2,
    parse_status = $3,
    ingestion_status = $4,
    chunk_count = $5,
    source_hash = $6,
    update_user = $7,
    update_time = $8
WHERE tenant_id = $9 AND dataset_id = $10 AND id = $11;
"#,
        )
        .bind(&document.name)
        .bind(&document.content_type)
        .bind(document.parse_status)
        .bind(document.ingestion_status)
        .bind(document.chunk_count)
        .bind(&document.source_hash)
        .bind(document.user_id)
        .bind(document.now)
        .bind(document.tenant_id)
        .bind(document.dataset_id)
        .bind(document.id)
        .execute(&mut *tx)
        .await?;
        ensure_affected(result.rows_affected())?;

        let result = sqlx::query(
            r#"
UPDATE ai_parser_job
SET status = $1,
    result_summary = $2,
    update_user = $3,
    update_time = $4
WHERE tenant_id = $5 AND dataset_id = $6 AND document_id = $7 AND id = $8;
"#,
        )
        .bind(parser_job.status)
        .bind(&parser_job.result_summary)
        .bind(parser_job.user_id)
        .bind(parser_job.now)
        .bind(parser_job.tenant_id)
        .bind(parser_job.dataset_id)
        .bind(parser_job.document_id)
        .bind(parser_job.id)
        .execute(&mut *tx)
        .await?;
        ensure_affected(result.rows_affected())?;

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
    section_path, content_role, display_capability, metadata, embedding_model, embedding_ref,
    embedding_status, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
    $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22
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
            .bind(&chunk.embedding_model)
            .bind(&chunk.embedding_ref)
            .bind(chunk.embedding_status)
            .bind(chunk.user_id)
            .bind(chunk.now)
            .execute(&mut *tx)
            .await?;
            insert_chunk_embedding(&mut tx, chunk).await?;
        }

        let result = sqlx::query(
            r#"
UPDATE ai_dataset
SET chunk_count = chunk_count + $1,
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

    pub async fn update_parser_job_status(
        &self,
        parser_job: &ParserJobSaveRecord,
        document_parse_status: i16,
        document_ingestion_status: i16,
        error_message: Option<&str>,
    ) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;
        let result = sqlx::query(
            r#"
UPDATE ai_document
SET parse_status = $1,
    ingestion_status = $2,
    update_user = $3,
    update_time = $4
WHERE tenant_id = $5 AND dataset_id = $6 AND id = $7;
"#,
        )
        .bind(document_parse_status)
        .bind(document_ingestion_status)
        .bind(parser_job.user_id)
        .bind(parser_job.now)
        .bind(parser_job.tenant_id)
        .bind(parser_job.dataset_id)
        .bind(parser_job.document_id)
        .execute(&mut *tx)
        .await?;
        ensure_affected(result.rows_affected())?;

        let result = sqlx::query(
            r#"
UPDATE ai_parser_job
SET status = $1,
    error_message = $2,
    result_summary = $3,
    update_user = $4,
    update_time = $5
WHERE tenant_id = $6 AND dataset_id = $7 AND document_id = $8 AND id = $9;
"#,
        )
        .bind(parser_job.status)
        .bind(error_message)
        .bind(&parser_job.result_summary)
        .bind(parser_job.user_id)
        .bind(parser_job.now)
        .bind(parser_job.tenant_id)
        .bind(parser_job.dataset_id)
        .bind(parser_job.document_id)
        .bind(parser_job.id)
        .execute(&mut *tx)
        .await?;
        ensure_affected(result.rows_affected())?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn parser_job_exists(
        &self,
        tenant_id: i64,
        dataset_id: i64,
        document_id: i64,
        job_id: i64,
    ) -> Result<bool, AppError> {
        Ok(sqlx::query_scalar::<_, bool>(
            r#"
SELECT EXISTS(
    SELECT 1
    FROM ai_parser_job
    WHERE tenant_id = $1 AND dataset_id = $2 AND document_id = $3 AND id = $4
);
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .bind(document_id)
        .bind(job_id)
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn get_parser_job(
        &self,
        filter: &ParserJobFilter,
    ) -> Result<Option<ParserJobRecord>, AppError> {
        Ok(
            sqlx::query_as::<_, ParserJobRecord>(parser_job_select_sql())
                .bind(filter.tenant_id)
                .bind(filter.dataset_id)
                .bind(filter.job_id)
                .fetch_optional(&self.db)
                .await?,
        )
    }

    pub async fn list_pending_parser_outbox(
        &self,
        limit: i64,
    ) -> Result<Vec<ParserOutboxRecord>, AppError> {
        Ok(sqlx::query_as::<_, ParserOutboxRecord>(
            r#"
SELECT
    id,
    tenant_id,
    dataset_id,
    document_id,
    parser_job_id,
    event_type,
    payload,
    status,
    attempt_count
FROM ai_parser_outbox
WHERE status = 1
ORDER BY create_time ASC, id ASC
LIMIT $1;
"#,
        )
        .bind(limit.max(1))
        .fetch_all(&self.db)
        .await?)
    }

    pub async fn mark_parser_outbox_published(
        &self,
        outbox_id: i64,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        let result = sqlx::query(
            r#"
UPDATE ai_parser_outbox
SET status = 2,
    published_time = $1,
    last_error = NULL,
    update_user = $2,
    update_time = $1
WHERE id = $3;
"#,
        )
        .bind(now)
        .bind(user_id)
        .bind(outbox_id)
        .execute(&self.db)
        .await?;
        ensure_affected(result.rows_affected())?;
        Ok(())
    }

    pub async fn mark_parser_outbox_publish_failed(
        &self,
        outbox_id: i64,
        error: &str,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        let result = sqlx::query(
            r#"
UPDATE ai_parser_outbox
SET status = 1,
    attempt_count = attempt_count + 1,
    last_error = $1,
    update_user = $2,
    update_time = $3
WHERE id = $4;
"#,
        )
        .bind(error)
        .bind(user_id)
        .bind(now)
        .bind(outbox_id)
        .execute(&self.db)
        .await?;
        ensure_affected(result.rows_affected())?;
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

    pub async fn get_vector_collection(
        &self,
        tenant_id: i64,
        dataset_id: i64,
    ) -> Result<Option<VectorCollectionRecord>, AppError> {
        Ok(sqlx::query_as::<_, VectorCollectionRecord>(
            r#"
SELECT
    id,
    vector_backend,
    provider_collection,
    dimension,
    metric_type,
    status
FROM ai_vector_collection
WHERE tenant_id = $1
  AND dataset_id = $2
ORDER BY create_time DESC
LIMIT 1;
"#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .fetch_optional(&self.db)
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

    pub async fn training_learning_summary(
        &self,
        tenant_id: i64,
        user_id: Option<i64>,
    ) -> Result<TrainingLearningSummaryRecord, AppError> {
        Ok(sqlx::query_as::<_, TrainingLearningSummaryRecord>(
            r#"
SELECT
    (
        SELECT COUNT(*)
        FROM ai_rag_trace AS t
        WHERE t.tenant_id = $1
          AND ($2::BIGINT IS NULL OR t.create_user = $2)
    ) AS rag_trace_count,
    (
        SELECT COUNT(*)
        FROM ai_feedback AS f
        WHERE f.tenant_id = $1
          AND ($2::BIGINT IS NULL OR f.create_user = $2)
    ) AS feedback_count,
    (
        SELECT COUNT(*)
        FROM ai_feedback AS f
        WHERE f.tenant_id = $1
          AND ($2::BIGINT IS NULL OR f.create_user = $2)
          AND f.rating IN ('quiz_wrong_answer', 'not_helpful', 'citation_issue')
    ) AS weak_signal_count,
    (
        SELECT COUNT(*)
        FROM ai_feedback AS f
        WHERE f.tenant_id = $1
          AND ($2::BIGINT IS NULL OR f.create_user = $2)
          AND f.rating = 'quiz_wrong_answer'
    ) AS quiz_wrong_count,
    (
        SELECT r.average_score
        FROM ai_eval_run AS r
        WHERE r.tenant_id = $1
          AND ($2::BIGINT IS NULL OR r.create_user = $2)
          AND r.dataset_code = 'training_regression'
        ORDER BY r.create_time DESC, r.id DESC
        LIMIT 1
    ) AS latest_eval_average_score,
    (
        SELECT r.total_cases
        FROM ai_eval_run AS r
        WHERE r.tenant_id = $1
          AND ($2::BIGINT IS NULL OR r.create_user = $2)
          AND r.dataset_code = 'training_regression'
        ORDER BY r.create_time DESC, r.id DESC
        LIMIT 1
    ) AS latest_eval_total_cases,
    (
        SELECT r.passed_cases
        FROM ai_eval_run AS r
        WHERE r.tenant_id = $1
          AND ($2::BIGINT IS NULL OR r.create_user = $2)
          AND r.dataset_code = 'training_regression'
        ORDER BY r.create_time DESC, r.id DESC
        LIMIT 1
    ) AS latest_eval_passed_cases;
"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn list_training_learning_activities(
        &self,
        tenant_id: i64,
        user_id: Option<i64>,
        limit: i64,
    ) -> Result<Vec<TrainingLearningActivityRecord>, AppError> {
        Ok(sqlx::query_as::<_, TrainingLearningActivityRecord>(
            r#"
SELECT
    activity.id,
    activity.learner_id,
    activity.learner_name,
    activity.kind,
    activity.title,
    activity.detail,
    activity.status,
    activity.score,
    activity.create_time
FROM (
    SELECT
        t.id,
        t.create_user AS learner_id,
        COALESCE(NULLIF(u.nickname, ''), NULLIF(u.username, ''), CONCAT('user-', t.create_user)) AS learner_name,
        'rag_ask'::TEXT AS kind,
        '知识库问答'::TEXT AS title,
        t.question AS detail,
        'completed'::TEXT AS status,
        NULL::DOUBLE PRECISION AS score,
        t.create_time
    FROM ai_rag_trace AS t
    LEFT JOIN sys_user AS u ON u.id = t.create_user
    WHERE t.tenant_id = $1
      AND ($2::BIGINT IS NULL OR t.create_user = $2)

    UNION ALL

    SELECT
        f.id,
        f.create_user AS learner_id,
        COALESCE(NULLIF(u.nickname, ''), NULLIF(u.username, ''), CONCAT('user-', f.create_user)) AS learner_name,
        CASE
            WHEN f.resource_type = 'training_quiz' THEN 'quiz_feedback'
            ELSE 'feedback'
        END AS kind,
        CASE
            WHEN f.resource_type = 'training_quiz' THEN '测验错题反馈'
            ELSE '问答反馈'
        END AS title,
        CONCAT(f.rating, CASE WHEN f.reason = '' THEN '' ELSE CONCAT(' · ', f.reason) END) AS detail,
        CASE
            WHEN f.rating IN ('quiz_wrong_answer', 'not_helpful', 'citation_issue') THEN 'needs_review'
            ELSE 'completed'
        END AS status,
        NULL::DOUBLE PRECISION AS score,
        f.create_time
    FROM ai_feedback AS f
    LEFT JOIN sys_user AS u ON u.id = f.create_user
    WHERE f.tenant_id = $1
      AND ($2::BIGINT IS NULL OR f.create_user = $2)

    UNION ALL

    SELECT
        r.id,
        r.create_user AS learner_id,
        COALESCE(NULLIF(u.nickname, ''), NULLIF(u.username, ''), CONCAT('user-', r.create_user)) AS learner_name,
        'eval_run'::TEXT AS kind,
        'Training Regression'::TEXT AS title,
        CONCAT(r.passed_cases, '/', r.total_cases, ' passed') AS detail,
        r.status,
        r.average_score AS score,
        r.create_time
    FROM ai_eval_run AS r
    LEFT JOIN sys_user AS u ON u.id = r.create_user
    WHERE r.tenant_id = $1
      AND ($2::BIGINT IS NULL OR r.create_user = $2)
      AND r.dataset_code = 'training_regression'
) AS activity
ORDER BY activity.create_time DESC, activity.id DESC
LIMIT $3;
"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.db)
        .await?)
    }

    pub async fn list_training_weak_points(
        &self,
        tenant_id: i64,
        user_id: Option<i64>,
        limit: i64,
    ) -> Result<Vec<TrainingWeakPointRecord>, AppError> {
        Ok(sqlx::query_as::<_, TrainingWeakPointRecord>(
            r#"
SELECT
    weak.topic,
    weak.evidence,
    COUNT(*) AS count,
    MAX(weak.create_time) AS last_seen_at
FROM (
    SELECT
        CASE
            WHEN f.rating = 'quiz_wrong_answer' THEN '客户数据外发与权限申请'
            WHEN f.rating = 'citation_issue' THEN '知识库引用定位'
            ELSE '培训资料理解'
        END AS topic,
        f.rating AS evidence,
        f.create_time
    FROM ai_feedback AS f
    WHERE f.tenant_id = $1
      AND ($2::BIGINT IS NULL OR f.create_user = $2)
      AND f.rating IN ('quiz_wrong_answer', 'not_helpful', 'citation_issue')

    UNION ALL

    SELECT
        '回归评测失败用例'::TEXT AS topic,
        CONCAT(r.dataset_code, ':failed') AS evidence,
        r.create_time
    FROM ai_eval_run AS r
    WHERE r.tenant_id = $1
      AND ($2::BIGINT IS NULL OR r.create_user = $2)
      AND r.dataset_code = 'training_regression'
      AND r.failed_cases > 0
) AS weak
GROUP BY weak.topic, weak.evidence
ORDER BY count DESC, last_seen_at DESC
LIMIT $3;
"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(limit)
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

fn push_dataset_access_filters(
    query: &mut QueryBuilder<'_, Postgres>,
    filter: &DatasetAccessFilter<'_>,
) {
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
    if filter.is_admin {
        return;
    }

    query
        .push(" AND (d.owner_id = ")
        .push_bind(filter.user_id)
        .push(" OR d.visibility IN (2, 3)")
        .push(
            " OR EXISTS (
                SELECT 1
                FROM sys_resource_permission AS rp
                WHERE rp.tenant_id = d.tenant_id
                  AND rp.resource_type = 'ai_dataset'
                  AND rp.resource_id = d.id::TEXT
                  AND rp.effect = 'allow'
                  AND rp.permission_value IN ('read', 'write', 'manage', 'owner')
                  AND (rp.expires_at IS NULL OR rp.expires_at > NOW())
                  AND (",
        )
        .push(" (rp.subject_type = 'user' AND rp.subject_id = ")
        .push_bind(filter.user_id.to_string())
        .push(")");

    let role_ids = filter
        .role_ids
        .iter()
        .copied()
        .filter(|role_id| *role_id > 0)
        .collect::<Vec<_>>();
    if !role_ids.is_empty() {
        query.push(" OR (rp.subject_type = 'role' AND rp.subject_id IN (");
        let mut separated = query.separated(", ");
        for role_id in role_ids {
            separated.push_bind(role_id.to_string());
        }
        query.push("))");
    }

    query.push(")))");
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

fn parser_job_select_sql() -> &'static str {
    r#"
SELECT
    j.id,
    j.tenant_id,
    j.dataset_id,
    j.document_id,
    j.job_type,
    j.status,
    j.attempt_count,
    COALESCE(j.error_message, '') AS error_message,
    j.result_summary,
    d.name AS document_name,
    COALESCE(d.source_uri, '') AS source_uri,
    d.file_id,
    COALESCE(d.content_type, '') AS content_type,
    d.parse_status,
    d.ingestion_status,
    d.chunk_count,
    j.create_time,
    COALESCE(cu.nickname, '') AS create_user_string,
    j.update_time,
    COALESCE(uu.nickname, '') AS update_user_string
FROM ai_parser_job AS j
JOIN ai_document AS d ON d.tenant_id = j.tenant_id AND d.id = j.document_id
LEFT JOIN sys_user AS cu ON cu.id = j.create_user
LEFT JOIN sys_user AS uu ON uu.id = j.update_user
WHERE j.tenant_id = $1 AND j.dataset_id = $2 AND j.id = $3
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

async fn parser_outbox_table_exists(tx: &mut Transaction<'_, Postgres>) -> Result<bool, AppError> {
    Ok(sqlx::query_scalar::<_, Option<String>>(
        "SELECT to_regclass('public.ai_parser_outbox')::TEXT;",
    )
    .fetch_one(&mut **tx)
    .await?
    .is_some())
}

async fn insert_dataset_vector_collection(
    tx: &mut Transaction<'_, Postgres>,
    record: &DatasetSaveRecord<'_>,
) -> Result<(), AppError> {
    ensure_dataset_vector_collection(
        tx,
        record.tenant_id,
        record.id,
        Some(record.name),
        record.user_id,
        record.now,
    )
    .await
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CollectionEmbeddingShape {
    embedding_model_route: String,
    dimension: i32,
}

fn collection_embedding_shape_from_chunks(
    chunks: &[ChunkSaveRecord],
) -> Option<CollectionEmbeddingShape> {
    chunks.iter().find_map(|chunk| {
        let vector = chunk_embedding_vector(chunk)?;
        let dimension = chunk_embedding_dimension(&vector);
        if dimension <= 0 {
            return None;
        }
        Some(CollectionEmbeddingShape {
            embedding_model_route: chunk
                .embedding_model
                .clone()
                .unwrap_or_else(|| DEFAULT_EMBEDDING_MODEL_ROUTE.to_owned()),
            dimension,
        })
    })
}

async fn update_vector_collection_embedding_shape(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: i64,
    dataset_id: i64,
    chunks: &[ChunkSaveRecord],
) -> Result<(), AppError> {
    let Some(shape) = collection_embedding_shape_from_chunks(chunks) else {
        return Ok(());
    };

    sqlx::query(
        r#"
UPDATE ai_vector_collection
SET embedding_model_route = $1,
    dimension = $2,
    index_policy = jsonb_set(index_policy, '{dimension}', to_jsonb($2::int), true),
    update_user = 0,
    update_time = NOW()
WHERE tenant_id = $3 AND dataset_id = $4;
"#,
    )
    .bind(&shape.embedding_model_route)
    .bind(shape.dimension)
    .bind(tenant_id)
    .bind(dataset_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn ensure_dataset_vector_collection(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: i64,
    dataset_id: i64,
    dataset_name: Option<&str>,
    user_id: i64,
    now: NaiveDateTime,
) -> Result<(), AppError> {
    let collection_code = dataset_vector_collection_code(tenant_id, dataset_id);
    let collection_name = dataset_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| format!("{name} Vector Collection"))
        .unwrap_or_else(|| format!("Dataset {dataset_id} Vector Collection"));
    let provider_collection = provider_vector_collection_name(tenant_id, dataset_id);

    sqlx::query(
        r#"
INSERT INTO ai_vector_collection (
    id, tenant_id, dataset_id, code, name, vector_backend, provider_collection,
    embedding_model_route, dimension, metric_type, status, index_policy, filter_policy,
    metadata, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'cosine', $10, $11, $12, $13, $14, $15)
ON CONFLICT (tenant_id, dataset_id) DO NOTHING;
"#,
    )
    .bind(next_id())
    .bind(tenant_id)
    .bind(dataset_id)
    .bind(&collection_code)
    .bind(&collection_name)
    .bind(DEFAULT_VECTOR_BACKEND)
    .bind(&provider_collection)
    .bind(DEFAULT_EMBEDDING_MODEL_ROUTE)
    .bind(DEFAULT_VECTOR_DIMENSION)
    .bind(VECTOR_COLLECTION_STATUS_READY)
    .bind(json!({
        "kind": "local-poc",
        "metricType": "cosine",
        "dimension": DEFAULT_VECTOR_DIMENSION,
    }))
    .bind(json!({
        "tenantId": tenant_id,
        "datasetId": dataset_id,
    }))
    .bind(json!({
        "source": "novex",
        "contract": "m1-vector-persistence",
    }))
    .bind(user_id)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_chunk_embedding(
    tx: &mut Transaction<'_, Postgres>,
    chunk: &ChunkSaveRecord,
) -> Result<(), AppError> {
    let Some(vector) = chunk_embedding_vector(chunk) else {
        return Ok(());
    };
    let embedding_model_route = chunk
        .embedding_model
        .as_deref()
        .unwrap_or(DEFAULT_EMBEDDING_MODEL_ROUTE);
    let embedding_ref = chunk
        .embedding_ref
        .as_deref()
        .unwrap_or_else(|| chunk_embedding_ref(&chunk.chunk_uid));
    let dimension = chunk_embedding_dimension(&vector);
    let embedding_metadata = json!({
        "chunkUid": chunk.chunk_uid,
        "chunkIndex": chunk.chunk_index,
        "source": "ai_document_chunk.metadata.embedding",
        "embedding": chunk.metadata.get("embedding").cloned().unwrap_or_else(|| json!({})),
    });

    sqlx::query(
        r#"
INSERT INTO ai_embedding (
    id, tenant_id, dataset_id, document_id, chunk_id, chunk_uid, collection_id,
    collection_code, embedding_ref, embedding_model_route, embedding_status, dimension,
    vector, content_hash, metadata, create_user, create_time
)
SELECT
    $1, $2, $3, $4, $5, $6, c.id, c.code, $7, $8, $9, $10, $11, $12, $13, $14, $15
FROM ai_vector_collection AS c
WHERE c.tenant_id = $2 AND c.dataset_id = $3
ON CONFLICT (tenant_id, chunk_id, embedding_model_route) DO UPDATE
SET collection_id = EXCLUDED.collection_id,
    collection_code = EXCLUDED.collection_code,
    embedding_ref = EXCLUDED.embedding_ref,
    embedding_status = EXCLUDED.embedding_status,
    dimension = EXCLUDED.dimension,
    vector = EXCLUDED.vector,
    metadata = EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
    )
    .bind(next_id())
    .bind(chunk.tenant_id)
    .bind(chunk.dataset_id)
    .bind(chunk.document_id)
    .bind(chunk.id)
    .bind(&chunk.chunk_uid)
    .bind(embedding_ref)
    .bind(embedding_model_route)
    .bind(chunk.embedding_status)
    .bind(dimension)
    .bind(&vector)
    .bind(chunk_content_hash(chunk))
    .bind(&embedding_metadata)
    .bind(chunk.user_id)
    .bind(chunk.now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn dataset_vector_collection_code(tenant_id: i64, dataset_id: i64) -> String {
    format!("tenant-{tenant_id}-dataset-{dataset_id}-default")
}

fn provider_vector_collection_name(tenant_id: i64, dataset_id: i64) -> String {
    format!("novex_t{tenant_id}_dataset_{dataset_id}")
}

fn chunk_embedding_ref(chunk_uid: &str) -> &str {
    if chunk_uid.is_empty() {
        "postgres-jsonb:unknown"
    } else {
        chunk_uid
    }
}

fn chunk_embedding_vector(chunk: &ChunkSaveRecord) -> Option<Value> {
    let vector = chunk
        .metadata
        .get("embedding")
        .and_then(|embedding| embedding.get("vector"))?;
    vector.as_array().filter(|items| !items.is_empty())?;
    Some(vector.clone())
}

fn chunk_embedding_dimension(vector: &Value) -> i32 {
    vector
        .as_array()
        .map(|items| items.len().min(i32::MAX as usize) as i32)
        .filter(|dimension| *dimension > 0)
        .unwrap_or(DEFAULT_VECTOR_DIMENSION)
}

fn chunk_content_hash(chunk: &ChunkSaveRecord) -> Option<String> {
    chunk
        .metadata
        .get("contentHash")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk_record_with_embedding(route: &str, vector: Vec<f32>) -> ChunkSaveRecord {
        ChunkSaveRecord {
            id: 1,
            tenant_id: 1,
            dataset_id: 7,
            document_id: 9,
            chunk_uid: "chunk-a".to_owned(),
            chunk_index: 0,
            content: "content".to_owned(),
            semantic_search_text: "content".to_owned(),
            token_count: 1,
            citation: json!({}),
            segment_type: "text".to_owned(),
            segment_index: 0,
            page_no: None,
            section_path: json!([]),
            content_role: "canonical".to_owned(),
            display_capability: "text_only".to_owned(),
            metadata: json!({
                "embedding": {
                    "routeId": route,
                    "source": "runtime",
                    "vector": vector
                }
            }),
            embedding_model: Some(route.to_owned()),
            embedding_ref: Some("chunk-a".to_owned()),
            embedding_status: 4,
            user_id: 99,
            now: chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc(),
        }
    }

    #[test]
    fn collection_embedding_shape_uses_runtime_chunk_embedding_dimension() {
        let chunks = vec![chunk_record_with_embedding(
            "runtime.embedding",
            vec![0.1, 0.2, 0.3],
        )];

        let shape = collection_embedding_shape_from_chunks(&chunks).unwrap();

        assert_eq!(shape.embedding_model_route, "runtime.embedding");
        assert_eq!(shape.dimension, 3);
    }

    #[test]
    fn vector_persistence_migration_declares_collection_and_embedding_tables() {
        let migration_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/202606060001_create_ai_vector_persistence.sql"
        );
        let migration = std::fs::read_to_string(migration_path)
            .expect("missing AI vector persistence migration");

        for needle in [
            "CREATE TABLE IF NOT EXISTS ai_vector_collection",
            "CREATE TABLE IF NOT EXISTS ai_embedding",
            "tenant_id",
            "dataset_id",
            "document_id",
            "chunk_id",
            "embedding_ref",
            "embedding_model_route",
            "dimension",
            "vector JSONB",
            "idx_ai_vector_collection_dataset",
            "idx_ai_embedding_dataset",
            "idx_ai_embedding_document",
            "idx_ai_embedding_status",
        ] {
            assert!(
                migration.contains(needle),
                "{needle} missing from migration"
            );
        }
    }

    #[test]
    fn parser_outbox_migration_defines_durable_queue_contract() {
        let migration_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/202606080001_create_ai_parser_outbox.sql"
        );
        let migration =
            std::fs::read_to_string(migration_path).expect("missing AI parser outbox migration");

        for needle in [
            "CREATE TABLE IF NOT EXISTS ai_parser_outbox",
            "tenant_id",
            "dataset_id",
            "document_id",
            "parser_job_id",
            "event_type",
            "payload JSONB",
            "status",
            "attempt_count",
            "last_error",
            "published_time",
            "uq_ai_parser_outbox_parser_job",
            "idx_ai_parser_outbox_status",
            "idx_ai_parser_outbox_parser_job",
        ] {
            assert!(
                migration.contains(needle),
                "{needle} missing from parser outbox migration"
            );
        }
    }

    #[test]
    fn repository_persists_vector_collection_and_embedding_records() {
        let source = include_str!("ai_knowledge_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "INSERT INTO ai_vector_collection",
            "INSERT INTO ai_embedding",
            "get_vector_collection",
            "provider_collection",
            "vector_backend",
            "dataset_vector_collection_code",
            "chunk_embedding_vector",
            "embedding_status",
        ] {
            assert!(source.contains(needle), "{needle} missing from repository");
        }
    }

    #[test]
    fn repository_creates_parser_outbox_in_parse_job_transaction() {
        let source = include_str!("ai_knowledge_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "ParserOutboxSaveRecord",
            "create_document_parse_job",
            "INSERT INTO ai_parser_outbox",
            "parser_job_id",
            "event_type",
            "payload",
            "attempt_count",
        ] {
            assert!(source.contains(needle), "{needle} missing from repository");
        }
    }

    #[test]
    fn repository_deletes_dataset_cascade_records_in_one_transaction() {
        let source = include_str!("ai_knowledge_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "delete_dataset_cascade",
            "DELETE FROM ai_feedback AS f",
            "DELETE FROM ai_chat_flow_message AS m",
            "DELETE FROM ai_chat_flow_session",
            "DELETE FROM ai_rag_trace_hit",
            "DELETE FROM ai_rag_trace",
            "parser_outbox_table_exists(&mut tx).await?",
            "DELETE FROM ai_parser_outbox",
            "DELETE FROM ai_parser_job",
            "DELETE FROM ai_embedding",
            "DELETE FROM ai_vector_collection",
            "DELETE FROM ai_document_block",
            "DELETE FROM ai_document_chunk",
            "DELETE FROM ai_document",
            "DELETE FROM sys_resource_permission",
            "DELETE FROM ai_dataset",
            "tx.commit().await?",
            "ensure_affected(result.rows_affected())?",
        ] {
            assert!(
                source.contains(needle),
                "{needle} missing from repository cascade delete"
            );
        }
    }

    #[test]
    fn repository_manages_parser_outbox_publish_state() {
        let source = include_str!("ai_knowledge_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "list_pending_parser_outbox",
            "mark_parser_outbox_published",
            "mark_parser_outbox_publish_failed",
            "FROM ai_parser_outbox",
            "status = 1",
            "status = 2",
            "published_time",
            "attempt_count = attempt_count + 1",
            "last_error",
        ] {
            assert!(source.contains(needle), "{needle} missing from repository");
        }
    }

    #[test]
    fn dataset_queries_enforce_owner_visibility_and_resource_permissions() {
        let source = include_str!("ai_knowledge_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "DatasetAccessFilter",
            "push_dataset_access_filters",
            "d.owner_id =",
            "d.visibility IN (2, 3)",
            "sys_resource_permission",
            "rp.resource_type = 'ai_dataset'",
            "rp.subject_type = 'role'",
            "rp.permission_value IN ('read', 'write', 'manage', 'owner')",
        ] {
            assert!(
                source.contains(needle),
                "{needle} missing from dataset access query"
            );
        }
    }
}
