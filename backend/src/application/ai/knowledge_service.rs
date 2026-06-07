use std::{
    collections::{HashMap, HashSet},
    env,
    time::Duration,
};

use chrono::{NaiveDateTime, Utc};
use novex_model::{ModelRuntimeConfig, ModelRuntimeTarget};
use novex_rag::{
    build_extractive_answer, build_semantic_search_text, chunk_document, keyword_retrieve,
    parse_document_content, parse_milvus_search_hits, BoundingBox, ChunkMetadata, ChunkSegmentType,
    CitationRef, ContentRole, DisplayCapability, DocumentChunk as RagDocumentChunk,
    MilvusMetricType, MilvusSearchHit, MilvusSearchRequest, MilvusUpsertRequest, MilvusUpsertRow,
    RagAnswer, RagModelRoutes, RetrievalHit, LOCAL_EMBEDDING_ROUTE,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::{
    application::ai::model_service::{
        ModelChatCommand, ModelChatMessage, ModelChatResp, ModelEmbeddingVector, ModelRerankScore,
        ModelRuntimeService,
    },
    application::system::{
        ensure_max_chars, file_service::FileResp, format_datetime, format_optional_datetime,
    },
    infrastructure::persistence::ai_knowledge_repository::{
        AiKnowledgeRepository, BlockSaveRecord, ChunkRecord, ChunkSaveRecord, DatasetAccessFilter,
        DatasetFilter, DatasetRecord, DatasetSaveRecord, DocumentFilter, DocumentRecord,
        DocumentSaveRecord, FeedbackSaveRecord, ParserJobFilter, ParserJobRecord,
        ParserJobSaveRecord, RagTraceHitSaveRecord, RagTraceSaveRecord, VectorCollectionRecord,
    },
    shared::{
        error::AppError,
        id::next_id,
        pagination::{PageQuery, PageResult, DEFAULT_PAGE, DEFAULT_PAGE_SIZE},
    },
};

const DEFAULT_TENANT_ID: i64 = 1;
const DATASET_STATUS_DRAFT: i16 = 1;
const VISIBILITY_PRIVATE: i16 = 1;
const RETRIEVAL_MODE_HYBRID: i16 = 3;
const DEFAULT_DOCUMENT_CONTENT_TYPE: &str = "text/plain";
const DEFAULT_CHUNK_MAX_CHARS: usize = 1200;
const DEFAULT_CHUNK_OVERLAP_CHARS: usize = 120;
const DOCUMENT_PARSE_STATUS_PARSED: i16 = 3;
const DOCUMENT_PARSE_STATUS_PARSING: i16 = 2;
const DOCUMENT_PARSE_STATUS_FAILED: i16 = 4;
const DOCUMENT_INGESTION_STATUS_PENDING: i16 = 1;
const DOCUMENT_INGESTION_STATUS_INDEXED: i16 = 4;
const PARSER_JOB_TYPE_TEXT: i16 = 1;
const PARSER_JOB_TYPE_WORKER: i16 = 2;
const PARSER_JOB_STATUS_SUBMITTED: i16 = 2;
const PARSER_JOB_STATUS_SUCCEEDED: i16 = 3;
const PARSER_JOB_STATUS_FAILED: i16 = 4;
const CHUNK_EMBEDDING_STATUS_INDEXED: i16 = 4;
const DEFAULT_RAG_LIMIT: usize = 5;
const MAX_RAG_LIMIT: usize = 10;
const RERANK_CANDIDATE_MULTIPLIER: usize = 4;
const MAX_RERANK_CANDIDATES: usize = 30;
const DEFAULT_RAG_ANSWER_MAX_TOKENS: u32 = 1024;
const LOCAL_EMBEDDING_DIMENSION: usize = 64;
const MAX_LOCAL_RETRIEVAL_CHUNKS: i64 = 500;
const VECTOR_COLLECTION_STATUS_READY: i16 = 1;
const MILVUS_SEARCH_TIMEOUT: Duration = Duration::from_secs(5);
const FEEDBACK_STATUS_OPEN: i16 = 1;
const FEEDBACK_RESOURCE_RAG_TRACE: &str = "rag_trace";
const FEEDBACK_RATING_HELPFUL: &str = "helpful";
const FEEDBACK_RATING_NOT_HELPFUL: &str = "not_helpful";
const FEEDBACK_RATING_CITATION_ISSUE: &str = "citation_issue";

#[derive(Debug, Clone, PartialEq, Eq)]
struct MilvusSearchConfig {
    endpoint: String,
    token: Option<String>,
}

impl MilvusSearchConfig {
    fn from_env() -> Option<Self> {
        Self::from_env_map(|key| env::var(key).ok())
    }

    fn from_env_map<F>(mut env_get: F) -> Option<Self>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let endpoint = env_get("MILVUS_ENDPOINT")
            .or_else(|| env_get("NOVEX_MILVUS_ENDPOINT"))
            .map(|value| value.trim().trim_end_matches('/').to_owned())
            .filter(|value| !value.is_empty())?;
        let token = env_get("MILVUS_TOKEN")
            .or_else(|| env_get("NOVEX_MILVUS_TOKEN"))
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());

        Some(Self { endpoint, token })
    }

    fn search_url(&self) -> String {
        format!("{}/v2/vectordb/entities/search", self.endpoint)
    }

    fn upsert_url(&self) -> String {
        format!("{}/v2/vectordb/entities/upsert", self.endpoint)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_size")]
    pub size: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub status: Option<i16>,
}

impl Default for DatasetQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            size: DEFAULT_PAGE_SIZE,
            name: None,
            status: None,
        }
    }
}

impl DatasetQuery {
    pub fn page_query(&self) -> PageQuery {
        PageQuery {
            page: self.page,
            size: self.size,
        }
        .normalized()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetCommand {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_visibility")]
    pub visibility: i16,
    #[serde(default = "default_retrieval_mode")]
    pub retrieval_mode: i16,
}

impl Default for DatasetCommand {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            visibility: VISIBILITY_PRIVATE,
            retrieval_mode: RETRIEVAL_MODE_HYBRID,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_size")]
    pub size: u64,
}

impl Default for DocumentQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            size: DEFAULT_PAGE_SIZE,
        }
    }
}

impl DocumentQuery {
    pub fn page_query(&self) -> PageQuery {
        PageQuery {
            page: self.page,
            size: self.size,
        }
        .normalized()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentUploadCommand {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub content: String,
    #[serde(default = "default_document_content_type")]
    pub content_type: String,
}

impl Default for DocumentUploadCommand {
    fn default() -> Self {
        Self {
            name: String::new(),
            content: String::new(),
            content_type: DEFAULT_DOCUMENT_CONTENT_TYPE.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedDocumentUploadCommand {
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_document_content_type")]
    pub content_type: String,
    pub parser_result: ParserWorkerParseResult,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentParseJobCommand {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub file_id: Option<i64>,
    #[serde(default)]
    pub source_uri: String,
    #[serde(default = "default_document_content_type")]
    pub content_type: String,
    #[serde(default)]
    pub source_hash: String,
    #[serde(default)]
    pub source_kind: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserJobStatusUpdateCommand {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub callback_status: String,
    #[serde(default)]
    pub parser_result: Value,
    #[serde(default)]
    pub mineru_task: Value,
    #[serde(default)]
    pub error: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserWorkerParseResult {
    #[serde(default)]
    pub tenant_id: i64,
    #[serde(default)]
    pub dataset_id: i64,
    #[serde(default)]
    pub document_id: i64,
    #[serde(default)]
    pub parser_job_id: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub error: Option<Value>,
    #[serde(default)]
    pub blocks: Vec<ParserWorkerBlock>,
    #[serde(default)]
    pub chunks: Vec<ParserWorkerChunk>,
    #[serde(default)]
    pub metadata: ParserWorkerMetadata,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserWorkerBlock {
    #[serde(default)]
    pub block_id: String,
    #[serde(default, rename = "type")]
    pub block_type: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub page_no: Option<i32>,
    #[serde(default)]
    pub section_path: Vec<String>,
    #[serde(default)]
    pub bbox: Option<ParserWorkerBbox>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserWorkerBbox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserWorkerChunk {
    #[serde(default)]
    pub chunk_uid: String,
    #[serde(default)]
    pub chunk_index: usize,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub semantic_search_text: String,
    #[serde(default)]
    pub segment_type: String,
    #[serde(default)]
    pub table_header: Vec<String>,
    #[serde(default)]
    pub image_access_keys: Vec<String>,
    #[serde(default)]
    pub content_role: String,
    #[serde(default)]
    pub display_capability: String,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub token_count: usize,
    pub citation: ParserWorkerChunkCitation,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserWorkerChunkCitation {
    #[serde(default)]
    pub document_id: String,
    #[serde(default)]
    pub chunk_id: String,
    #[serde(default)]
    pub page_no: Option<i32>,
    #[serde(default)]
    pub section_path: Vec<String>,
    #[serde(default)]
    pub block_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserWorkerMetadata {
    #[serde(default)]
    pub parser: Option<String>,
    #[serde(default)]
    pub page_count: Option<i32>,
    #[serde(default)]
    pub line_count: Option<i32>,
    #[serde(default)]
    pub source_hash: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RagAskCommand {
    #[serde(default)]
    pub question: String,
    #[serde(default = "default_rag_limit")]
    pub limit: usize,
}

impl Default for RagAskCommand {
    fn default() -> Self {
        Self {
            question: String::new(),
            limit: DEFAULT_RAG_LIMIT,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RagFeedbackCommand {
    #[serde(default)]
    pub trace_id: i64,
    #[serde(default)]
    pub rating: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiFeedbackCommand {
    #[serde(default)]
    pub resource_type: String,
    #[serde(default)]
    pub resource_id: String,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub rating: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetResp {
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
    pub create_user_string: String,
    pub create_time: String,
    pub update_user_string: String,
    pub update_time: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentResp {
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
    pub create_user_string: String,
    pub create_time: String,
    pub update_user_string: String,
    pub update_time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserJobResp {
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
    pub parser_request: Option<Value>,
    pub create_user_string: String,
    pub create_time: String,
    pub update_user_string: String,
    pub update_time: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CitationResp {
    pub document_id: String,
    pub chunk_id: String,
    pub page_no: Option<i32>,
    pub section_path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RagAskResp {
    pub trace_id: i64,
    pub answer: String,
    pub citations: Vec<CitationResp>,
    pub retrieval_hit_count: usize,
    pub answer_strategy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackResp {
    pub id: i64,
    pub trace_id: i64,
    pub rating: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiFeedbackResp {
    pub id: i64,
    pub resource_type: String,
    pub resource_id: String,
    pub trace_id: Option<String>,
    pub rating: String,
}

#[derive(Debug, Clone)]
struct IndexedRagChunk {
    chunk_db_id: i64,
    document_id: i64,
    embedding_vector: Option<Vec<f32>>,
    chunk: RagDocumentChunk,
}

#[derive(Debug, Clone)]
struct ParsedDocumentIngestionParts {
    document: DocumentSaveRecord,
    parser_job: ParserJobSaveRecord,
    blocks: Vec<BlockSaveRecord>,
    chunks: Vec<ChunkSaveRecord>,
}

#[derive(Debug, Clone)]
struct ParserJobStatusUpdateRecord {
    parser_job: ParserJobSaveRecord,
    document_parse_status: i16,
    document_ingestion_status: i16,
    error_message: Option<String>,
}

#[derive(Debug, Clone)]
struct IndexedRetrievalHit {
    chunk_db_id: i64,
    document_id: i64,
    rank: i32,
    score: f32,
    citation: CitationRef,
    content: String,
    token_count: i32,
}

#[derive(Debug, Clone)]
pub struct KnowledgeService {
    repo: AiKnowledgeRepository,
}

impl KnowledgeService {
    pub fn new(db: PgPool) -> Self {
        Self {
            repo: AiKnowledgeRepository::new(db),
        }
    }

    pub async fn list_datasets(
        &self,
        query: DatasetQuery,
    ) -> Result<PageResult<DatasetResp>, AppError> {
        self.list_datasets_for_tenant(DEFAULT_TENANT_ID, query)
            .await
    }

    pub async fn list_datasets_for_tenant(
        &self,
        tenant_id: i64,
        query: DatasetQuery,
    ) -> Result<PageResult<DatasetResp>, AppError> {
        let page = query.page_query();
        let filter = DatasetFilter {
            tenant_id,
            name: query.name.as_deref(),
            status: query.status,
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_datasets(&filter).await?;
        let list = self
            .repo
            .list_datasets(&filter)
            .await?
            .into_iter()
            .map(DatasetResp::from)
            .collect();
        Ok(PageResult::new(list, total))
    }

    pub async fn list_datasets_for_user(
        &self,
        tenant_id: i64,
        user_id: i64,
        role_ids: &[i64],
        is_admin: bool,
        query: DatasetQuery,
    ) -> Result<PageResult<DatasetResp>, AppError> {
        let page = query.page_query();
        let filter = DatasetAccessFilter {
            tenant_id,
            user_id,
            role_ids,
            is_admin,
            name: query.name.as_deref(),
            status: query.status,
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_accessible_datasets(&filter).await?;
        let list = self
            .repo
            .list_accessible_datasets(&filter)
            .await?
            .into_iter()
            .map(DatasetResp::from)
            .collect();
        Ok(PageResult::new(list, total))
    }

    pub async fn create_dataset(
        &self,
        user_id: i64,
        command: DatasetCommand,
    ) -> Result<i64, AppError> {
        self.create_dataset_for_tenant(DEFAULT_TENANT_ID, user_id, command)
            .await
    }

    pub async fn create_dataset_for_tenant(
        &self,
        tenant_id: i64,
        user_id: i64,
        command: DatasetCommand,
    ) -> Result<i64, AppError> {
        let command = normalize_dataset_command(command)?;
        let id = next_id();
        let record = dataset_save_record(tenant_id, id, user_id, &command);
        self.repo.create_dataset(&record).await?;
        Ok(id)
    }

    pub async fn list_documents(
        &self,
        dataset_id: i64,
        query: DocumentQuery,
    ) -> Result<PageResult<DocumentResp>, AppError> {
        self.list_documents_for_tenant(DEFAULT_TENANT_ID, dataset_id, query)
            .await
    }

    pub async fn list_documents_for_tenant(
        &self,
        tenant_id: i64,
        dataset_id: i64,
        query: DocumentQuery,
    ) -> Result<PageResult<DocumentResp>, AppError> {
        if dataset_id <= 0 {
            return Err(AppError::bad_request("知识库 ID 不合法"));
        }
        if !self.repo.dataset_exists(tenant_id, dataset_id).await? {
            return Err(AppError::NotFound);
        }
        let page = query.page_query();
        let filter = DocumentFilter {
            tenant_id,
            dataset_id,
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_documents(&filter).await?;
        let list = self
            .repo
            .list_documents(&filter)
            .await?
            .into_iter()
            .map(DocumentResp::from)
            .collect();
        Ok(PageResult::new(list, total))
    }

    pub async fn list_documents_for_user(
        &self,
        tenant_id: i64,
        user_id: i64,
        role_ids: &[i64],
        is_admin: bool,
        dataset_id: i64,
        query: DocumentQuery,
    ) -> Result<PageResult<DocumentResp>, AppError> {
        if dataset_id <= 0 {
            return Err(AppError::bad_request("知识库 ID 不合法"));
        }
        if !self
            .repo
            .dataset_readable(
                &DatasetAccessFilter {
                    tenant_id,
                    user_id,
                    role_ids,
                    is_admin,
                    name: None,
                    status: None,
                    limit: 1,
                    offset: 0,
                },
                dataset_id,
            )
            .await?
        {
            return Err(AppError::NotFound);
        }
        let page = query.page_query();
        let filter = DocumentFilter {
            tenant_id,
            dataset_id,
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_documents(&filter).await?;
        let list = self
            .repo
            .list_documents(&filter)
            .await?
            .into_iter()
            .map(DocumentResp::from)
            .collect();
        Ok(PageResult::new(list, total))
    }

    pub async fn upload_text_document(
        &self,
        user_id: i64,
        dataset_id: i64,
        command: DocumentUploadCommand,
    ) -> Result<i64, AppError> {
        self.upload_text_document_for_tenant(DEFAULT_TENANT_ID, user_id, dataset_id, command)
            .await
    }

    pub async fn upload_text_document_for_tenant(
        &self,
        tenant_id: i64,
        user_id: i64,
        dataset_id: i64,
        command: DocumentUploadCommand,
    ) -> Result<i64, AppError> {
        if dataset_id <= 0 {
            return Err(AppError::bad_request("知识库 ID 不合法"));
        }
        if !self.repo.dataset_exists(tenant_id, dataset_id).await? {
            return Err(AppError::NotFound);
        }
        let command = normalize_document_upload_command(command)?;
        let document_id = next_id();
        let chunks = document_upload_chunks(document_id, &command);
        let now = Utc::now().naive_utc();
        let document = DocumentSaveRecord {
            id: document_id,
            tenant_id,
            dataset_id,
            name: command.name.clone(),
            source_uri: None,
            file_id: None,
            content_type: Some(command.content_type.clone()),
            owner_id: user_id,
            visibility: VISIBILITY_PRIVATE,
            parse_status: DOCUMENT_PARSE_STATUS_PARSED,
            ingestion_status: DOCUMENT_INGESTION_STATUS_INDEXED,
            chunk_count: chunks.len() as i32,
            source_hash: Some(sha256_hex(&command.content)),
            user_id,
            now,
        };
        let parser_job = ParserJobSaveRecord {
            id: next_id(),
            tenant_id,
            dataset_id,
            document_id,
            job_type: PARSER_JOB_TYPE_TEXT,
            status: PARSER_JOB_STATUS_SUCCEEDED,
            result_summary: parser_job_result_summary(&command, &chunks),
            user_id,
            now,
        };
        let mut chunk_records =
            chunk_save_records(tenant_id, dataset_id, document_id, chunks, user_id, now);
        enrich_chunk_records_with_runtime_embeddings(&mut chunk_records).await;

        self.repo
            .create_document_ingestion(&document, &parser_job, &[], &chunk_records)
            .await?;
        self.upsert_chunks_to_milvus_after_ingestion(tenant_id, dataset_id, &chunk_records)
            .await;
        Ok(document_id)
    }

    pub async fn upload_parsed_document(
        &self,
        user_id: i64,
        dataset_id: i64,
        command: ParsedDocumentUploadCommand,
    ) -> Result<i64, AppError> {
        self.upload_parsed_document_for_tenant(DEFAULT_TENANT_ID, user_id, dataset_id, command)
            .await
    }

    pub async fn upload_parsed_document_for_tenant(
        &self,
        tenant_id: i64,
        user_id: i64,
        dataset_id: i64,
        command: ParsedDocumentUploadCommand,
    ) -> Result<i64, AppError> {
        if dataset_id <= 0 {
            return Err(AppError::bad_request("知识库 ID 不合法"));
        }
        if !self.repo.dataset_exists(tenant_id, dataset_id).await? {
            return Err(AppError::NotFound);
        }

        let mut parts = parsed_document_ingestion_parts(
            tenant_id,
            dataset_id,
            user_id,
            command,
            Utc::now().naive_utc(),
        )?;
        enrich_chunk_records_with_runtime_embeddings(&mut parts.chunks).await;
        let document_id = parts.document.id;
        if self
            .repo
            .parser_job_exists(
                tenant_id,
                dataset_id,
                parts.document.id,
                parts.parser_job.id,
            )
            .await?
        {
            self.repo
                .complete_document_parse_job(
                    &parts.document,
                    &parts.parser_job,
                    &parts.blocks,
                    &parts.chunks,
                )
                .await?;
        } else {
            self.repo
                .create_document_ingestion(
                    &parts.document,
                    &parts.parser_job,
                    &parts.blocks,
                    &parts.chunks,
                )
                .await?;
        }
        self.upsert_chunks_to_milvus_after_ingestion(tenant_id, dataset_id, &parts.chunks)
            .await;
        Ok(document_id)
    }

    async fn upsert_chunks_to_milvus_after_ingestion(
        &self,
        tenant_id: i64,
        dataset_id: i64,
        chunks: &[ChunkSaveRecord],
    ) {
        let Some(config) = MilvusSearchConfig::from_env() else {
            return;
        };
        let collection = match self.repo.get_vector_collection(tenant_id, dataset_id).await {
            Ok(Some(collection)) => collection,
            Ok(None) => return,
            Err(err) => {
                tracing::warn!(error = %err, tenant_id, dataset_id, "Milvus collection lookup failed after ingestion");
                return;
            }
        };
        let Some(request) = milvus_upsert_request_for_collection(&collection, chunks) else {
            return;
        };

        if let Err(err) = milvus_upsert_chunks(&config, &request).await {
            tracing::warn!(error = %err, tenant_id, dataset_id, "Milvus upsert failed after ingestion");
        }
    }

    pub async fn create_parse_job(
        &self,
        user_id: i64,
        dataset_id: i64,
        command: DocumentParseJobCommand,
    ) -> Result<ParserJobResp, AppError> {
        self.create_parse_job_for_tenant(DEFAULT_TENANT_ID, user_id, dataset_id, command)
            .await
    }

    pub async fn create_parse_job_for_tenant(
        &self,
        tenant_id: i64,
        user_id: i64,
        dataset_id: i64,
        command: DocumentParseJobCommand,
    ) -> Result<ParserJobResp, AppError> {
        if dataset_id <= 0 {
            return Err(AppError::bad_request("知识库 ID 不合法"));
        }
        if !self.repo.dataset_exists(tenant_id, dataset_id).await? {
            return Err(AppError::NotFound);
        }
        let command = normalize_document_parse_job_command(command)?;
        let document_id = next_id();
        let parser_job_id = next_id();
        let now = Utc::now().naive_utc();
        let parser_request =
            parser_worker_request(tenant_id, dataset_id, document_id, parser_job_id, &command);
        let document = DocumentSaveRecord {
            id: document_id,
            tenant_id,
            dataset_id,
            name: command.name.clone(),
            source_uri: Some(command.source_uri.clone()),
            file_id: command.file_id,
            content_type: Some(command.content_type.clone()),
            owner_id: user_id,
            visibility: VISIBILITY_PRIVATE,
            parse_status: DOCUMENT_PARSE_STATUS_PARSING,
            ingestion_status: DOCUMENT_INGESTION_STATUS_PENDING,
            chunk_count: 0,
            source_hash: non_empty_parser_string(&command.source_hash),
            user_id,
            now,
        };
        let parser_job = ParserJobSaveRecord {
            id: parser_job_id,
            tenant_id,
            dataset_id,
            document_id,
            job_type: PARSER_JOB_TYPE_WORKER,
            status: PARSER_JOB_STATUS_SUBMITTED,
            result_summary: parser_job_submitted_summary(&command, &parser_request),
            user_id,
            now,
        };

        self.repo
            .create_document_parse_job(&document, &parser_job)
            .await?;

        Ok(ParserJobResp {
            id: parser_job_id,
            tenant_id,
            dataset_id,
            document_id,
            job_type: PARSER_JOB_TYPE_WORKER,
            status: PARSER_JOB_STATUS_SUBMITTED,
            attempt_count: 0,
            error_message: String::new(),
            result_summary: parser_job.result_summary,
            document_name: document.name,
            source_uri: document.source_uri.unwrap_or_default(),
            file_id: document.file_id,
            content_type: document.content_type.unwrap_or_default(),
            parse_status: document.parse_status,
            ingestion_status: document.ingestion_status,
            chunk_count: document.chunk_count,
            parser_request: Some(parser_request),
            create_user_string: String::new(),
            create_time: format_datetime(now),
            update_user_string: String::new(),
            update_time: String::new(),
        })
    }

    pub async fn get_parse_job(
        &self,
        dataset_id: i64,
        job_id: i64,
    ) -> Result<ParserJobResp, AppError> {
        self.get_parse_job_for_tenant(DEFAULT_TENANT_ID, dataset_id, job_id)
            .await
    }

    pub async fn get_parse_job_for_tenant(
        &self,
        tenant_id: i64,
        dataset_id: i64,
        job_id: i64,
    ) -> Result<ParserJobResp, AppError> {
        if dataset_id <= 0 || job_id <= 0 {
            return Err(AppError::bad_request("解析任务 ID 不合法"));
        }
        let record = self
            .repo
            .get_parser_job(&ParserJobFilter {
                tenant_id,
                dataset_id,
                job_id,
            })
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(ParserJobResp::from(record))
    }

    pub async fn update_parse_job_status(
        &self,
        user_id: i64,
        dataset_id: i64,
        job_id: i64,
        command: ParserJobStatusUpdateCommand,
    ) -> Result<ParserJobResp, AppError> {
        self.update_parse_job_status_for_tenant(
            DEFAULT_TENANT_ID,
            user_id,
            dataset_id,
            job_id,
            command,
        )
        .await
    }

    pub async fn update_parse_job_status_for_tenant(
        &self,
        tenant_id: i64,
        user_id: i64,
        dataset_id: i64,
        job_id: i64,
        command: ParserJobStatusUpdateCommand,
    ) -> Result<ParserJobResp, AppError> {
        if dataset_id <= 0 || job_id <= 0 {
            return Err(AppError::bad_request("解析任务 ID 不合法"));
        }
        let filter = ParserJobFilter {
            tenant_id,
            dataset_id,
            job_id,
        };
        let record = self
            .repo
            .get_parser_job(&filter)
            .await?
            .ok_or(AppError::NotFound)?;
        let update = parser_job_status_update_record(
            tenant_id,
            dataset_id,
            record.document_id,
            job_id,
            user_id,
            command,
            Utc::now().naive_utc(),
        )?;

        self.repo
            .update_parser_job_status(
                &update.parser_job,
                update.document_parse_status,
                update.document_ingestion_status,
                update.error_message.as_deref(),
            )
            .await?;

        let record = self
            .repo
            .get_parser_job(&filter)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(ParserJobResp::from(record))
    }

    pub async fn ask_dataset(
        &self,
        user_id: i64,
        dataset_id: i64,
        command: RagAskCommand,
    ) -> Result<RagAskResp, AppError> {
        self.ask_dataset_for_tenant(DEFAULT_TENANT_ID, user_id, dataset_id, command)
            .await
    }

    pub async fn ask_dataset_for_tenant(
        &self,
        tenant_id: i64,
        user_id: i64,
        dataset_id: i64,
        command: RagAskCommand,
    ) -> Result<RagAskResp, AppError> {
        if dataset_id <= 0 {
            return Err(AppError::bad_request("知识库 ID 不合法"));
        }
        if !self.repo.dataset_exists(tenant_id, dataset_id).await? {
            return Err(AppError::NotFound);
        }
        self.ask_existing_dataset_for_tenant(tenant_id, user_id, dataset_id, command)
            .await
    }

    pub async fn ask_dataset_for_user(
        &self,
        tenant_id: i64,
        user_id: i64,
        role_ids: &[i64],
        is_admin: bool,
        dataset_id: i64,
        command: RagAskCommand,
    ) -> Result<RagAskResp, AppError> {
        if dataset_id <= 0 {
            return Err(AppError::bad_request("知识库 ID 不合法"));
        }
        if !self
            .repo
            .dataset_readable(
                &DatasetAccessFilter {
                    tenant_id,
                    user_id,
                    role_ids,
                    is_admin,
                    name: None,
                    status: None,
                    limit: 1,
                    offset: 0,
                },
                dataset_id,
            )
            .await?
        {
            return Err(AppError::NotFound);
        }
        self.ask_existing_dataset_for_tenant(tenant_id, user_id, dataset_id, command)
            .await
    }

    async fn ask_existing_dataset_for_tenant(
        &self,
        tenant_id: i64,
        user_id: i64,
        dataset_id: i64,
        command: RagAskCommand,
    ) -> Result<RagAskResp, AppError> {
        let command = normalize_rag_ask_command(command)?;
        let chunk_records = self
            .repo
            .list_indexed_chunks(tenant_id, dataset_id, MAX_LOCAL_RETRIEVAL_CHUNKS)
            .await?;
        let indexed_chunks = indexed_rag_chunks(chunk_records);
        let vector_collection = self
            .repo
            .get_vector_collection(tenant_id, dataset_id)
            .await?;
        let candidate_limit = rerank_candidate_limit(command.limit);
        let candidate_hits = hybrid_retrieve_indexed_chunks_with_milvus_or_local(
            &command.question,
            tenant_id,
            dataset_id,
            vector_collection.as_ref(),
            &indexed_chunks,
            candidate_limit,
        )
        .await?;
        let hits = rerank_dataset_hits(&command.question, candidate_hits, command.limit).await?;
        let indexed_hits = indexed_retrieval_hits(&hits, &indexed_chunks);
        let answer = generate_rag_answer(&command.question, &hits).await?;
        let model_routes = rag_model_routes();
        let trace_id = next_id();
        let now = Utc::now().naive_utc();
        let trace = rag_trace_record(
            trace_id,
            tenant_id,
            user_id,
            dataset_id,
            &command,
            &answer,
            &indexed_hits,
            &model_routes,
            now,
        );
        let trace_hits = rag_trace_hit_records(trace_id, tenant_id, dataset_id, &indexed_hits, now);

        self.repo.create_rag_trace(&trace, &trace_hits).await?;

        Ok(rag_ask_response(trace_id, answer))
    }

    pub async fn submit_rag_feedback(
        &self,
        user_id: i64,
        command: RagFeedbackCommand,
    ) -> Result<FeedbackResp, AppError> {
        self.submit_rag_feedback_for_tenant(DEFAULT_TENANT_ID, user_id, command)
            .await
    }

    pub async fn submit_rag_feedback_for_tenant(
        &self,
        tenant_id: i64,
        user_id: i64,
        command: RagFeedbackCommand,
    ) -> Result<FeedbackResp, AppError> {
        let command = normalize_rag_feedback_command(command)?;
        let feedback_id = next_id();
        let record = FeedbackSaveRecord {
            id: feedback_id,
            tenant_id,
            resource_type: FEEDBACK_RESOURCE_RAG_TRACE.to_owned(),
            resource_id: command.trace_id.to_string(),
            trace_id: Some(command.trace_id.to_string()),
            rating: command.rating.clone(),
            reason: command.reason.clone(),
            metadata: rag_feedback_metadata(&command),
            status: FEEDBACK_STATUS_OPEN,
            user_id,
            now: Utc::now().naive_utc(),
        };

        self.repo.create_feedback(&record).await?;
        Ok(FeedbackResp {
            id: feedback_id,
            trace_id: command.trace_id,
            rating: command.rating,
        })
    }

    pub async fn submit_ai_feedback_for_tenant(
        &self,
        tenant_id: i64,
        user_id: i64,
        command: AiFeedbackCommand,
    ) -> Result<AiFeedbackResp, AppError> {
        let command = normalize_ai_feedback_command(command)?;
        let feedback_id = next_id();
        let record = FeedbackSaveRecord {
            id: feedback_id,
            tenant_id,
            resource_type: command.resource_type.clone(),
            resource_id: command.resource_id.clone(),
            trace_id: command.trace_id.clone(),
            rating: command.rating.clone(),
            reason: command.reason.clone(),
            metadata: ai_feedback_metadata(&command),
            status: FEEDBACK_STATUS_OPEN,
            user_id,
            now: Utc::now().naive_utc(),
        };

        self.repo.create_feedback(&record).await?;
        Ok(AiFeedbackResp {
            id: feedback_id,
            resource_type: command.resource_type,
            resource_id: command.resource_id,
            trace_id: command.trace_id,
            rating: command.rating,
        })
    }
}

impl From<DatasetRecord> for DatasetResp {
    fn from(record: DatasetRecord) -> Self {
        Self {
            id: record.id,
            tenant_id: record.tenant_id,
            name: record.name,
            description: record.description,
            owner_id: record.owner_id,
            visibility: record.visibility,
            status: record.status,
            retrieval_mode: record.retrieval_mode,
            document_count: record.document_count,
            chunk_count: record.chunk_count,
            create_user_string: record.create_user_string,
            create_time: format_datetime(record.create_time),
            update_user_string: record.update_user_string,
            update_time: format_optional_datetime(record.update_time),
        }
    }
}

impl From<DocumentRecord> for DocumentResp {
    fn from(record: DocumentRecord) -> Self {
        Self {
            id: record.id,
            tenant_id: record.tenant_id,
            dataset_id: record.dataset_id,
            name: record.name,
            source_uri: record.source_uri,
            file_id: record.file_id,
            content_type: record.content_type,
            owner_id: record.owner_id,
            visibility: record.visibility,
            parse_status: record.parse_status,
            ingestion_status: record.ingestion_status,
            chunk_count: record.chunk_count,
            source_hash: record.source_hash,
            create_user_string: record.create_user_string,
            create_time: format_datetime(record.create_time),
            update_user_string: record.update_user_string,
            update_time: format_optional_datetime(record.update_time),
        }
    }
}

impl From<ParserJobRecord> for ParserJobResp {
    fn from(record: ParserJobRecord) -> Self {
        let parser_request = parser_request_from_result_summary(&record.result_summary);

        Self {
            id: record.id,
            tenant_id: record.tenant_id,
            dataset_id: record.dataset_id,
            document_id: record.document_id,
            job_type: record.job_type,
            status: record.status,
            attempt_count: record.attempt_count,
            error_message: record.error_message,
            result_summary: record.result_summary,
            document_name: record.document_name,
            source_uri: record.source_uri,
            file_id: record.file_id,
            content_type: record.content_type,
            parse_status: record.parse_status,
            ingestion_status: record.ingestion_status,
            chunk_count: record.chunk_count,
            parser_request,
            create_user_string: record.create_user_string,
            create_time: format_datetime(record.create_time),
            update_user_string: record.update_user_string,
            update_time: format_optional_datetime(record.update_time),
        }
    }
}

fn parser_request_from_result_summary(result_summary: &Value) -> Option<Value> {
    result_summary
        .get("parserRequest")
        .filter(|value| value.is_object())
        .cloned()
}

pub fn normalize_dataset_command(mut command: DatasetCommand) -> Result<DatasetCommand, AppError> {
    command.name = command.name.trim().to_owned();
    command.description = command.description.trim().to_owned();
    if command.visibility == 0 {
        command.visibility = VISIBILITY_PRIVATE;
    }
    if command.retrieval_mode == 0 {
        command.retrieval_mode = RETRIEVAL_MODE_HYBRID;
    }
    if command.name.is_empty() {
        return Err(AppError::bad_request("名称不能为空"));
    }
    ensure_max_chars("名称", &command.name, 100)?;
    ensure_max_chars("描述", &command.description, 2000)?;
    if !(1..=3).contains(&command.visibility) {
        return Err(AppError::bad_request("可见性不合法"));
    }
    if !(1..=3).contains(&command.retrieval_mode) {
        return Err(AppError::bad_request("检索模式不合法"));
    }
    Ok(command)
}

pub fn normalize_document_upload_command(
    mut command: DocumentUploadCommand,
) -> Result<DocumentUploadCommand, AppError> {
    command.name = command.name.trim().to_owned();
    command.content = command.content.trim().to_owned();
    command.content_type = command.content_type.trim().to_owned();
    if command.content_type.is_empty() {
        command.content_type = DEFAULT_DOCUMENT_CONTENT_TYPE.to_owned();
    }
    if command.name.is_empty() {
        return Err(AppError::bad_request("文档名称不能为空"));
    }
    if command.content.is_empty() {
        return Err(AppError::bad_request("文档内容不能为空"));
    }
    ensure_max_chars("文档名称", &command.name, 255)?;
    ensure_max_chars("内容类型", &command.content_type, 255)?;
    Ok(command)
}

pub fn normalize_document_parse_job_command(
    mut command: DocumentParseJobCommand,
) -> Result<DocumentParseJobCommand, AppError> {
    command.name = command.name.trim().to_owned();
    command.source_uri = command.source_uri.trim().to_owned();
    command.content_type = command.content_type.trim().to_owned();
    command.source_hash = command.source_hash.trim().to_owned();
    command.source_kind = command.source_kind.trim().to_owned();
    if command.name.is_empty() {
        return Err(AppError::bad_request("文档名称不能为空"));
    }
    if command.file_id.unwrap_or_default() <= 0 && command.source_uri.is_empty() {
        return Err(AppError::bad_request("文件来源不能为空"));
    }
    if command.content_type.is_empty() {
        command.content_type = mime_guess::from_path(&command.name)
            .first_or_octet_stream()
            .essence_str()
            .to_owned();
    }
    if command.source_kind.is_empty() {
        command.source_kind = infer_parser_source_kind(command.file_id, &command.source_uri);
    }
    if !matches!(
        command.source_kind.as_str(),
        "inlineText" | "objectStorage" | "localFile" | "remoteUrl"
    ) {
        return Err(AppError::bad_request("文件来源类型不合法"));
    }
    ensure_max_chars("文档名称", &command.name, 255)?;
    ensure_max_chars("内容类型", &command.content_type, 255)?;
    ensure_max_chars("文件来源", &command.source_uri, 2000)?;
    ensure_max_chars("文件 Hash", &command.source_hash, 256)?;
    Ok(command)
}

pub fn parse_job_command_from_uploaded_file(
    file: &FileResp,
) -> Result<DocumentParseJobCommand, AppError> {
    if file.id <= 0 {
        return Err(AppError::bad_request("文件 ID 不合法"));
    }
    if file.file_type == 0 {
        return Err(AppError::bad_request("文件夹不能创建解析任务"));
    }
    let name = non_empty_parser_string(&file.original_name).unwrap_or_else(|| file.name.clone());
    let source_uri = non_empty_parser_string(&file.url).unwrap_or_else(|| file.path.clone());
    normalize_document_parse_job_command(DocumentParseJobCommand {
        name,
        file_id: Some(file.id),
        source_uri,
        content_type: file.content_type.clone(),
        source_hash: file.sha256.clone(),
        source_kind: "objectStorage".to_owned(),
    })
}

fn infer_parser_source_kind(file_id: Option<i64>, source_uri: &str) -> String {
    if file_id.unwrap_or_default() > 0 {
        "objectStorage".to_owned()
    } else if source_uri.starts_with("http://") || source_uri.starts_with("https://") {
        "remoteUrl".to_owned()
    } else {
        "localFile".to_owned()
    }
}

fn parser_worker_request(
    tenant_id: i64,
    dataset_id: i64,
    document_id: i64,
    parser_job_id: i64,
    command: &DocumentParseJobCommand,
) -> Value {
    json!({
        "tenantId": tenant_id,
        "datasetId": dataset_id,
        "documentId": document_id,
        "parserJobId": parser_job_id,
        "source": {
            "kind": command.source_kind,
            "contentType": command.content_type,
            "name": command.name,
            "uri": command.source_uri,
            "fileId": command.file_id,
            "sourceHash": non_empty_parser_string(&command.source_hash),
        },
        "options": {
            "maxChunkChars": DEFAULT_CHUNK_MAX_CHARS,
            "chunkOverlapChars": DEFAULT_CHUNK_OVERLAP_CHARS,
            "extractTables": true,
            "extractFormula": true,
            "ocr": false,
            "mineruModelVersion": "vlm",
        },
        "trace": {
            "requestId": format!("parser-job-{parser_job_id}"),
        }
    })
}

fn parser_job_submitted_summary(
    command: &DocumentParseJobCommand,
    parser_request: &Value,
) -> Value {
    json!({
        "parser": "parser-worker",
        "status": "submitted",
        "sourceFileName": command.name,
        "contentType": command.content_type,
        "sourceKind": command.source_kind,
        "fileId": command.file_id,
        "sourceUri": command.source_uri,
        "sourceHash": non_empty_parser_string(&command.source_hash),
        "parserRequest": parser_request,
    })
}

fn parser_job_status_update_record(
    tenant_id: i64,
    dataset_id: i64,
    document_id: i64,
    job_id: i64,
    user_id: i64,
    mut command: ParserJobStatusUpdateCommand,
    now: NaiveDateTime,
) -> Result<ParserJobStatusUpdateRecord, AppError> {
    command.status = command.status.trim().to_ascii_lowercase();
    if command.status.is_empty() {
        command.status = json_string_field(&command.parser_result, "status").unwrap_or_default();
    }
    command.callback_status = command.callback_status.trim().to_ascii_lowercase();
    if command.callback_status.is_empty() {
        command.callback_status = match command.status.as_str() {
            "submitted" => "deferred",
            "failed" => "failed",
            _ => "not_applicable",
        }
        .to_owned();
    }

    validate_parser_job_status_scope(tenant_id, dataset_id, document_id, job_id, &command)?;
    let mineru_task = non_empty_json(command.mineru_task)
        .or_else(|| json_field_clone(&command.parser_result, "mineruTask"));
    let error = command
        .error
        .and_then(non_empty_json)
        .or_else(|| json_field_clone(&command.parser_result, "error"));

    let (parser_status, document_parse_status, document_ingestion_status, error_message) =
        match command.status.as_str() {
            "submitted" => (
                PARSER_JOB_STATUS_SUBMITTED,
                DOCUMENT_PARSE_STATUS_PARSING,
                DOCUMENT_INGESTION_STATUS_PENDING,
                None,
            ),
            "failed" => (
                PARSER_JOB_STATUS_FAILED,
                DOCUMENT_PARSE_STATUS_FAILED,
                DOCUMENT_INGESTION_STATUS_PENDING,
                parser_error_message(error.as_ref()).or_else(|| Some("解析任务失败".to_owned())),
            ),
            "succeeded" => {
                return Err(AppError::bad_request(
                    "解析成功结果必须通过 documents/parsed 接口入库",
                ));
            }
            _ => return Err(AppError::bad_request("解析任务状态不合法")),
        };

    let result_summary = json!({
        "parser": "parser-worker",
        "status": command.status,
        "callbackStatus": command.callback_status,
        "parserResult": command.parser_result,
        "mineruTask": mineru_task.unwrap_or(Value::Null),
        "error": error.unwrap_or(Value::Null),
        "updatedAt": format_datetime(now),
    });

    Ok(ParserJobStatusUpdateRecord {
        parser_job: ParserJobSaveRecord {
            id: job_id,
            tenant_id,
            dataset_id,
            document_id,
            job_type: PARSER_JOB_TYPE_WORKER,
            status: parser_status,
            result_summary,
            user_id,
            now,
        },
        document_parse_status,
        document_ingestion_status,
        error_message,
    })
}

fn validate_parser_job_status_scope(
    tenant_id: i64,
    dataset_id: i64,
    document_id: i64,
    job_id: i64,
    command: &ParserJobStatusUpdateCommand,
) -> Result<(), AppError> {
    for (field, expected) in [
        ("tenantId", tenant_id),
        ("datasetId", dataset_id),
        ("documentId", document_id),
        ("parserJobId", job_id),
    ] {
        if let Some(actual) = json_i64_field(&command.parser_result, field) {
            if actual != expected {
                return Err(AppError::bad_request("解析任务状态归属不匹配"));
            }
        }
    }
    Ok(())
}

fn json_i64_field(value: &Value, field: &str) -> Option<i64> {
    let value = value.as_object()?.get(field)?;
    if let Some(number) = value.as_i64() {
        return Some(number);
    }
    value
        .as_str()
        .and_then(|text| text.trim().parse::<i64>().ok())
}

fn json_string_field(value: &Value, field: &str) -> Option<String> {
    value
        .as_object()?
        .get(field)?
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_ascii_lowercase)
}

fn json_field_clone(value: &Value, field: &str) -> Option<Value> {
    value
        .as_object()?
        .get(field)
        .cloned()
        .and_then(non_empty_json)
}

fn non_empty_json(value: Value) -> Option<Value> {
    match value {
        Value::Null => None,
        Value::Object(map) if map.is_empty() => None,
        Value::Array(items) if items.is_empty() => None,
        Value::String(text) if text.trim().is_empty() => None,
        value => Some(value),
    }
}

fn parser_error_message(error: Option<&Value>) -> Option<String> {
    let error = error?;
    let message = match error {
        Value::String(text) => non_empty_parser_string(text),
        Value::Object(map) => ["message", "errMsg", "error"]
            .iter()
            .find_map(|field| map.get(*field).and_then(Value::as_str))
            .and_then(non_empty_parser_string)
            .or_else(|| Some(error.to_string())),
        _ => Some(error.to_string()),
    }?;

    Some(message.chars().take(2000).collect())
}

pub fn normalize_rag_ask_command(mut command: RagAskCommand) -> Result<RagAskCommand, AppError> {
    command.question = command.question.trim().to_owned();
    if command.limit == 0 {
        command.limit = DEFAULT_RAG_LIMIT;
    }
    command.limit = command.limit.min(MAX_RAG_LIMIT);
    if command.question.is_empty() {
        return Err(AppError::bad_request("问题不能为空"));
    }
    ensure_max_chars("问题", &command.question, 2000)?;
    Ok(command)
}

pub fn normalize_rag_feedback_command(
    mut command: RagFeedbackCommand,
) -> Result<RagFeedbackCommand, AppError> {
    command.rating = command.rating.trim().to_ascii_lowercase();
    command.reason = command.reason.trim().to_owned();
    if command.trace_id <= 0 {
        return Err(AppError::bad_request("Trace ID 不合法"));
    }
    if !matches!(
        command.rating.as_str(),
        FEEDBACK_RATING_HELPFUL | FEEDBACK_RATING_NOT_HELPFUL | FEEDBACK_RATING_CITATION_ISSUE
    ) {
        return Err(AppError::bad_request("反馈类型不合法"));
    }
    ensure_max_chars("反馈原因", &command.reason, 1000)?;
    Ok(command)
}

pub fn normalize_ai_feedback_command(
    mut command: AiFeedbackCommand,
) -> Result<AiFeedbackCommand, AppError> {
    command.resource_type = command.resource_type.trim().to_owned();
    command.resource_id = command.resource_id.trim().to_owned();
    command.trace_id = command
        .trace_id
        .map(|trace_id| trace_id.trim().to_owned())
        .filter(|trace_id| !trace_id.is_empty());
    command.rating = command.rating.trim().to_ascii_lowercase();
    command.reason = command.reason.trim().to_owned();
    if command.resource_type.is_empty() {
        return Err(AppError::bad_request("反馈资源类型不能为空"));
    }
    if command.resource_id.is_empty() {
        return Err(AppError::bad_request("反馈资源 ID 不能为空"));
    }
    if command.rating.is_empty() {
        return Err(AppError::bad_request("反馈类型不能为空"));
    }
    ensure_max_chars("反馈资源类型", &command.resource_type, 64)?;
    ensure_max_chars("反馈资源 ID", &command.resource_id, 128)?;
    if let Some(trace_id) = &command.trace_id {
        ensure_max_chars("Trace ID", trace_id, 128)?;
    }
    ensure_max_chars("反馈类型", &command.rating, 64)?;
    ensure_max_chars("反馈原因", &command.reason, 1000)?;
    if command.metadata.is_null() {
        command.metadata = json!({});
    }
    Ok(command)
}

fn rag_feedback_metadata(command: &RagFeedbackCommand) -> Value {
    json!({
        "rating": command.rating,
        "reasonLength": command.reason.chars().count(),
        "source": "training-web"
    })
}

fn ai_feedback_metadata(command: &AiFeedbackCommand) -> Value {
    command.metadata.clone()
}

fn document_upload_chunks(
    document_id: i64,
    command: &DocumentUploadCommand,
) -> Vec<RagDocumentChunk> {
    let parsed = parse_document_content(
        document_id.to_string(),
        &command.name,
        &command.content_type,
        &command.content,
    );
    chunk_document(
        &parsed,
        DEFAULT_CHUNK_MAX_CHARS,
        DEFAULT_CHUNK_OVERLAP_CHARS,
    )
}

fn parser_job_result_summary(
    command: &DocumentUploadCommand,
    chunks: &[RagDocumentChunk],
) -> Value {
    let mut segment_type_counts = serde_json::Map::new();
    let mut min_semantic_chars: Option<usize> = None;
    let mut max_semantic_chars = 0usize;
    let mut empty_semantic_count = 0usize;

    for chunk in chunks {
        let key = chunk.metadata.segment_type.as_str();
        let count = segment_type_counts
            .get(key)
            .and_then(Value::as_u64)
            .unwrap_or(0)
            + 1;
        segment_type_counts.insert(key.to_owned(), json!(count));

        let semantic_chars = chunk.semantic_search_text.chars().count();
        if semantic_chars == 0 {
            empty_semantic_count += 1;
        }
        min_semantic_chars = Some(
            min_semantic_chars
                .map(|current| current.min(semantic_chars))
                .unwrap_or(semantic_chars),
        );
        max_semantic_chars = max_semantic_chars.max(semantic_chars);
    }

    json!({
        "parser": "novex-rag-local-structured",
        "chunker": "file-type-default",
        "embeddingInput": "semanticSearchText",
        "sourceFileName": command.name,
        "contentType": command.content_type,
        "lineCount": command.content.lines().filter(|line| !line.trim().is_empty()).count(),
        "chunkCount": chunks.len(),
        "maxChunkChars": DEFAULT_CHUNK_MAX_CHARS,
        "overlapChars": DEFAULT_CHUNK_OVERLAP_CHARS,
        "segmentTypeCounts": Value::Object(segment_type_counts),
        "semanticSearchText": {
            "minChars": min_semantic_chars.unwrap_or(0),
            "maxChars": max_semantic_chars,
            "emptyCount": empty_semantic_count,
        }
    })
}

fn parsed_document_ingestion_parts(
    tenant_id: i64,
    dataset_id: i64,
    user_id: i64,
    command: ParsedDocumentUploadCommand,
    now: NaiveDateTime,
) -> Result<ParsedDocumentIngestionParts, AppError> {
    let command = normalize_parsed_document_upload_command(tenant_id, dataset_id, command)?;
    let result = &command.parser_result;
    let document_id = result.document_id;
    let rag_chunks = parser_result_chunks(&command)?;
    let document = DocumentSaveRecord {
        id: document_id,
        tenant_id,
        dataset_id,
        name: command.name.clone(),
        source_uri: None,
        file_id: None,
        content_type: Some(command.content_type.clone()),
        owner_id: user_id,
        visibility: VISIBILITY_PRIVATE,
        parse_status: DOCUMENT_PARSE_STATUS_PARSED,
        ingestion_status: DOCUMENT_INGESTION_STATUS_INDEXED,
        chunk_count: rag_chunks.len() as i32,
        source_hash: parsed_source_hash(&command),
        user_id,
        now,
    };
    let parser_job = ParserJobSaveRecord {
        id: result.parser_job_id,
        tenant_id,
        dataset_id,
        document_id,
        job_type: PARSER_JOB_TYPE_WORKER,
        status: PARSER_JOB_STATUS_SUCCEEDED,
        result_summary: parser_result_summary(&command, &rag_chunks),
        user_id,
        now,
    };
    let blocks =
        parser_block_save_records(tenant_id, dataset_id, document_id, &command, user_id, now);
    let chunks = parser_chunk_save_records(
        tenant_id,
        dataset_id,
        document_id,
        &command,
        rag_chunks,
        user_id,
        now,
    );

    Ok(ParsedDocumentIngestionParts {
        document,
        parser_job,
        blocks,
        chunks,
    })
}

fn normalize_parsed_document_upload_command(
    tenant_id: i64,
    dataset_id: i64,
    mut command: ParsedDocumentUploadCommand,
) -> Result<ParsedDocumentUploadCommand, AppError> {
    command.name = command.name.trim().to_owned();
    command.content_type = command.content_type.trim().to_owned();
    if command.content_type.is_empty() {
        command.content_type = DEFAULT_DOCUMENT_CONTENT_TYPE.to_owned();
    }
    command.parser_result.status = command.parser_result.status.trim().to_ascii_lowercase();
    if command.name.is_empty() {
        return Err(AppError::bad_request("文档名称不能为空"));
    }
    ensure_max_chars("文档名称", &command.name, 255)?;
    ensure_max_chars("内容类型", &command.content_type, 255)?;
    if command.parser_result.tenant_id != tenant_id {
        return Err(AppError::bad_request("解析结果租户不匹配"));
    }
    if command.parser_result.dataset_id != dataset_id {
        return Err(AppError::bad_request("解析结果知识库不匹配"));
    }
    if command.parser_result.document_id <= 0 {
        return Err(AppError::bad_request("解析结果文档 ID 不合法"));
    }
    if command.parser_result.parser_job_id <= 0 {
        return Err(AppError::bad_request("解析任务 ID 不合法"));
    }
    if command.parser_result.status != "succeeded" {
        return Err(AppError::bad_request("解析结果未成功"));
    }
    if command.parser_result.chunks.is_empty() {
        return Err(AppError::bad_request("解析结果 chunk 不能为空"));
    }
    for block in &mut command.parser_result.blocks {
        block.block_id = block.block_id.trim().to_owned();
        block.block_type = normalize_parser_block_type(&block.block_type);
        block.text = block.text.trim().to_owned();
        normalize_parser_string_list(&mut block.section_path);
    }

    let known_block_ids = command
        .parser_result
        .blocks
        .iter()
        .filter(|block| !block.block_id.is_empty())
        .map(|block| block.block_id.as_str())
        .collect::<HashSet<_>>();
    let mut chunk_uids = HashSet::new();
    let mut chunk_indexes = HashSet::new();

    for chunk in &mut command.parser_result.chunks {
        chunk.chunk_uid = chunk.chunk_uid.trim().to_owned();
        chunk.text = chunk.text.trim().to_owned();
        chunk.semantic_search_text = chunk.semantic_search_text.trim().to_owned();
        chunk.segment_type = chunk.segment_type.trim().to_ascii_lowercase();
        chunk.content_role = chunk.content_role.trim().to_ascii_lowercase();
        chunk.display_capability = chunk.display_capability.trim().to_ascii_lowercase();
        normalize_parser_string_list(&mut chunk.table_header);
        normalize_parser_string_list(&mut chunk.image_access_keys);
        chunk.citation.document_id = chunk.citation.document_id.trim().to_owned();
        chunk.citation.chunk_id = chunk.citation.chunk_id.trim().to_owned();
        normalize_parser_string_list(&mut chunk.citation.section_path);
        normalize_parser_string_list(&mut chunk.citation.block_ids);
        if chunk.chunk_uid.is_empty() {
            return Err(AppError::bad_request("解析结果 chunkUid 不能为空"));
        }
        if chunk.text.is_empty() {
            return Err(AppError::bad_request("解析结果 chunk 文本不能为空"));
        }
        if !chunk_uids.insert(chunk.chunk_uid.clone()) {
            return Err(AppError::bad_request("解析结果 chunkUid 重复"));
        }
        if !chunk_indexes.insert(chunk.chunk_index) {
            return Err(AppError::bad_request("解析结果 chunkIndex 重复"));
        }
        if !chunk.segment_type.is_empty()
            && parser_segment_type_value(&chunk.segment_type).is_none()
        {
            return Err(AppError::bad_request("解析结果 segmentType 不合法"));
        }
        if !chunk.content_role.is_empty()
            && parser_content_role_value(&chunk.content_role).is_none()
        {
            return Err(AppError::bad_request("解析结果 contentRole 不合法"));
        }
        if !chunk.display_capability.is_empty()
            && parser_display_capability_value(&chunk.display_capability).is_none()
        {
            return Err(AppError::bad_request("解析结果 displayCapability 不合法"));
        }
        for block_id in &chunk.citation.block_ids {
            if !known_block_ids.contains(block_id.as_str()) {
                return Err(AppError::bad_request(
                    "解析结果 blockIds 引用了不存在的 block",
                ));
            }
        }
    }
    Ok(command)
}

fn normalize_parser_string_list(items: &mut Vec<String>) {
    let mut seen = HashSet::new();
    *items = items
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .filter_map(|item| {
            if seen.insert(item.to_owned()) {
                Some(item.to_owned())
            } else {
                None
            }
        })
        .collect();
}

fn parser_result_chunks(
    command: &ParsedDocumentUploadCommand,
) -> Result<Vec<RagDocumentChunk>, AppError> {
    let result = &command.parser_result;
    let block_index = result
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| !block.block_id.is_empty())
        .map(|(index, block)| (block.block_id.as_str(), (index, block)))
        .collect::<HashMap<_, _>>();

    let mut chunks = Vec::with_capacity(result.chunks.len());
    for parser_chunk in &result.chunks {
        let referenced_blocks = parser_chunk
            .citation
            .block_ids
            .iter()
            .filter_map(|block_id| block_index.get(block_id.as_str()).copied())
            .collect::<Vec<_>>();
        let segment_type =
            parser_chunk_segment_type(&parser_chunk.segment_type, &referenced_blocks)?;
        let page_no = parser_chunk.citation.page_no.or_else(|| {
            referenced_blocks
                .iter()
                .find_map(|(_, block)| block.page_no)
        });
        let section_path = if parser_chunk.citation.section_path.is_empty() {
            referenced_blocks
                .iter()
                .find_map(|(_, block)| {
                    (!block.section_path.is_empty()).then(|| block.section_path.clone())
                })
                .unwrap_or_default()
        } else {
            parser_chunk.citation.section_path.clone()
        };
        let bbox = referenced_blocks
            .iter()
            .find_map(|(_, block)| block.bbox.as_ref())
            .and_then(parser_bbox);
        let table_header = if !parser_chunk.table_header.is_empty() {
            parser_chunk.table_header.clone()
        } else if segment_type == ChunkSegmentType::Table {
            parser_table_header(&parser_chunk.text)
        } else {
            vec![]
        };
        let image_access_keys = merge_parser_lists(
            &parser_chunk.image_access_keys,
            &parser_image_access_keys(&referenced_blocks),
        );
        let segment_index = referenced_blocks
            .first()
            .map(|(index, _)| *index)
            .unwrap_or(parser_chunk.chunk_index);
        let display_capability = parser_display_capability(
            &parser_chunk.display_capability,
            segment_type,
            page_no,
            bbox.as_ref(),
        )?;
        let metadata = ChunkMetadata {
            source_title: None,
            source_file_name: Some(command.name.clone()),
            source_content_type: Some(command.content_type.clone()),
            segment_type,
            segment_index,
            page_no,
            section_path: section_path.clone(),
            table_header,
            image_access_keys,
            bbox,
            content_role: parser_content_role(
                &parser_chunk.content_role,
                &section_path,
                &parser_chunk.text,
            )?,
            display_capability,
        };
        let semantic_input = non_empty_parser_string(&parser_chunk.semantic_search_text)
            .unwrap_or_else(|| parser_chunk.text.clone());
        let semantic_search_text = build_semantic_search_text(&semantic_input, &metadata);
        let token_count = if parser_chunk.token_count > 0 {
            parser_chunk.token_count
        } else {
            tokenish_count(&semantic_search_text).max(0) as usize
        };
        let citation = CitationRef {
            document_id: non_empty_parser_string(&parser_chunk.citation.document_id)
                .unwrap_or_else(|| result.document_id.to_string()),
            chunk_id: non_empty_parser_string(&parser_chunk.citation.chunk_id)
                .unwrap_or_else(|| parser_chunk.chunk_uid.clone()),
            page_no,
            section_path,
        };

        chunks.push(RagDocumentChunk {
            document_id: result.document_id.to_string(),
            chunk_id: parser_chunk.chunk_uid.clone(),
            chunk_index: parser_chunk.chunk_index,
            text: parser_chunk.text.clone(),
            semantic_search_text,
            token_count,
            citation,
            metadata,
        });
    }

    Ok(chunks)
}

fn parser_block_save_records(
    tenant_id: i64,
    dataset_id: i64,
    document_id: i64,
    command: &ParsedDocumentUploadCommand,
    user_id: i64,
    now: NaiveDateTime,
) -> Vec<BlockSaveRecord> {
    command
        .parser_result
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| !block.block_id.is_empty())
        .map(|(index, block)| BlockSaveRecord {
            id: next_id(),
            tenant_id,
            dataset_id,
            document_id,
            block_uid: block.block_id.clone(),
            block_index: index as i32,
            block_type: block.block_type.clone(),
            text: block.text.clone(),
            page_no: block.page_no,
            section_path: json!(block.section_path),
            bbox: block
                .bbox
                .as_ref()
                .map(parser_bbox_value)
                .unwrap_or_else(|| json!({})),
            metadata: json!({
                "parser": parser_name(&command.parser_result.metadata),
                "sourceFileName": command.name,
                "sourceContentType": command.content_type,
            }),
            user_id,
            now,
        })
        .collect()
}

fn parser_chunk_save_records(
    tenant_id: i64,
    dataset_id: i64,
    document_id: i64,
    command: &ParsedDocumentUploadCommand,
    chunks: Vec<RagDocumentChunk>,
    user_id: i64,
    now: NaiveDateTime,
) -> Vec<ChunkSaveRecord> {
    let parser_chunk_by_uid = command
        .parser_result
        .chunks
        .iter()
        .map(|chunk| (chunk.chunk_uid.as_str(), chunk))
        .collect::<HashMap<_, _>>();
    chunks
        .into_iter()
        .map(|chunk| {
            let metadata = chunk.metadata.clone();
            let chunk_uid = chunk.chunk_id;
            let semantic_search_text = chunk.semantic_search_text;
            let mut metadata_value = chunk_metadata_value(&metadata);
            if let Some(object) = metadata_value.as_object_mut() {
                object.insert(
                    "parser".to_owned(),
                    json!(parser_name(&command.parser_result.metadata)),
                );
                object.insert("parserChunkUid".to_owned(), json!(chunk_uid.clone()));
                object.insert(
                    "parserBlockIds".to_owned(),
                    json!(parser_chunk_by_uid
                        .get(chunk_uid.as_str())
                        .map(|parser_chunk| parser_chunk.citation.block_ids.clone())
                        .unwrap_or_default()),
                );
                if let Some(parser_chunk) = parser_chunk_by_uid.get(chunk_uid.as_str()) {
                    if !parser_chunk.metadata.is_null() {
                        object.insert(
                            "parserChunkMetadata".to_owned(),
                            parser_chunk.metadata.clone(),
                        );
                    }
                }
            }
            let vector = local_embedding_vector(&semantic_search_text);
            attach_embedding_metadata(
                &mut metadata_value,
                LOCAL_EMBEDDING_ROUTE,
                "local",
                &chunk_uid,
                &vector,
            );
            ChunkSaveRecord {
                id: next_id(),
                tenant_id,
                dataset_id,
                document_id,
                chunk_uid: chunk_uid.clone(),
                chunk_index: chunk.chunk_index as i32,
                content: chunk.text,
                semantic_search_text,
                token_count: chunk.token_count as i32,
                citation: citation_value(&chunk.citation),
                segment_type: metadata.segment_type.as_str().to_owned(),
                segment_index: metadata.segment_index as i32,
                page_no: metadata.page_no,
                section_path: json!(metadata.section_path),
                content_role: metadata.content_role.as_str().to_owned(),
                display_capability: metadata.display_capability.as_str().to_owned(),
                metadata: metadata_value,
                embedding_model: Some(LOCAL_EMBEDDING_ROUTE.to_owned()),
                embedding_ref: Some(embedding_ref(&chunk_uid)),
                embedding_status: CHUNK_EMBEDDING_STATUS_INDEXED,
                user_id,
                now,
            }
        })
        .collect()
}

fn parser_result_summary(
    command: &ParsedDocumentUploadCommand,
    chunks: &[RagDocumentChunk],
) -> Value {
    let mut segment_type_counts = serde_json::Map::new();
    let mut min_semantic_chars: Option<usize> = None;
    let mut max_semantic_chars = 0usize;
    for chunk in chunks {
        let key = chunk.metadata.segment_type.as_str();
        let count = segment_type_counts
            .get(key)
            .and_then(Value::as_u64)
            .unwrap_or(0)
            + 1;
        segment_type_counts.insert(key.to_owned(), json!(count));
        let semantic_chars = chunk.semantic_search_text.chars().count();
        min_semantic_chars = Some(
            min_semantic_chars
                .map(|current| current.min(semantic_chars))
                .unwrap_or(semantic_chars),
        );
        max_semantic_chars = max_semantic_chars.max(semantic_chars);
    }
    let metadata = &command.parser_result.metadata;
    json!({
        "parser": parser_name(metadata),
        "status": command.parser_result.status,
        "sourceFileName": command.name,
        "contentType": command.content_type,
        "pageCount": metadata.page_count,
        "lineCount": metadata.line_count,
        "sourceHash": metadata.source_hash,
        "warnings": metadata.warnings,
        "blockCount": command.parser_result.blocks.len(),
        "chunkCount": chunks.len(),
        "embeddingInput": "semanticSearchText",
        "segmentTypeCounts": Value::Object(segment_type_counts),
        "semanticSearchText": {
            "minChars": min_semantic_chars.unwrap_or(0),
            "maxChars": max_semantic_chars,
        }
    })
}

fn parsed_source_hash(command: &ParsedDocumentUploadCommand) -> Option<String> {
    non_empty_parser_string(
        command
            .parser_result
            .metadata
            .source_hash
            .as_deref()
            .unwrap_or(""),
    )
    .or_else(|| {
        let text = command
            .parser_result
            .chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        (!text.trim().is_empty()).then(|| sha256_hex(&text))
    })
}

fn parser_chunk_segment_type(
    explicit_value: &str,
    blocks: &[(usize, &ParserWorkerBlock)],
) -> Result<ChunkSegmentType, AppError> {
    if let Some(segment_type) = parser_segment_type_value(explicit_value) {
        return Ok(segment_type);
    }

    if blocks.iter().any(|(_, block)| block.block_type == "table") {
        Ok(ChunkSegmentType::Table)
    } else if blocks.iter().any(|(_, block)| block.block_type == "image") {
        Ok(ChunkSegmentType::Image)
    } else {
        Ok(ChunkSegmentType::Text)
    }
}

fn parser_segment_type_value(value: &str) -> Option<ChunkSegmentType> {
    match value.trim() {
        "" => None,
        "table" => Some(ChunkSegmentType::Table),
        "image" => Some(ChunkSegmentType::Image),
        "text" | "title" | "paragraph" | "list" | "code" | "formula" | "caption" => {
            Some(ChunkSegmentType::Text)
        }
        _ => None,
    }
}

fn merge_parser_lists(left: &[String], right: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    left.iter()
        .chain(right.iter())
        .filter_map(|item| {
            let item = item.trim();
            if item.is_empty() || !seen.insert(item.to_owned()) {
                None
            } else {
                Some(item.to_owned())
            }
        })
        .collect()
}

fn parser_table_header(text: &str) -> Vec<String> {
    text.lines()
        .next()
        .map(|line| {
            let delimiter = if line.contains('\t') {
                '\t'
            } else if line.contains('|') && !line.contains(',') {
                '|'
            } else {
                ','
            };
            line.split(delimiter)
                .map(|cell| cell.trim().trim_matches('|').to_owned())
                .filter(|cell| !cell.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn parser_image_access_keys(blocks: &[(usize, &ParserWorkerBlock)]) -> Vec<String> {
    blocks
        .iter()
        .filter(|(_, block)| block.block_type == "image")
        .filter_map(|(_, block)| {
            if block.block_id.starts_with("img/") || block.block_id.starts_with("image/") {
                Some(block.block_id.clone())
            } else {
                None
            }
        })
        .collect()
}

fn parser_content_role(
    explicit_value: &str,
    section_path: &[String],
    text: &str,
) -> Result<ContentRole, AppError> {
    if let Some(content_role) = parser_content_role_value(explicit_value) {
        return Ok(content_role);
    }

    Ok(infer_parser_content_role(section_path, text))
}

fn parser_content_role_value(value: &str) -> Option<ContentRole> {
    match value.trim() {
        "" => None,
        "canonical" => Some(ContentRole::Canonical),
        "summary_faq" => Some(ContentRole::SummaryFaq),
        "test_case" => Some(ContentRole::TestCase),
        _ => None,
    }
}

fn infer_parser_content_role(section_path: &[String], text: &str) -> ContentRole {
    let haystack = format!("{} {text}", section_path.join(" ")).to_ascii_lowercase();
    if haystack.contains("faq") || haystack.contains("问答") || haystack.contains("常见问题")
    {
        ContentRole::SummaryFaq
    } else if haystack.contains("test")
        || haystack.contains("测试")
        || haystack.contains("示例")
        || haystack.contains("example")
    {
        ContentRole::TestCase
    } else {
        ContentRole::Canonical
    }
}

fn parser_display_capability(
    explicit_value: &str,
    segment_type: ChunkSegmentType,
    page_no: Option<i32>,
    bbox: Option<&BoundingBox>,
) -> Result<DisplayCapability, AppError> {
    if let Some(display_capability) = parser_display_capability_value(explicit_value) {
        return Ok(display_capability);
    }

    Ok(infer_parser_display_capability(segment_type, page_no, bbox))
}

fn parser_display_capability_value(value: &str) -> Option<DisplayCapability> {
    match value.trim() {
        "" => None,
        "precise_anchor" => Some(DisplayCapability::PreciseAnchor),
        "row_only" => Some(DisplayCapability::RowOnly),
        "text_only" => Some(DisplayCapability::TextOnly),
        _ => None,
    }
}

fn infer_parser_display_capability(
    segment_type: ChunkSegmentType,
    page_no: Option<i32>,
    bbox: Option<&BoundingBox>,
) -> DisplayCapability {
    if page_no.is_some() || bbox.is_some() {
        DisplayCapability::PreciseAnchor
    } else if segment_type == ChunkSegmentType::Table {
        DisplayCapability::RowOnly
    } else {
        DisplayCapability::TextOnly
    }
}

fn normalize_parser_block_type(value: &str) -> String {
    match value.trim() {
        "title" | "paragraph" | "table" | "image" | "list" | "code" | "formula" | "caption"
        | "pageBreak" => value.trim().to_owned(),
        _ => "paragraph".to_owned(),
    }
}

fn parser_bbox(bbox: &ParserWorkerBbox) -> Option<BoundingBox> {
    Some(BoundingBox {
        x: f64_to_i32(bbox.x)?,
        y: f64_to_i32(bbox.y)?,
        width: f64_to_i32(bbox.width)?,
        height: f64_to_i32(bbox.height)?,
    })
}

fn parser_bbox_value(bbox: &ParserWorkerBbox) -> Value {
    parser_bbox(bbox)
        .map(|bbox| {
            json!({
                "x": bbox.x,
                "y": bbox.y,
                "width": bbox.width,
                "height": bbox.height,
            })
        })
        .unwrap_or_else(|| json!({}))
}

fn f64_to_i32(value: f64) -> Option<i32> {
    if !value.is_finite() || value < i32::MIN as f64 || value > i32::MAX as f64 {
        return None;
    }
    Some(value.round() as i32)
}

fn parser_name(metadata: &ParserWorkerMetadata) -> String {
    metadata
        .parser
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("parser-worker")
        .to_owned()
}

fn non_empty_parser_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

fn embedding_ref(chunk_uid: &str) -> String {
    format!("postgres-jsonb:{chunk_uid}")
}

fn local_embedding_vector(text: &str) -> Vec<f32> {
    let tokens = embedding_tokens(text);
    if tokens.is_empty() {
        return vec![0.0; LOCAL_EMBEDDING_DIMENSION];
    }

    let mut vector = vec![0.0f32; LOCAL_EMBEDDING_DIMENSION];
    for token in tokens {
        let hash = Sha256::digest(token.as_bytes());
        let bucket = (((hash[0] as usize) << 8) | hash[1] as usize) % LOCAL_EMBEDDING_DIMENSION;
        let sign = if hash[2] & 1 == 0 { 1.0 } else { -1.0 };
        vector[bucket] += sign;
    }
    normalize_embedding_vector(vector)
}

fn embedding_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut ascii_token = String::new();
    for character in text.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            ascii_token.push(character);
            continue;
        }
        if !ascii_token.is_empty() {
            tokens.push(std::mem::take(&mut ascii_token));
        }
        if is_cjk_character(character) {
            tokens.push(character.to_string());
        }
    }
    if !ascii_token.is_empty() {
        tokens.push(ascii_token);
    }
    tokens
}

fn is_cjk_character(character: char) -> bool {
    matches!(
        character as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF
    )
}

fn normalize_embedding_vector(mut vector: Vec<f32>) -> Vec<f32> {
    let norm = vector
        .iter()
        .map(|value| (*value as f64) * (*value as f64))
        .sum::<f64>()
        .sqrt();
    if norm <= f64::EPSILON {
        return vector;
    }
    for value in &mut vector {
        *value = (*value as f64 / norm) as f32;
    }
    vector
}

fn embedding_metadata(route_id: &str, source: &str, chunk_uid: &str, vector: &[f32]) -> Value {
    json!({
        "routeId": route_id,
        "source": source,
        "ref": embedding_ref(chunk_uid),
        "dimension": vector.len(),
        "vector": vector,
    })
}

fn attach_embedding_metadata(
    metadata: &mut Value,
    route_id: &str,
    source: &str,
    chunk_uid: &str,
    vector: &[f32],
) {
    if !metadata.is_object() {
        *metadata = json!({});
    }
    if let Some(object) = metadata.as_object_mut() {
        object.insert(
            "embedding".to_owned(),
            embedding_metadata(route_id, source, chunk_uid, vector),
        );
    }
}

fn apply_embedding_vectors_to_chunk_records(
    records: &mut [ChunkSaveRecord],
    route_id: &str,
    source: &str,
    vectors: &[ModelEmbeddingVector],
) {
    for vector in vectors {
        let Some(record) = records.get_mut(vector.index) else {
            continue;
        };
        record.embedding_model = Some(route_id.to_owned());
        record.embedding_ref = Some(embedding_ref(&record.chunk_uid));
        attach_embedding_metadata(
            &mut record.metadata,
            route_id,
            source,
            &record.chunk_uid,
            &vector.vector,
        );
    }
}

async fn enrich_chunk_records_with_runtime_embeddings(records: &mut [ChunkSaveRecord]) {
    let config = ModelRuntimeConfig::from_env();
    enrich_chunk_records_with_runtime_embeddings_from_config(records, &config).await;
}

async fn enrich_chunk_records_with_runtime_embeddings_from_config(
    records: &mut [ChunkSaveRecord],
    config: &ModelRuntimeConfig,
) {
    let Some(route) = config.route(ModelRuntimeTarget::Embedding) else {
        return;
    };
    if records.is_empty() {
        return;
    }

    let texts = records
        .iter()
        .map(|record| record.semantic_search_text.clone())
        .collect::<Vec<_>>();
    let Ok(vectors) = ModelRuntimeService::embed_texts(route, &texts).await else {
        return;
    };

    apply_embedding_vectors_to_chunk_records(
        records,
        &route.summary().route_id,
        "runtime",
        &vectors,
    );
}

fn chunk_save_records(
    tenant_id: i64,
    dataset_id: i64,
    document_id: i64,
    chunks: Vec<RagDocumentChunk>,
    user_id: i64,
    now: NaiveDateTime,
) -> Vec<ChunkSaveRecord> {
    chunks
        .into_iter()
        .map(|chunk| {
            let metadata = chunk.metadata.clone();
            let chunk_uid = chunk.chunk_id;
            let semantic_search_text = chunk.semantic_search_text;
            let vector = local_embedding_vector(&semantic_search_text);
            let mut metadata_value = chunk_metadata_value(&metadata);
            attach_embedding_metadata(
                &mut metadata_value,
                LOCAL_EMBEDDING_ROUTE,
                "local",
                &chunk_uid,
                &vector,
            );
            ChunkSaveRecord {
                id: next_id(),
                tenant_id,
                dataset_id,
                document_id,
                chunk_uid: chunk_uid.clone(),
                chunk_index: chunk.chunk_index as i32,
                content: chunk.text,
                semantic_search_text,
                token_count: chunk.token_count as i32,
                citation: citation_value(&chunk.citation),
                segment_type: metadata.segment_type.as_str().to_owned(),
                segment_index: metadata.segment_index as i32,
                page_no: metadata.page_no,
                section_path: json!(metadata.section_path),
                content_role: metadata.content_role.as_str().to_owned(),
                display_capability: metadata.display_capability.as_str().to_owned(),
                metadata: metadata_value,
                embedding_model: Some(LOCAL_EMBEDDING_ROUTE.to_owned()),
                embedding_ref: Some(embedding_ref(&chunk_uid)),
                embedding_status: CHUNK_EMBEDDING_STATUS_INDEXED,
                user_id,
                now,
            }
        })
        .collect()
}

fn indexed_rag_chunks(records: Vec<ChunkRecord>) -> Vec<IndexedRagChunk> {
    records
        .into_iter()
        .map(|record| {
            let citation =
                citation_from_value(record.document_id, &record.chunk_uid, &record.citation);
            let embedding_vector = embedding_vector_from_metadata(&record.metadata);
            let metadata = chunk_metadata_from_record(&record);
            IndexedRagChunk {
                chunk_db_id: record.id,
                document_id: record.document_id,
                embedding_vector,
                chunk: RagDocumentChunk {
                    document_id: record.document_id.to_string(),
                    chunk_id: record.chunk_uid,
                    chunk_index: record.chunk_index.max(0) as usize,
                    text: record.content,
                    semantic_search_text: record.semantic_search_text,
                    token_count: record.token_count.max(0) as usize,
                    citation,
                    metadata,
                },
            }
        })
        .collect()
}

#[cfg(test)]
fn hybrid_retrieve_indexed_chunks(
    question: &str,
    indexed_chunks: &[IndexedRagChunk],
    limit: usize,
) -> Vec<RetrievalHit> {
    let query_embeddings = vec![local_embedding_vector(question)];
    hybrid_retrieve_indexed_chunks_with_query_embeddings(
        question,
        indexed_chunks,
        limit,
        &query_embeddings,
    )
}

async fn hybrid_retrieve_indexed_chunks_with_milvus_or_local(
    question: &str,
    tenant_id: i64,
    dataset_id: i64,
    vector_collection: Option<&VectorCollectionRecord>,
    indexed_chunks: &[IndexedRagChunk],
    limit: usize,
) -> Result<Vec<RetrievalHit>, AppError> {
    hybrid_retrieve_indexed_chunks_with_milvus_or_local_strict(
        question,
        tenant_id,
        dataset_id,
        vector_collection,
        indexed_chunks,
        limit,
        strict_live_rag_required(),
    )
    .await
}

async fn hybrid_retrieve_indexed_chunks_with_milvus_or_local_strict(
    question: &str,
    tenant_id: i64,
    dataset_id: i64,
    vector_collection: Option<&VectorCollectionRecord>,
    indexed_chunks: &[IndexedRagChunk],
    limit: usize,
    strict: bool,
) -> Result<Vec<RetrievalHit>, AppError> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let milvus_config = MilvusSearchConfig::from_env();
    if strict && milvus_config.is_none() {
        return Err(AppError::bad_request("Milvus 环境变量未配置完整"));
    }
    if strict && vector_collection.is_none() {
        return Err(AppError::bad_request("Milvus collection 未就绪"));
    }

    let (query_embeddings, runtime_query_embedding) = retrieval_query_embeddings(question).await;
    if strict && runtime_query_embedding.is_none() {
        return Err(AppError::bad_request("Runtime Embedding 模型未配置或调用失败"));
    }

    let milvus_query_vector = runtime_query_embedding
        .clone()
        .or_else(|| query_embeddings.last().cloned());

    match (
        milvus_config,
        vector_collection,
        milvus_query_vector,
    ) {
        (Some(config), Some(collection), Some(query_vector)) => {
            let Some(request) = milvus_search_request_for_collection(
                tenant_id,
                dataset_id,
                collection,
                query_vector,
                limit,
            ) else {
                if strict {
                    return Err(AppError::bad_request("Milvus 检索请求无法构造"));
                }
                return Ok(hybrid_retrieve_indexed_chunks_with_query_embeddings(
                    question,
                    indexed_chunks,
                    limit,
                    &query_embeddings,
                ));
            };

            match milvus_search_hits(&config, &request).await {
                Ok(milvus_hits) => {
                    let embedding_hits =
                        retrieval_hits_from_milvus_hits(&milvus_hits, indexed_chunks, limit);
                    if !embedding_hits.is_empty() {
                        let rag_chunks = indexed_chunks
                            .iter()
                            .map(|chunk| chunk.chunk.clone())
                            .collect::<Vec<_>>();
                        let keyword_hits = keyword_retrieve(question, &rag_chunks, limit);
                        return Ok(merge_hybrid_retrieval_hits(
                            keyword_hits,
                            embedding_hits,
                            limit,
                        ));
                    }
                    if strict {
                        return Err(AppError::bad_request(
                            "Milvus 检索未返回可用的已索引 chunk",
                        ));
                    }
                }
                Err(err) if strict => return Err(err),
                Err(_) => {}
            }
        }
        (_, _, None) if strict => return Err(AppError::bad_request("Milvus 查询向量为空")),
        _ => {}
    }

    Ok(hybrid_retrieve_indexed_chunks_with_query_embeddings(
        question,
        indexed_chunks,
        limit,
        &query_embeddings,
    ))
}

async fn retrieval_query_embeddings(question: &str) -> (Vec<Vec<f32>>, Option<Vec<f32>>) {
    let mut query_embeddings = vec![local_embedding_vector(question)];
    if let Some(runtime_embedding) = runtime_query_embedding(question).await {
        let runtime_query_embedding = Some(runtime_embedding.clone());
        query_embeddings.push(runtime_embedding);
        return (query_embeddings, runtime_query_embedding);
    }
    (query_embeddings, None)
}

async fn runtime_query_embedding(question: &str) -> Option<Vec<f32>> {
    let config = ModelRuntimeConfig::from_env();
    let route = config.route(ModelRuntimeTarget::Embedding)?;
    let mut vectors = ModelRuntimeService::embed_texts(route, &[question.to_owned()])
        .await
        .ok()?;
    vectors.sort_by_key(|vector| vector.index);
    vectors.into_iter().next().map(|vector| vector.vector)
}

fn milvus_search_request_for_collection(
    tenant_id: i64,
    dataset_id: i64,
    collection: &VectorCollectionRecord,
    query_vector: Vec<f32>,
    limit: usize,
) -> Option<MilvusSearchRequest> {
    if limit == 0
        || query_vector.is_empty()
        || collection.status != VECTOR_COLLECTION_STATUS_READY
        || !collection
            .vector_backend
            .trim()
            .eq_ignore_ascii_case("milvus")
    {
        return None;
    }
    if collection.dimension > 0 && collection.dimension as usize != query_vector.len() {
        return None;
    }

    let provider_collection = collection.provider_collection.trim();
    if provider_collection.is_empty() {
        return None;
    }

    Some(
        MilvusSearchRequest::new(
            provider_collection,
            query_vector,
            limit,
            tenant_id,
            dataset_id,
        )
        .with_metric_type(milvus_metric_type(&collection.metric_type)),
    )
}

fn milvus_upsert_request_for_collection(
    collection: &VectorCollectionRecord,
    chunks: &[ChunkSaveRecord],
) -> Option<MilvusUpsertRequest> {
    if collection.status != VECTOR_COLLECTION_STATUS_READY
        || !collection
            .vector_backend
            .trim()
            .eq_ignore_ascii_case("milvus")
    {
        return None;
    }

    let provider_collection = collection.provider_collection.trim();
    if provider_collection.is_empty() {
        return None;
    }

    let rows = chunks
        .iter()
        .filter_map(|chunk| milvus_upsert_row_for_chunk(collection, chunk))
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return None;
    }

    Some(MilvusUpsertRequest::new(provider_collection, rows))
}

fn milvus_upsert_row_for_chunk(
    collection: &VectorCollectionRecord,
    chunk: &ChunkSaveRecord,
) -> Option<MilvusUpsertRow> {
    let embedding = chunk_save_record_embedding_vector(chunk)?;
    if collection.dimension > 0 && collection.dimension as usize != embedding.len() {
        return None;
    }

    Some(MilvusUpsertRow {
        id: chunk.id,
        tenant_id: chunk.tenant_id,
        dataset_id: chunk.dataset_id,
        document_id: chunk.document_id,
        chunk_uid: chunk.chunk_uid.clone(),
        chunk_index: chunk.chunk_index,
        embedding,
        semantic_search_text: non_empty_parser_string(&chunk.semantic_search_text)
            .unwrap_or_else(|| chunk.content.clone()),
        segment_type: chunk.segment_type.clone(),
        content_role: chunk.content_role.clone(),
    })
}

fn chunk_save_record_embedding_vector(chunk: &ChunkSaveRecord) -> Option<Vec<f32>> {
    let vector = chunk
        .metadata
        .get("embedding")?
        .get("vector")?
        .as_array()?
        .iter()
        .filter_map(json_value_f32)
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if vector.is_empty() {
        None
    } else {
        Some(vector)
    }
}

fn milvus_metric_type(metric_type: &str) -> MilvusMetricType {
    match metric_type.trim().to_ascii_lowercase().as_str() {
        "ip" | "inner_product" => MilvusMetricType::Ip,
        "l2" | "euclidean" => MilvusMetricType::L2,
        _ => MilvusMetricType::Cosine,
    }
}

async fn milvus_search_hits(
    config: &MilvusSearchConfig,
    request: &MilvusSearchRequest,
) -> Result<Vec<MilvusSearchHit>, AppError> {
    let client = milvus_http_client()?;
    let mut builder = client
        .post(config.search_url())
        .json(&request.to_rest_search_body());
    if let Some(token) = config.token.as_deref() {
        builder = builder.bearer_auth(token);
    }

    let response = builder
        .send()
        .await
        .map_err(|err| AppError::bad_request(format!("Milvus 检索请求失败: {err}")))?;
    if !response.status().is_success() {
        return Err(AppError::bad_request(format!(
            "Milvus 检索请求失败: HTTP {}",
            response.status()
        )));
    }

    let payload = response
        .json::<Value>()
        .await
        .map_err(|err| AppError::bad_request(format!("Milvus 检索响应解析失败: {err}")))?;
    if let Some(code) = payload.get("code").and_then(Value::as_i64) {
        if code != 0 {
            let message = payload
                .get("message")
                .or_else(|| payload.get("msg"))
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            return Err(AppError::bad_request(format!("Milvus 检索失败: {message}")));
        }
    }

    Ok(parse_milvus_search_hits(&payload))
}

async fn milvus_upsert_chunks(
    config: &MilvusSearchConfig,
    request: &MilvusUpsertRequest,
) -> Result<(), AppError> {
    if request.is_empty() {
        return Ok(());
    }

    let client = milvus_http_client()?;
    let mut builder = client
        .post(config.upsert_url())
        .json(&request.to_rest_upsert_body());
    if let Some(token) = config.token.as_deref() {
        builder = builder.bearer_auth(token);
    }

    let response = builder
        .send()
        .await
        .map_err(|err| AppError::bad_request(format!("Milvus 写入请求失败: {err}")))?;
    if !response.status().is_success() {
        return Err(AppError::bad_request(format!(
            "Milvus 写入请求失败: HTTP {}",
            response.status()
        )));
    }

    let payload = response
        .json::<Value>()
        .await
        .map_err(|err| AppError::bad_request(format!("Milvus 写入响应解析失败: {err}")))?;
    if let Some(code) = payload.get("code").and_then(Value::as_i64) {
        if code != 0 {
            let message = payload
                .get("message")
                .or_else(|| payload.get("msg"))
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            return Err(AppError::bad_request(format!("Milvus 写入失败: {message}")));
        }
    }

    Ok(())
}

fn milvus_http_client() -> Result<reqwest::Client, AppError> {
    reqwest::Client::builder()
        .timeout(MILVUS_SEARCH_TIMEOUT)
        .build()
        .map_err(|err| AppError::bad_request(format!("Milvus 客户端初始化失败: {err}")))
}

fn hybrid_retrieve_indexed_chunks_with_query_embeddings(
    question: &str,
    indexed_chunks: &[IndexedRagChunk],
    limit: usize,
    query_embeddings: &[Vec<f32>],
) -> Vec<RetrievalHit> {
    if limit == 0 {
        return Vec::new();
    }

    let rag_chunks = indexed_chunks
        .iter()
        .map(|chunk| chunk.chunk.clone())
        .collect::<Vec<_>>();
    let keyword_hits = keyword_retrieve(question, &rag_chunks, limit);
    let embedding_hits = embedding_retrieve_indexed_chunks(query_embeddings, indexed_chunks, limit);

    merge_hybrid_retrieval_hits(keyword_hits, embedding_hits, limit)
}

fn embedding_vector_from_metadata(metadata: &Value) -> Option<Vec<f32>> {
    let vector = metadata
        .get("embedding")?
        .get("vector")?
        .as_array()?
        .iter()
        .filter_map(json_value_f32)
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if vector.is_empty() {
        None
    } else {
        Some(vector)
    }
}

fn json_value_f32(value: &Value) -> Option<f32> {
    if let Some(value) = value.as_f64() {
        return Some(value as f32);
    }
    value.as_str().and_then(|text| text.parse::<f32>().ok())
}

fn embedding_retrieve_indexed_chunks(
    query_embeddings: &[Vec<f32>],
    indexed_chunks: &[IndexedRagChunk],
    limit: usize,
) -> Vec<RetrievalHit> {
    if query_embeddings.is_empty() || limit == 0 {
        return Vec::new();
    }

    let mut scored = indexed_chunks
        .iter()
        .filter_map(|indexed_chunk| {
            let vector = indexed_chunk.embedding_vector.as_deref()?;
            let score = query_embeddings
                .iter()
                .filter_map(|query_embedding| cosine_similarity(query_embedding, vector))
                .max_by(|left, right| {
                    left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
                })?;
            if score <= 0.0 {
                return None;
            }
            Some((score, indexed_chunk.chunk.clone()))
        })
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.1.chunk_index.cmp(&right.1.chunk_index))
    });

    scored
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(index, (score, chunk))| RetrievalHit {
            rank: index + 1,
            score,
            citation: chunk.citation.clone(),
            chunk,
        })
        .collect()
}

fn retrieval_hits_from_milvus_hits(
    hits: &[MilvusSearchHit],
    indexed_chunks: &[IndexedRagChunk],
    limit: usize,
) -> Vec<RetrievalHit> {
    if limit == 0 {
        return Vec::new();
    }

    let chunk_by_uid = indexed_chunks
        .iter()
        .map(|chunk| (chunk.chunk.chunk_id.as_str(), chunk))
        .collect::<HashMap<_, _>>();

    hits.iter()
        .filter_map(|hit| {
            let indexed_chunk = chunk_by_uid.get(hit.chunk_uid.as_str())?;
            Some(RetrievalHit {
                rank: 0,
                score: hit.score,
                citation: indexed_chunk.chunk.citation.clone(),
                chunk: indexed_chunk.chunk.clone(),
            })
        })
        .take(limit)
        .enumerate()
        .map(|(index, mut hit)| {
            hit.rank = index + 1;
            hit
        })
        .collect()
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }
    let mut dot = 0.0f32;
    let mut left_norm = 0.0f32;
    let mut right_norm = 0.0f32;
    for (left_value, right_value) in left.iter().zip(right.iter()) {
        dot += left_value * right_value;
        left_norm += left_value * left_value;
        right_norm += right_value * right_value;
    }
    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        return None;
    }
    Some(dot / (left_norm.sqrt() * right_norm.sqrt()))
}

fn merge_hybrid_retrieval_hits(
    keyword_hits: Vec<RetrievalHit>,
    embedding_hits: Vec<RetrievalHit>,
    limit: usize,
) -> Vec<RetrievalHit> {
    let mut merged = HashMap::<String, (RetrievalHit, f32)>::new();

    for hit in keyword_hits {
        let key = hit.chunk.chunk_id.clone();
        let weighted_score = hit.score * 0.65;
        merged
            .entry(key)
            .and_modify(|(_, score)| *score += weighted_score)
            .or_insert((hit, weighted_score));
    }

    for hit in embedding_hits {
        let key = hit.chunk.chunk_id.clone();
        let weighted_score = hit.score * 0.35;
        merged
            .entry(key)
            .and_modify(|(_, score)| *score += weighted_score)
            .or_insert((hit, weighted_score));
    }

    let mut scored = merged
        .into_values()
        .map(|(mut hit, score)| {
            hit.score = score;
            hit
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.chunk.chunk_index.cmp(&right.chunk.chunk_index))
    });
    scored.truncate(limit);
    rank_retrieval_hits(scored)
}

fn indexed_retrieval_hits(
    hits: &[RetrievalHit],
    indexed_chunks: &[IndexedRagChunk],
) -> Vec<IndexedRetrievalHit> {
    let chunk_by_uid = indexed_chunks
        .iter()
        .map(|chunk| (chunk.chunk.chunk_id.as_str(), chunk))
        .collect::<HashMap<_, _>>();

    hits.iter()
        .filter_map(|hit| {
            chunk_by_uid
                .get(hit.chunk.chunk_id.as_str())
                .map(|indexed_chunk| IndexedRetrievalHit {
                    chunk_db_id: indexed_chunk.chunk_db_id,
                    document_id: indexed_chunk.document_id,
                    rank: hit.rank as i32,
                    score: hit.score,
                    citation: hit.citation.clone(),
                    content: hit.chunk.text.clone(),
                    token_count: hit.chunk.token_count as i32,
                })
        })
        .collect()
}

async fn rerank_dataset_hits(
    question: &str,
    hits: Vec<RetrievalHit>,
    limit: usize,
) -> Result<Vec<RetrievalHit>, AppError> {
    if hits.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let config = ModelRuntimeConfig::from_env();
    let strict = strict_live_rag_required();
    let Some(route) = rerank_route_for_mode(&config, strict)? else {
        return Ok(rerank_retrieval_hits(&hits, &[], limit));
    };
    let documents = hits.iter().map(rerank_document_text).collect::<Vec<_>>();

    match ModelRuntimeService::rerank_documents(route, question, &documents).await {
        Ok(scores) if !scores.is_empty() => Ok(rerank_retrieval_hits(&hits, &scores, limit)),
        Ok(_) if strict => Err(AppError::bad_request("Rerank 模型响应为空")),
        Err(err) if strict => Err(err),
        _ => Ok(rerank_retrieval_hits(&hits, &[], limit)),
    }
}

fn rerank_route_for_mode<'a>(
    config: &'a ModelRuntimeConfig,
    strict: bool,
) -> Result<Option<&'a novex_model::ModelRuntimeRoute>, AppError> {
    match config.route(ModelRuntimeTarget::Reranker) {
        Some(route) => Ok(Some(route)),
        None if strict => Err(AppError::bad_request("Rerank 模型环境变量未配置完整")),
        None => Ok(None),
    }
}

fn rerank_retrieval_hits(
    hits: &[RetrievalHit],
    scores: &[ModelRerankScore],
    limit: usize,
) -> Vec<RetrievalHit> {
    if limit == 0 {
        return Vec::new();
    }
    if scores.is_empty() {
        return rank_retrieval_hits(hits.iter().take(limit).cloned().collect());
    }

    let mut used = HashSet::new();
    let mut sorted_scores = scores.to_vec();
    sorted_scores.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.index.cmp(&right.index))
    });
    let mut ordered = sorted_scores
        .into_iter()
        .filter_map(|score| {
            if !used.insert(score.index) {
                return None;
            }
            let mut hit = hits.get(score.index)?.clone();
            hit.score = score.score;
            Some(hit)
        })
        .collect::<Vec<_>>();

    for (index, hit) in hits.iter().enumerate() {
        if used.insert(index) {
            ordered.push(hit.clone());
        }
    }

    ordered.truncate(limit);
    rank_retrieval_hits(ordered)
}

fn rank_retrieval_hits(mut hits: Vec<RetrievalHit>) -> Vec<RetrievalHit> {
    for (index, hit) in hits.iter_mut().enumerate() {
        hit.rank = index + 1;
    }
    hits
}

fn rerank_document_text(hit: &RetrievalHit) -> String {
    non_empty_parser_string(&hit.chunk.semantic_search_text)
        .unwrap_or_else(|| hit.chunk.text.clone())
}

fn rerank_candidate_limit(limit: usize) -> usize {
    limit
        .saturating_mul(RERANK_CANDIDATE_MULTIPLIER)
        .clamp(limit, MAX_RERANK_CANDIDATES)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RagAnswerMode {
    UseLlm,
    RequireLlm,
    ExtractiveFallback,
}

fn rag_answer_mode_from_config(config: &ModelRuntimeConfig, strict: bool) -> RagAnswerMode {
    if config.route(ModelRuntimeTarget::Llm).is_some() {
        RagAnswerMode::UseLlm
    } else if strict {
        RagAnswerMode::RequireLlm
    } else {
        RagAnswerMode::ExtractiveFallback
    }
}

async fn generate_rag_answer(question: &str, hits: &[RetrievalHit]) -> Result<RagAnswer, AppError> {
    let config = ModelRuntimeConfig::from_env();
    match rag_answer_mode_from_config(&config, strict_live_rag_required()) {
        RagAnswerMode::UseLlm => {
            let chat = ModelRuntimeService::chat_completion(rag_answer_chat_command(
                question,
                hits,
                DEFAULT_RAG_ANSWER_MAX_TOKENS,
            ))
            .await?;
            let answer = rag_answer_from_model_chat(chat, hits);
            if answer.answer.trim().is_empty() {
                return Err(AppError::bad_request("LLM RAG 回答为空"));
            }
            Ok(answer)
        }
        RagAnswerMode::RequireLlm => Err(AppError::bad_request("LLM 模型环境变量未配置完整")),
        RagAnswerMode::ExtractiveFallback => Ok(build_extractive_answer(question, hits)),
    }
}

fn strict_live_rag_required() -> bool {
    parse_bool_env_flag(env::var("NOVEX_LIVE_RAG_TEST").ok().as_deref())
        || parse_bool_env_flag(env::var("NOVEX_REQUIRE_LIVE_RAG").ok().as_deref())
        || parse_bool_env_flag(env::var("NOVEX_REQUIRE_MILVUS").ok().as_deref())
}

fn parse_bool_env_flag(value: Option<&str>) -> bool {
    matches!(
        value.map(|value| value.trim().to_ascii_lowercase()),
        Some(value) if matches!(value.as_str(), "1" | "true" | "yes" | "on")
    )
}

fn rag_answer_chat_command(
    question: &str,
    hits: &[RetrievalHit],
    max_tokens: u32,
) -> ModelChatCommand {
    ModelChatCommand {
        conversation_id: None,
        messages: vec![
            ModelChatMessage {
                role: "system".to_owned(),
                content: "Only answer from the provided context. If the context does not contain enough evidence, say that the answer is not available in the provided context. Keep the answer concise and grounded. Do not invent citations."
                    .to_owned(),
            },
            ModelChatMessage {
                role: "user".to_owned(),
                content: rag_context_message(question, hits),
            },
        ],
        file_contexts: vec![],
        temperature: Some(0.2),
        max_tokens: Some(max_tokens),
    }
}

fn rag_context_message(question: &str, hits: &[RetrievalHit]) -> String {
    let mut message = format!("Question:\n{}\n\nContext:\n", question.trim());
    if hits.is_empty() {
        message.push_str("(no retrieved context)");
        return message;
    }

    for (index, hit) in hits.iter().enumerate() {
        let label = index + 1;
        let chunk_id = hit.citation.chunk_id.trim();
        let section = if hit.citation.section_path.is_empty() {
            String::new()
        } else {
            format!(" section={}", hit.citation.section_path.join(" > "))
        };
        let page = hit
            .citation
            .page_no
            .map(|page_no| format!(" page={page_no}"))
            .unwrap_or_default();
        message.push_str(&format!(
            "[{label}] {chunk_id}{page}{section}\n{}\n\n",
            hit.chunk.text.trim()
        ));
    }

    message.push_str("Answer with the facts supported by the context above.");
    message
}

fn rag_answer_from_model_chat(chat: ModelChatResp, hits: &[RetrievalHit]) -> RagAnswer {
    RagAnswer {
        answer: chat.answer.trim().to_owned(),
        citations: citations_from_retrieval_hits(hits),
        trace: novex_rag::RagTraceSnapshot {
            retrieval_hit_count: hits.len(),
            answer_strategy: "llm_grounded".to_owned(),
        },
    }
}

fn citations_from_retrieval_hits(hits: &[RetrievalHit]) -> Vec<CitationRef> {
    let mut seen = HashSet::new();
    hits.iter()
        .filter_map(|hit| {
            if seen.insert(hit.citation.chunk_id.clone()) {
                Some(hit.citation.clone())
            } else {
                None
            }
        })
        .collect()
}

fn rag_trace_record(
    trace_id: i64,
    tenant_id: i64,
    user_id: i64,
    dataset_id: i64,
    command: &RagAskCommand,
    answer: &RagAnswer,
    hits: &[IndexedRetrievalHit],
    model_routes: &RagModelRoutes,
    now: NaiveDateTime,
) -> RagTraceSaveRecord {
    RagTraceSaveRecord {
        id: trace_id,
        tenant_id,
        dataset_id,
        question: command.question.clone(),
        answer: answer.answer.clone(),
        answer_strategy: answer.trace.answer_strategy.clone(),
        retrieval_mode: RETRIEVAL_MODE_HYBRID,
        embedding_model_route: Some(model_routes.embedding_model_route.clone()),
        rerank_model_route: Some(model_routes.rerank_model_route.clone()),
        answer_model_route: Some(model_routes.answer_model_route.clone()),
        retrieval_hit_count: hits.len() as i32,
        context_token_count: hits.iter().map(|hit| hit.token_count).sum(),
        output_token_count: tokenish_count(&answer.answer),
        user_id,
        now,
    }
}

fn rag_model_routes() -> RagModelRoutes {
    rag_model_routes_from_config(&ModelRuntimeConfig::from_env())
}

fn rag_model_routes_from_config(config: &ModelRuntimeConfig) -> RagModelRoutes {
    RagModelRoutes::from_runtime_config(config)
}

fn rag_trace_hit_records(
    trace_id: i64,
    tenant_id: i64,
    dataset_id: i64,
    hits: &[IndexedRetrievalHit],
    now: NaiveDateTime,
) -> Vec<RagTraceHitSaveRecord> {
    hits.iter()
        .map(|hit| RagTraceHitSaveRecord {
            id: next_id(),
            tenant_id,
            trace_id,
            dataset_id,
            document_id: hit.document_id,
            chunk_id: hit.chunk_db_id,
            rank: hit.rank,
            score: hit.score,
            citation: citation_value(&hit.citation),
            content_preview: preview_text(&hit.content),
            now,
        })
        .collect()
}

fn rag_ask_response(trace_id: i64, answer: RagAnswer) -> RagAskResp {
    RagAskResp {
        trace_id,
        answer: answer.answer,
        citations: answer
            .citations
            .into_iter()
            .map(CitationResp::from)
            .collect(),
        retrieval_hit_count: answer.trace.retrieval_hit_count,
        answer_strategy: answer.trace.answer_strategy,
    }
}

fn citation_value(citation: &novex_rag::CitationRef) -> Value {
    json!({
        "documentId": citation.document_id,
        "chunkId": citation.chunk_id,
        "pageNo": citation.page_no,
        "sectionPath": citation.section_path,
    })
}

fn chunk_metadata_value(metadata: &novex_rag::ChunkMetadata) -> Value {
    serde_json::to_value(metadata).unwrap_or_else(|_| json!({}))
}

fn citation_from_value(document_id: i64, chunk_uid: &str, value: &Value) -> CitationRef {
    let document_id = value
        .get("documentId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| document_id.to_string());
    let chunk_id = value
        .get("chunkId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| chunk_uid.to_owned());
    let page_no = value
        .get("pageNo")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok());
    let section_path = value
        .get("sectionPath")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    CitationRef {
        document_id,
        chunk_id,
        page_no,
        section_path,
    }
}

fn chunk_metadata_from_record(record: &ChunkRecord) -> novex_rag::ChunkMetadata {
    novex_rag::ChunkMetadata {
        source_title: metadata_string(&record.metadata, "sourceTitle"),
        source_file_name: metadata_string(&record.metadata, "sourceFileName"),
        source_content_type: metadata_string(&record.metadata, "sourceContentType"),
        segment_type: segment_type_from_str(&record.segment_type),
        segment_index: record.segment_index.max(0) as usize,
        page_no: record.page_no.or_else(|| {
            record
                .metadata
                .get("pageNo")
                .and_then(Value::as_i64)
                .and_then(|value| i32::try_from(value).ok())
        }),
        section_path: string_array_from_value(&record.section_path).unwrap_or_else(|| {
            record
                .metadata
                .get("sectionPath")
                .and_then(string_array_from_value)
                .unwrap_or_default()
        }),
        table_header: record
            .metadata
            .get("tableHeader")
            .and_then(string_array_from_value)
            .unwrap_or_default(),
        image_access_keys: record
            .metadata
            .get("imageAccessKeys")
            .and_then(string_array_from_value)
            .unwrap_or_default(),
        bbox: record.metadata.get("bbox").and_then(bbox_from_value),
        content_role: content_role_from_str(&record.content_role),
        display_capability: display_capability_from_str(&record.display_capability),
    }
}

fn metadata_string(metadata: &Value, key: &str) -> Option<String> {
    metadata.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn string_array_from_value(value: &Value) -> Option<Vec<String>> {
    value.as_array().map(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    })
}

fn bbox_from_value(value: &Value) -> Option<novex_rag::BoundingBox> {
    let object = value.as_object()?;
    Some(novex_rag::BoundingBox {
        x: json_i32(object.get("x"))?,
        y: json_i32(object.get("y"))?,
        width: json_i32(object.get("width"))?,
        height: json_i32(object.get("height"))?,
    })
}

fn json_i32(value: Option<&Value>) -> Option<i32> {
    value
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
}

fn segment_type_from_str(value: &str) -> novex_rag::ChunkSegmentType {
    match value {
        "table" => novex_rag::ChunkSegmentType::Table,
        "image" => novex_rag::ChunkSegmentType::Image,
        _ => novex_rag::ChunkSegmentType::Text,
    }
}

fn content_role_from_str(value: &str) -> novex_rag::ContentRole {
    match value {
        "summary_faq" => novex_rag::ContentRole::SummaryFaq,
        "test_case" => novex_rag::ContentRole::TestCase,
        _ => novex_rag::ContentRole::Canonical,
    }
}

fn display_capability_from_str(value: &str) -> novex_rag::DisplayCapability {
    match value {
        "precise_anchor" => novex_rag::DisplayCapability::PreciseAnchor,
        "row_only" => novex_rag::DisplayCapability::RowOnly,
        _ => novex_rag::DisplayCapability::TextOnly,
    }
}

impl From<CitationRef> for CitationResp {
    fn from(citation: CitationRef) -> Self {
        Self {
            document_id: citation.document_id,
            chunk_id: citation.chunk_id,
            page_no: citation.page_no,
            section_path: citation.section_path,
        }
    }
}

fn sha256_hex(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn tokenish_count(text: &str) -> i32 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }
    let whitespace_count = trimmed.split_whitespace().count();
    if whitespace_count > 1 {
        whitespace_count as i32
    } else {
        trimmed.chars().count() as i32
    }
}

fn preview_text(text: &str) -> String {
    text.chars().take(240).collect()
}

fn dataset_save_record<'a>(
    tenant_id: i64,
    id: i64,
    user_id: i64,
    command: &'a DatasetCommand,
) -> DatasetSaveRecord<'a> {
    DatasetSaveRecord {
        id,
        tenant_id,
        name: &command.name,
        description: non_empty(&command.description),
        owner_id: user_id,
        visibility: command.visibility,
        status: DATASET_STATUS_DRAFT,
        retrieval_mode: command.retrieval_mode,
        user_id,
        now: Utc::now().naive_utc(),
    }
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_size() -> u64 {
    DEFAULT_PAGE_SIZE
}

fn default_visibility() -> i16 {
    VISIBILITY_PRIVATE
}

fn default_retrieval_mode() -> i16 {
    RETRIEVAL_MODE_HYBRID
}

fn default_document_content_type() -> String {
    DEFAULT_DOCUMENT_CONTENT_TYPE.to_owned()
}

fn default_rag_limit() -> usize {
    DEFAULT_RAG_LIMIT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ai::model_service::{ModelChatResp, ModelChatUsage};

    #[test]
    fn normalize_dataset_command_rejects_empty_name() {
        let command = DatasetCommand {
            name: "   ".to_owned(),
            ..DatasetCommand::default()
        };

        let err = normalize_dataset_command(command).unwrap_err();

        assert!(err.to_string().contains("名称不能为空"));
    }

    #[test]
    fn normalize_dataset_command_trims_text_and_applies_defaults() {
        let command = DatasetCommand {
            name: "  员工手册  ".to_owned(),
            description: "  入职培训  ".to_owned(),
            ..DatasetCommand::default()
        };

        let command = normalize_dataset_command(command).unwrap();

        assert_eq!(command.name, "员工手册");
        assert_eq!(command.description, "入职培训");
        assert_eq!(command.visibility, 1);
        assert_eq!(command.retrieval_mode, 3);
    }

    #[test]
    fn dataset_query_normalizes_pagination() {
        let query = DatasetQuery {
            page: 0,
            size: u64::MAX,
            ..DatasetQuery::default()
        };
        let page = query.page_query();

        assert_eq!(page.offset(), 0);
        assert_eq!(page.limit(), 100);
    }

    #[test]
    fn normalize_document_upload_rejects_empty_content() {
        let command = DocumentUploadCommand {
            name: "手册".to_owned(),
            content: "   ".to_owned(),
            ..DocumentUploadCommand::default()
        };

        let err = normalize_document_upload_command(command).unwrap_err();

        assert!(err.to_string().contains("文档内容不能为空"));
    }

    #[test]
    fn normalize_document_upload_trims_metadata_and_defaults_content_type() {
        let command = DocumentUploadCommand {
            name: "  入职手册.md  ".to_owned(),
            content: "  入职培训第一天开始。  ".to_owned(),
            ..DocumentUploadCommand::default()
        };

        let command = normalize_document_upload_command(command).unwrap();

        assert_eq!(command.name, "入职手册.md");
        assert_eq!(command.content, "入职培训第一天开始。");
        assert_eq!(command.content_type, "text/plain");
    }

    #[test]
    fn document_upload_chunks_are_stable_for_document_id() {
        let command = DocumentUploadCommand {
            name: "handbook.txt".to_owned(),
            content: "Alpha beta gamma delta epsilon zeta eta theta. ".repeat(40),
            ..DocumentUploadCommand::default()
        };
        let command = normalize_document_upload_command(command).unwrap();

        let chunks = document_upload_chunks(42, &command);

        assert!(chunks.len() > 1);
        assert_eq!(chunks[0].document_id, "42");
        assert_eq!(chunks[0].chunk_id, "42:0");
        assert_eq!(chunks[0].chunk_index, 0);
    }

    #[test]
    fn document_upload_chunks_apply_file_type_chunker_and_search_metadata() {
        let command = DocumentUploadCommand {
            name: "training.csv".to_owned(),
            content: "employee,deadline,status\nAlice,Friday,done\nBob,Monday,pending".to_owned(),
            content_type: "text/csv".to_owned(),
        };
        let command = normalize_document_upload_command(command).unwrap();

        let chunks = document_upload_chunks(42, &command);

        assert!(!chunks.is_empty());
        assert_eq!(
            chunks[0].metadata.segment_type,
            novex_rag::ChunkSegmentType::Table
        );
        assert_eq!(
            chunks[0].metadata.table_header,
            vec!["employee", "deadline", "status"]
        );
        assert!(chunks[0].semantic_search_text.contains("training.csv"));
        assert!(chunks[0].semantic_search_text.contains("deadline"));
    }

    #[test]
    fn normalize_parse_job_command_builds_parser_worker_request_for_uploaded_file() {
        let command = DocumentParseJobCommand {
            name: " training.pdf ".to_owned(),
            file_id: Some(88),
            source_uri: " /uploads/training.pdf ".to_owned(),
            content_type: " ".to_owned(),
            source_hash: " abc123 ".to_owned(),
            source_kind: " ".to_owned(),
        };

        let command = normalize_document_parse_job_command(command).unwrap();
        let request = parser_worker_request(DEFAULT_TENANT_ID, 7, 42, 99, &command);

        assert_eq!(command.name, "training.pdf");
        assert_eq!(command.content_type, "application/pdf");
        assert_eq!(command.source_kind, "objectStorage");
        assert_eq!(request["tenantId"], 1);
        assert_eq!(request["datasetId"], 7);
        assert_eq!(request["documentId"], 42);
        assert_eq!(request["parserJobId"], 99);
        assert_eq!(request["source"]["kind"], "objectStorage");
        assert_eq!(request["source"]["fileId"], 88);
        assert_eq!(request["source"]["uri"], "/uploads/training.pdf");
        assert_eq!(request["source"]["sourceHash"], "abc123");
        assert_eq!(request["options"]["maxChunkChars"], DEFAULT_CHUNK_MAX_CHARS);
        assert_eq!(
            request["options"]["chunkOverlapChars"],
            DEFAULT_CHUNK_OVERLAP_CHARS
        );
        assert_eq!(request["trace"]["requestId"], "parser-job-99");
    }

    #[test]
    fn normalize_parse_job_command_rejects_missing_source() {
        let err = normalize_document_parse_job_command(DocumentParseJobCommand {
            name: "training.pdf".to_owned(),
            ..DocumentParseJobCommand::default()
        })
        .unwrap_err();

        assert!(err.to_string().contains("文件来源不能为空"));
    }

    #[test]
    fn parse_job_command_from_uploaded_file_uses_asset_metadata() {
        let file = crate::application::system::file_service::FileResp {
            id: 88,
            name: "88.pdf".to_owned(),
            original_name: "员工手册.pdf".to_owned(),
            size: 1024,
            url: "/file/knowledge/88.pdf".to_owned(),
            parent_path: "/knowledge".to_owned(),
            path: "/knowledge/88.pdf".to_owned(),
            sha256: "file-hash".to_owned(),
            content_type: "application/pdf".to_owned(),
            metadata: "{}".to_owned(),
            thumbnail_size: 0,
            thumbnail_name: String::new(),
            thumbnail_metadata: String::new(),
            thumbnail_url: String::new(),
            extension: "pdf".to_owned(),
            file_type: 4,
            storage_id: 1,
            storage_name: "本地".to_owned(),
            create_user_string: "admin".to_owned(),
            create_time: "2026-06-05 10:00:00".to_owned(),
            update_user_string: String::new(),
            update_time: String::new(),
        };

        let command = parse_job_command_from_uploaded_file(&file).unwrap();

        assert_eq!(command.name, "员工手册.pdf");
        assert_eq!(command.file_id, Some(88));
        assert_eq!(command.source_uri, "/file/knowledge/88.pdf");
        assert_eq!(command.content_type, "application/pdf");
        assert_eq!(command.source_hash, "file-hash");
        assert_eq!(command.source_kind, "objectStorage");
    }

    #[test]
    fn parser_job_response_restores_persisted_parser_request() {
        let parser_request = serde_json::json!({
            "tenantId": 1,
            "datasetId": 7,
            "documentId": 42,
            "parserJobId": 99,
            "source": {
                "kind": "objectStorage",
                "contentType": "application/pdf",
                "name": "training.pdf",
                "uri": "/file/knowledge/88.pdf"
            },
            "options": {
                "maxChunkChars": DEFAULT_CHUNK_MAX_CHARS,
                "chunkOverlapChars": DEFAULT_CHUNK_OVERLAP_CHARS
            },
            "trace": {
                "requestId": "parser-job-99"
            }
        });
        let now = Utc::now().naive_utc();

        let resp = ParserJobResp::from(ParserJobRecord {
            id: 99,
            tenant_id: 1,
            dataset_id: 7,
            document_id: 42,
            job_type: PARSER_JOB_TYPE_WORKER,
            status: PARSER_JOB_STATUS_SUBMITTED,
            attempt_count: 0,
            error_message: String::new(),
            result_summary: serde_json::json!({
                "parser": "parser-worker",
                "status": "submitted",
                "parserRequest": parser_request.clone()
            }),
            document_name: "training.pdf".to_owned(),
            source_uri: "/file/knowledge/88.pdf".to_owned(),
            file_id: Some(88),
            content_type: "application/pdf".to_owned(),
            parse_status: DOCUMENT_PARSE_STATUS_PARSING,
            ingestion_status: DOCUMENT_INGESTION_STATUS_PENDING,
            chunk_count: 0,
            create_time: now,
            create_user_string: "admin".to_owned(),
            update_time: None,
            update_user_string: String::new(),
        });

        assert_eq!(resp.parser_request, Some(parser_request));
    }

    #[test]
    fn parser_job_status_update_persists_deferred_mineru_task() {
        let command = serde_json::from_value::<ParserJobStatusUpdateCommand>(serde_json::json!({
            "status": " submitted ",
            "callbackStatus": " deferred ",
            "parserResult": {
                "status": "submitted",
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 42,
                "parserJobId": 99,
                "mineruTask": {
                    "taskId": "task-1",
                    "state": "pending",
                    "fullZipUrl": ""
                },
                "metadata": {"parser": "mineru"}
            }
        }))
        .unwrap();

        let update =
            parser_job_status_update_record(1, 7, 42, 99, 9, command, Utc::now().naive_utc())
                .unwrap();

        assert_eq!(update.parser_job.status, PARSER_JOB_STATUS_SUBMITTED);
        assert_eq!(update.document_parse_status, DOCUMENT_PARSE_STATUS_PARSING);
        assert_eq!(
            update.document_ingestion_status,
            DOCUMENT_INGESTION_STATUS_PENDING
        );
        assert_eq!(update.error_message, None);
        assert_eq!(update.parser_job.result_summary["parser"], "parser-worker");
        assert_eq!(update.parser_job.result_summary["status"], "submitted");
        assert_eq!(
            update.parser_job.result_summary["callbackStatus"],
            "deferred"
        );
        assert_eq!(
            update.parser_job.result_summary["parserResult"]["mineruTask"]["taskId"],
            "task-1"
        );
    }

    #[test]
    fn parser_job_status_update_marks_failed_with_error_summary() {
        let command = serde_json::from_value::<ParserJobStatusUpdateCommand>(serde_json::json!({
            "status": "failed",
            "callbackStatus": "failed",
            "error": {"message": "MinerU task failed"},
            "parserResult": {
                "status": "failed",
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 42,
                "parserJobId": 99,
                "error": {"message": "MinerU task failed"}
            }
        }))
        .unwrap();

        let update =
            parser_job_status_update_record(1, 7, 42, 99, 9, command, Utc::now().naive_utc())
                .unwrap();

        assert_eq!(update.parser_job.status, PARSER_JOB_STATUS_FAILED);
        assert_eq!(update.document_parse_status, DOCUMENT_PARSE_STATUS_FAILED);
        assert_eq!(
            update.document_ingestion_status,
            DOCUMENT_INGESTION_STATUS_PENDING
        );
        assert_eq!(update.error_message.as_deref(), Some("MinerU task failed"));
        assert_eq!(update.parser_job.result_summary["status"], "failed");
    }

    #[test]
    fn parser_job_status_update_rejects_succeeded_without_chunks() {
        let command = serde_json::from_value::<ParserJobStatusUpdateCommand>(serde_json::json!({
            "status": "succeeded",
            "callbackStatus": "posted"
        }))
        .unwrap();

        let err = parser_job_status_update_record(1, 7, 42, 99, 9, command, Utc::now().naive_utc())
            .unwrap_err();

        assert!(err.to_string().contains("documents/parsed"));
    }

    #[test]
    fn chunk_save_records_keep_semantic_text_columns_and_metadata_json() {
        let command = DocumentUploadCommand {
            name: "handbook.md".to_owned(),
            content: "# 入职培训\n[[page: 3]]\n第一天需要完成安全培训。".to_owned(),
            content_type: "text/markdown".to_owned(),
        };
        let command = normalize_document_upload_command(command).unwrap();
        let chunks = document_upload_chunks(42, &command);
        let now = Utc::now().naive_utc();

        let records = chunk_save_records(1, 7, 42, chunks, 9, now);

        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].semantic_search_text,
            "handbook.md\n入职培训\n第一天需要完成安全培训。"
        );
        assert_eq!(records[0].segment_type, "text");
        assert_eq!(records[0].segment_index, 0);
        assert_eq!(records[0].page_no, Some(3));
        assert_eq!(records[0].section_path, serde_json::json!(["入职培训"]));
        assert_eq!(
            records[0].embedding_model.as_deref(),
            Some(LOCAL_EMBEDDING_ROUTE)
        );
        assert_eq!(
            records[0].embedding_ref.as_deref(),
            Some("postgres-jsonb:42:0")
        );
        assert_eq!(
            records[0].metadata["embedding"]["routeId"],
            LOCAL_EMBEDDING_ROUTE
        );
        assert_eq!(records[0].metadata["embedding"]["dimension"], 64);
        assert_eq!(
            records[0].metadata["embedding"]["vector"]
                .as_array()
                .map(Vec::len),
            Some(64)
        );
        assert_eq!(records[0].content_role, "canonical");
        assert_eq!(records[0].display_capability, "precise_anchor");
        assert_eq!(records[0].metadata["sourceFileName"], "handbook.md");
    }

    #[test]
    fn runtime_embedding_vectors_override_local_chunk_embedding_metadata() {
        let command = normalize_document_upload_command(DocumentUploadCommand {
            name: "training.txt".to_owned(),
            content: "Onboarding training starts Monday.".to_owned(),
            ..DocumentUploadCommand::default()
        })
        .unwrap();
        let chunks = document_upload_chunks(42, &command);
        let now = Utc::now().naive_utc();
        let mut records = chunk_save_records(1, 7, 42, chunks, 9, now);

        apply_embedding_vectors_to_chunk_records(
            &mut records,
            "runtime.embedding",
            "runtime",
            &[
                crate::application::ai::model_service::ModelEmbeddingVector {
                    index: 0,
                    vector: vec![0.25, 0.75],
                },
            ],
        );

        assert_eq!(
            records[0].embedding_model.as_deref(),
            Some("runtime.embedding")
        );
        assert_eq!(
            records[0].embedding_ref.as_deref(),
            Some("postgres-jsonb:42:0")
        );
        assert_eq!(
            records[0].metadata["embedding"]["routeId"],
            "runtime.embedding"
        );
        assert_eq!(records[0].metadata["embedding"]["source"], "runtime");
        assert_eq!(records[0].metadata["embedding"]["dimension"], 2);
        assert_eq!(
            records[0].metadata["embedding"]["vector"],
            serde_json::json!([0.25, 0.75])
        );
    }

    #[tokio::test]
    async fn runtime_embedding_enrichment_keeps_local_fallback_without_runtime_route() {
        let command = normalize_document_upload_command(DocumentUploadCommand {
            name: "training.txt".to_owned(),
            content: "Onboarding training starts Monday.".to_owned(),
            ..DocumentUploadCommand::default()
        })
        .unwrap();
        let chunks = document_upload_chunks(42, &command);
        let now = Utc::now().naive_utc();
        let mut records = chunk_save_records(1, 7, 42, chunks, 9, now);
        let local_embedding = records[0].metadata["embedding"].clone();
        let config = ModelRuntimeConfig::from_env_map(|_| None);

        enrich_chunk_records_with_runtime_embeddings_from_config(&mut records, &config).await;

        assert_eq!(
            records[0].embedding_model.as_deref(),
            Some(LOCAL_EMBEDDING_ROUTE)
        );
        assert_eq!(records[0].metadata["embedding"], local_embedding);
    }

    #[test]
    fn hybrid_retrieval_uses_chunk_embedding_metadata_when_keyword_misses() {
        let citation = CitationRef {
            document_id: "42".to_owned(),
            chunk_id: "42:0".to_owned(),
            page_no: None,
            section_path: vec!["安全培训".to_owned()],
        };
        let chunks = vec![IndexedRagChunk {
            chunk_db_id: 11,
            document_id: 42,
            embedding_vector: Some(local_embedding_vector("安全培训")),
            chunk: RagDocumentChunk {
                document_id: "42".to_owned(),
                chunk_id: "42:0".to_owned(),
                chunk_index: 0,
                text: "employee onboarding handbook".to_owned(),
                semantic_search_text: "employee onboarding handbook".to_owned(),
                token_count: 3,
                citation: citation.clone(),
                metadata: ChunkMetadata::default(),
            },
        }];

        let hits = hybrid_retrieve_indexed_chunks("安全培训", &chunks, 5);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk.chunk_id, "42:0");
        assert!(hits[0].score > 0.0);
    }

    #[test]
    fn hybrid_retrieval_uses_runtime_query_embedding_dimension() {
        let citation = CitationRef {
            document_id: "42".to_owned(),
            chunk_id: "42:0".to_owned(),
            page_no: None,
            section_path: vec!["安全培训".to_owned()],
        };
        let chunks = vec![IndexedRagChunk {
            chunk_db_id: 11,
            document_id: 42,
            embedding_vector: Some(vec![0.0, 1.0, 0.0]),
            chunk: RagDocumentChunk {
                document_id: "42".to_owned(),
                chunk_id: "42:0".to_owned(),
                chunk_index: 0,
                text: "employee onboarding handbook".to_owned(),
                semantic_search_text: "employee onboarding handbook".to_owned(),
                token_count: 3,
                citation: citation.clone(),
                metadata: ChunkMetadata::default(),
            },
        }];
        let query_embeddings = vec![local_embedding_vector("安全培训"), vec![0.0, 0.9, 0.1]];

        let hits = hybrid_retrieve_indexed_chunks_with_query_embeddings(
            "安全培训",
            &chunks,
            5,
            &query_embeddings,
        );

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk.chunk_id, "42:0");
    }

    #[test]
    fn milvus_search_config_reads_endpoint_token_and_search_url() {
        let config = MilvusSearchConfig::from_env_map(|key| match key {
            "MILVUS_ENDPOINT" => Some(" http://localhost:19530/ ".to_owned()),
            "MILVUS_TOKEN" => Some("root:Milvus".to_owned()),
            _ => None,
        })
        .expect("milvus config should be present");

        assert_eq!(config.endpoint, "http://localhost:19530");
        assert_eq!(config.token.as_deref(), Some("root:Milvus"));
        assert_eq!(
            config.search_url(),
            "http://localhost:19530/v2/vectordb/entities/search"
        );
    }

    #[test]
    fn milvus_request_uses_vector_collection_mapping_dimension_and_metric() {
        let collection = VectorCollectionRecord {
            id: 100,
            vector_backend: "milvus".to_owned(),
            provider_collection: "novex_t42_dataset_7".to_owned(),
            dimension: 2,
            metric_type: "ip".to_owned(),
            status: VECTOR_COLLECTION_STATUS_READY,
        };

        let request = milvus_search_request_for_collection(42, 7, &collection, vec![0.1, 0.2], 4)
            .expect("valid collection should build a milvus request");
        let body = request.to_rest_search_body();

        assert_eq!(body["collectionName"], "novex_t42_dataset_7");
        assert_eq!(body["filter"], "tenant_id == 42 and dataset_id == 7");
        assert_eq!(body["searchParams"]["metric_type"], "IP");
        assert!(milvus_search_request_for_collection(42, 7, &collection, vec![0.1], 4).is_none());
    }

    #[test]
    fn milvus_upsert_request_uses_chunk_embedding_and_collection_mapping() {
        let command = normalize_document_upload_command(DocumentUploadCommand {
            name: "training.txt".to_owned(),
            content: "Onboarding training starts Monday.".to_owned(),
            ..DocumentUploadCommand::default()
        })
        .unwrap();
        let chunks = document_upload_chunks(77, &command);
        let now = Utc::now().naive_utc();
        let records = chunk_save_records(42, 7, 77, chunks, 9, now);
        let collection = VectorCollectionRecord {
            id: 100,
            vector_backend: "milvus".to_owned(),
            provider_collection: "novex_t42_dataset_7".to_owned(),
            dimension: LOCAL_EMBEDDING_DIMENSION as i32,
            metric_type: "cosine".to_owned(),
            status: VECTOR_COLLECTION_STATUS_READY,
        };

        let request = milvus_upsert_request_for_collection(&collection, &records)
            .expect("chunk records with vectors should build a milvus upsert request");
        let body = request.to_rest_upsert_body();

        assert_eq!(body["collectionName"], "novex_t42_dataset_7");
        assert_eq!(body["data"][0]["id"], records[0].id);
        assert_eq!(body["data"][0]["chunk_db_id"], records[0].id);
        assert_eq!(body["data"][0]["tenant_id"], 42);
        assert_eq!(body["data"][0]["dataset_id"], 7);
        assert_eq!(body["data"][0]["document_id"], 77);
        assert_eq!(body["data"][0]["chunk_uid"], records[0].chunk_uid);
        assert!(body["data"][0]["embedding"].as_array().unwrap().len() > 1);
        assert_eq!(body["data"][0]["segment_type"], records[0].segment_type);
    }

    #[test]
    fn milvus_hits_are_mapped_back_to_indexed_postgres_chunks() {
        let citation = CitationRef {
            document_id: "42".to_owned(),
            chunk_id: "42:1".to_owned(),
            page_no: None,
            section_path: vec![],
        };
        let indexed_chunks = vec![IndexedRagChunk {
            chunk_db_id: 12,
            document_id: 42,
            embedding_vector: Some(vec![0.0, 1.0]),
            chunk: RagDocumentChunk {
                document_id: "42".to_owned(),
                chunk_id: "42:1".to_owned(),
                chunk_index: 1,
                text: "Milvus indexed context".to_owned(),
                semantic_search_text: "Milvus indexed context".to_owned(),
                token_count: 3,
                citation,
                metadata: ChunkMetadata::default(),
            },
        }];
        let milvus_hits = vec![
            MilvusSearchHit {
                chunk_uid: "missing".to_owned(),
                score: 0.99,
                chunk_db_id: None,
                document_id: None,
                fields: serde_json::json!({}),
            },
            MilvusSearchHit {
                chunk_uid: "42:1".to_owned(),
                score: 0.83,
                chunk_db_id: Some(12),
                document_id: Some(42),
                fields: serde_json::json!({}),
            },
        ];

        let hits = retrieval_hits_from_milvus_hits(&milvus_hits, &indexed_chunks, 5);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rank, 1);
        assert_eq!(hits[0].chunk.chunk_id, "42:1");
        assert!((hits[0].score - 0.83).abs() < f32::EPSILON);
    }

    #[test]
    fn parser_job_summary_describes_chunking_and_semantic_search_contract() {
        let command = normalize_document_upload_command(DocumentUploadCommand {
            name: "training.csv".to_owned(),
            content: "employee,deadline,status\nAlice,Friday,done\nBob,Monday,pending".to_owned(),
            content_type: "text/csv".to_owned(),
        })
        .unwrap();
        let chunks = document_upload_chunks(42, &command);

        let summary = parser_job_result_summary(&command, &chunks);

        assert_eq!(summary["parser"], "novex-rag-local-structured");
        assert_eq!(summary["chunker"], "file-type-default");
        assert_eq!(summary["embeddingInput"], "semanticSearchText");
        assert_eq!(summary["maxChunkChars"], DEFAULT_CHUNK_MAX_CHARS);
        assert_eq!(summary["overlapChars"], DEFAULT_CHUNK_OVERLAP_CHARS);
        assert_eq!(summary["segmentTypeCounts"]["table"], chunks.len());
        assert!(summary["semanticSearchText"]["maxChars"]
            .as_u64()
            .is_some_and(|count| count > 0));
    }

    #[test]
    fn chunk_save_records_keep_image_anchor_metadata() {
        let command = normalize_document_upload_command(DocumentUploadCommand {
            name: "architecture.md".to_owned(),
            content: "# 检索链路\n[[page: 2]]\n[[image: key=img/search-flow.png bbox=10,20,300,180 caption=系统架构图显示 hybrid recall 和 rerank 链路]]".to_owned(),
            content_type: "text/markdown".to_owned(),
        })
        .unwrap();
        let chunks = document_upload_chunks(42, &command);
        let now = Utc::now().naive_utc();

        let records = chunk_save_records(1, 7, 42, chunks, 9, now);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].segment_type, "image");
        assert_eq!(records[0].page_no, Some(2));
        assert_eq!(
            records[0].metadata["imageAccessKeys"],
            serde_json::json!(["img/search-flow.png"])
        );
        assert_eq!(records[0].metadata["bbox"]["width"], 300);
        assert!(records[0]
            .semantic_search_text
            .contains("系统架构图显示 hybrid recall"));
    }

    #[test]
    fn chunk_metadata_migration_defines_search_contract_columns() {
        let migration =
            include_str!("../../../migrations/202606050018_enrich_ai_document_chunk_metadata.sql");

        for column in [
            "semantic_search_text",
            "segment_type",
            "segment_index",
            "page_no",
            "section_path",
            "content_role",
            "display_capability",
        ] {
            assert!(migration.contains(column), "missing {column}");
        }
    }

    #[test]
    fn parser_result_ingestion_preserves_blocks_and_chunk_search_contract() {
        let command = serde_json::from_value::<ParsedDocumentUploadCommand>(serde_json::json!({
            "name": "salary-policy.pdf",
            "contentType": "application/pdf",
            "parserResult": {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 42,
                "parserJobId": 99,
                "status": "succeeded",
                "blocks": [
                    {
                        "blockId": "tbl-1",
                        "type": "table",
                        "text": "岗位,补贴\n工程师,100",
                        "pageNo": 5,
                        "sectionPath": ["薪酬政策"],
                        "bbox": {"x": 10, "y": 20, "width": 300, "height": 120}
                    }
                ],
                "chunks": [
                    {
                        "chunkUid": "42:0",
                        "chunkIndex": 0,
                        "text": "岗位,补贴\n工程师,100",
                        "tokenCount": 4,
                        "citation": {
                            "documentId": "42",
                            "chunkId": "42:0",
                            "pageNo": 5,
                            "sectionPath": ["薪酬政策"],
                            "blockIds": ["tbl-1"]
                        }
                    }
                ],
                "metadata": {
                    "parser": "mineru",
                    "pageCount": 8,
                    "lineCount": 20,
                    "sourceHash": "abc123",
                    "warnings": ["table confidence low"]
                }
            }
        }))
        .unwrap();
        let now = Utc::now().naive_utc();

        let parts = parsed_document_ingestion_parts(DEFAULT_TENANT_ID, 7, 9, command, now).unwrap();

        assert_eq!(parts.document.id, 42);
        assert_eq!(parts.document.name, "salary-policy.pdf");
        assert_eq!(parts.document.source_hash.as_deref(), Some("abc123"));
        assert_eq!(parts.parser_job.id, 99);
        assert_eq!(parts.parser_job.result_summary["parser"], "mineru");
        assert_eq!(parts.blocks.len(), 1);
        assert_eq!(parts.blocks[0].block_uid, "tbl-1");
        assert_eq!(parts.blocks[0].block_type, "table");
        assert_eq!(parts.blocks[0].page_no, Some(5));
        assert_eq!(
            parts.blocks[0].section_path,
            serde_json::json!(["薪酬政策"])
        );
        assert_eq!(parts.blocks[0].bbox["width"], 300);
        assert_eq!(parts.chunks.len(), 1);
        assert_eq!(parts.chunks[0].chunk_uid, "42:0");
        assert_eq!(parts.chunks[0].segment_type, "table");
        assert_eq!(parts.chunks[0].segment_index, 0);
        assert_eq!(parts.chunks[0].page_no, Some(5));
        assert_eq!(
            parts.chunks[0].section_path,
            serde_json::json!(["薪酬政策"])
        );
        assert_eq!(
            parts.chunks[0].metadata["parserBlockIds"],
            serde_json::json!(["tbl-1"])
        );
        assert_eq!(parts.chunks[0].metadata["bbox"]["height"], 120);
        assert!(parts.chunks[0]
            .semantic_search_text
            .contains("salary-policy.pdf"));
        assert!(parts.chunks[0].semantic_search_text.contains("薪酬政策"));
        assert!(parts.chunks[0].semantic_search_text.contains("岗位 补贴"));
    }

    #[test]
    fn parser_result_ingestion_uses_explicit_chunk_search_metadata() {
        let command = serde_json::from_value::<ParsedDocumentUploadCommand>(serde_json::json!({
            "name": "org-policy.pdf",
            "contentType": "application/pdf",
            "parserResult": {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 42,
                "parserJobId": 99,
                "status": "succeeded",
                "blocks": [
                    {
                        "blockId": "p-1",
                        "type": "paragraph",
                        "text": "原始 OCR 噪声 fallback",
                        "pageNo": 2,
                        "sectionPath": ["组织管理"]
                    }
                ],
                "chunks": [
                    {
                        "chunkUid": "42:0",
                        "chunkIndex": 0,
                        "text": "原始 OCR 噪声 fallback",
                        "semanticSearchText": "组织架构 团队权限 审批责任",
                        "segmentType": "table",
                        "tableHeader": ["团队", "权限", "审批人"],
                        "imageAccessKeys": [" img/org-chart.png ", "img/org-chart.png"],
                        "contentRole": "summary_faq",
                        "displayCapability": "row_only",
                        "metadata": {"confidence": 0.92, "origin": "mineru-layout"},
                        "tokenCount": 0,
                        "citation": {
                            "documentId": "42",
                            "chunkId": "42:0",
                            "blockIds": [" p-1 "],
                            "sectionPath": []
                        }
                    }
                ],
                "metadata": {
                    "parser": "mineru",
                    "warnings": []
                }
            }
        }))
        .unwrap();

        let parts = parsed_document_ingestion_parts(
            DEFAULT_TENANT_ID,
            7,
            9,
            command,
            Utc::now().naive_utc(),
        )
        .unwrap();

        let chunk = &parts.chunks[0];
        assert_eq!(chunk.segment_type, "table");
        assert_eq!(chunk.content_role, "summary_faq");
        assert_eq!(chunk.display_capability, "row_only");
        assert_eq!(chunk.page_no, Some(2));
        assert_eq!(chunk.section_path, serde_json::json!(["组织管理"]));
        assert_eq!(
            chunk.metadata["tableHeader"],
            serde_json::json!(["团队", "权限", "审批人"])
        );
        assert_eq!(
            chunk.metadata["imageAccessKeys"],
            serde_json::json!(["img/org-chart.png"])
        );
        assert_eq!(chunk.metadata["parserChunkMetadata"]["confidence"], 0.92);
        assert!(chunk.semantic_search_text.contains("org-policy.pdf"));
        assert!(chunk.semantic_search_text.contains("组织管理"));
        assert!(chunk.semantic_search_text.contains("团队 权限 审批人"));
        assert!(chunk
            .semantic_search_text
            .contains("组织架构 团队权限 审批责任"));
        assert!(!chunk.semantic_search_text.contains("原始 OCR 噪声"));
        assert!(chunk.token_count > 0);
    }

    #[test]
    fn parser_result_rejects_unknown_block_references() {
        let command = serde_json::from_value::<ParsedDocumentUploadCommand>(serde_json::json!({
            "name": "broken-block.pdf",
            "contentType": "application/pdf",
            "parserResult": {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 42,
                "parserJobId": 99,
                "status": "succeeded",
                "blocks": [
                    {
                        "blockId": "p-1",
                        "type": "paragraph",
                        "text": "有效段落"
                    }
                ],
                "chunks": [
                    {
                        "chunkUid": "42:0",
                        "chunkIndex": 0,
                        "text": "有效段落",
                        "tokenCount": 3,
                        "citation": {
                            "documentId": "42",
                            "chunkId": "42:0",
                            "sectionPath": [],
                            "blockIds": ["missing-block"]
                        }
                    }
                ],
                "metadata": {
                    "parser": "mineru",
                    "warnings": []
                }
            }
        }))
        .unwrap();

        let err = parsed_document_ingestion_parts(
            DEFAULT_TENANT_ID,
            7,
            9,
            command,
            Utc::now().naive_utc(),
        )
        .unwrap_err();

        assert!(err.to_string().contains("blockIds"));
    }

    #[test]
    fn parser_result_rejects_failed_or_empty_chunk_payloads() {
        let failed = serde_json::from_value::<ParsedDocumentUploadCommand>(serde_json::json!({
            "name": "broken.pdf",
            "contentType": "application/pdf",
            "parserResult": {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 42,
                "parserJobId": 99,
                "status": "failed",
                "error": {"code": "mineru_failed", "message": "parse failed", "retryable": false},
                "blocks": [],
                "chunks": [],
                "metadata": {"parser": "mineru", "warnings": []}
            }
        }))
        .unwrap();

        let err = parsed_document_ingestion_parts(
            DEFAULT_TENANT_ID,
            7,
            9,
            failed,
            Utc::now().naive_utc(),
        )
        .unwrap_err();

        assert!(err.to_string().contains("解析结果未成功"));
    }

    #[test]
    fn document_block_migration_defines_layout_block_store() {
        let migration =
            include_str!("../../../migrations/202606050020_create_ai_document_block.sql");

        for column in [
            "ai_document_block",
            "block_uid",
            "block_index",
            "block_type",
            "page_no",
            "section_path",
            "bbox",
        ] {
            assert!(migration.contains(column), "missing {column}");
        }
    }

    #[test]
    fn rag_ask_rejects_blank_question() {
        let command = RagAskCommand {
            question: "   ".to_owned(),
            ..RagAskCommand::default()
        };

        let err = normalize_rag_ask_command(command).unwrap_err();

        assert!(err.to_string().contains("问题不能为空"));
    }

    #[test]
    fn rag_ask_response_contains_answer_citations_and_trace_id() {
        let answer = novex_rag::RagAnswer {
            answer: "Training starts on Monday.".to_owned(),
            citations: vec![novex_rag::CitationRef {
                document_id: "7".to_owned(),
                chunk_id: "7:0".to_owned(),
                page_no: None,
                section_path: vec!["入职".to_owned()],
            }],
            trace: novex_rag::RagTraceSnapshot {
                retrieval_hit_count: 1,
                answer_strategy: "extractive".to_owned(),
            },
        };

        let resp = rag_ask_response(42, answer);

        assert_eq!(resp.trace_id, 42);
        assert_eq!(resp.answer, "Training starts on Monday.");
        assert_eq!(resp.citations.len(), 1);
        assert_eq!(resp.citations[0].document_id, "7");
    }

    #[test]
    fn rag_ask_trace_hits_keep_chunk_score_and_rank() {
        let now = Utc::now().naive_utc();
        let hit = IndexedRetrievalHit {
            chunk_db_id: 11,
            document_id: 7,
            rank: 2,
            score: 0.75,
            citation: novex_rag::CitationRef {
                document_id: "7".to_owned(),
                chunk_id: "7:1".to_owned(),
                page_no: None,
                section_path: vec![],
            },
            content: "Training policy preview".to_owned(),
            token_count: 3,
        };

        let records = rag_trace_hit_records(99, DEFAULT_TENANT_ID, 5, &[hit], now);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].trace_id, 99);
        assert_eq!(records[0].chunk_id, 11);
        assert_eq!(records[0].rank, 2);
        assert!((records[0].score - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn rag_feedback_rejects_invalid_rating_and_requires_trace() {
        let err = normalize_rag_feedback_command(RagFeedbackCommand {
            trace_id: 0,
            rating: "helpful".to_owned(),
            reason: "  ".to_owned(),
        })
        .unwrap_err();
        assert!(err.to_string().contains("Trace ID 不合法"));

        let err = normalize_rag_feedback_command(RagFeedbackCommand {
            trace_id: 42,
            rating: "maybe".to_owned(),
            reason: "  ".to_owned(),
        })
        .unwrap_err();
        assert!(err.to_string().contains("反馈类型不合法"));
    }

    #[test]
    fn rag_feedback_trims_reason_and_maps_eval_payload() {
        let command = normalize_rag_feedback_command(RagFeedbackCommand {
            trace_id: 42,
            rating: "citation_issue".to_owned(),
            reason: "  引用不准确  ".to_owned(),
        })
        .unwrap();

        assert_eq!(command.trace_id, 42);
        assert_eq!(command.rating, "citation_issue");
        assert_eq!(command.reason, "引用不准确");
        assert_eq!(rag_feedback_metadata(&command)["rating"], "citation_issue");
    }

    #[test]
    fn ai_feedback_trims_resource_and_preserves_customer_metadata() {
        let command = normalize_ai_feedback_command(AiFeedbackCommand {
            resource_type: " training_quiz ".to_owned(),
            resource_id: " 900 ".to_owned(),
            trace_id: Some(" agent-900 ".to_owned()),
            rating: " quiz_wrong_answer ".to_owned(),
            reason: "  错题答案需要复核  ".to_owned(),
            metadata: serde_json::json!({
                "source": "training-web",
                "quizRunId": 900
            }),
        })
        .unwrap();

        assert_eq!(command.resource_type, "training_quiz");
        assert_eq!(command.resource_id, "900");
        assert_eq!(command.trace_id.as_deref(), Some("agent-900"));
        assert_eq!(command.rating, "quiz_wrong_answer");
        assert_eq!(command.reason, "错题答案需要复核");
        assert_eq!(ai_feedback_metadata(&command)["source"], "training-web");
        assert_eq!(ai_feedback_metadata(&command)["quizRunId"], 900);
    }

    #[test]
    fn rag_model_routes_use_runtime_config_when_available() {
        let env = [
            ("LLM_API_KEY", "sk-fake-llm-secret-508d"),
            ("LLM_BASE_URL", "https://api.deepseek.com"),
            ("LLM_MODEL", "deepseek-v4-flash"),
            ("EMBEDDING_API_KEY", "sk-fake-embedding-secret-ffff"),
            (
                "EMBEDDING_BASE_URL",
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
            ),
            ("EMBEDDING_MODEL", "text-embedding-v4"),
            ("RERANKER_API_KEY", "sk-fake-reranker-secret-ffff"),
            (
                "RERANKER_BASE_URL",
                "https://dashscope.aliyuncs.com/compatible-api/v1",
            ),
            ("RERANKER_MODEL", "qwen3-rerank"),
        ];
        let config = novex_model::ModelRuntimeConfig::from_env_map(|key| {
            env.iter()
                .find_map(|(env_key, value)| (*env_key == key).then(|| (*value).to_owned()))
        });

        let routes = rag_model_routes_from_config(&config);

        assert_eq!(routes.embedding_model_route, "runtime.embedding");
        assert_eq!(routes.rerank_model_route, "runtime.reranker");
        assert_eq!(routes.answer_model_route, "runtime.llm");
    }

    #[test]
    fn rag_model_routes_fall_back_to_local_routes_when_runtime_config_missing() {
        let config = novex_model::ModelRuntimeConfig::from_env_map(|_| None);

        let routes = rag_model_routes_from_config(&config);

        assert_eq!(routes.embedding_model_route, LOCAL_EMBEDDING_ROUTE);
        assert_eq!(routes.rerank_model_route, novex_rag::LOCAL_RERANK_ROUTE);
        assert_eq!(routes.answer_model_route, novex_rag::LOCAL_ANSWER_ROUTE);
    }

    fn test_retrieval_hit(chunk_uid: &str, chunk_index: usize, text: &str) -> RetrievalHit {
        let citation = CitationRef {
            document_id: "42".to_owned(),
            chunk_id: chunk_uid.to_owned(),
            page_no: None,
            section_path: vec![],
        };
        RetrievalHit {
            rank: chunk_index + 1,
            score: 0.8 - (chunk_index as f32 * 0.01),
            citation: citation.clone(),
            chunk: RagDocumentChunk {
                document_id: "42".to_owned(),
                chunk_id: chunk_uid.to_owned(),
                chunk_index,
                text: text.to_owned(),
                semantic_search_text: text.to_owned(),
                token_count: tokenish_count(text).max(1) as usize,
                citation,
                metadata: ChunkMetadata::default(),
            },
        }
    }

    #[test]
    fn rag_answer_prompt_contains_question_context_and_citation_labels() {
        let hits = vec![test_retrieval_hit(
            "chunk-a",
            0,
            "Training starts on Monday.",
        )];

        let command = rag_answer_chat_command("When does training start?", &hits, 512);

        assert_eq!(command.temperature, Some(0.2));
        assert_eq!(command.max_tokens, Some(512));
        assert_eq!(command.messages.len(), 2);
        assert!(command.messages[0]
            .content
            .contains("Only answer from the provided context"));
        assert!(command.messages[1]
            .content
            .contains("Question:\nWhen does training start?"));
        assert!(command.messages[1].content.contains("[1] chunk-a"));
        assert!(command.messages[1]
            .content
            .contains("Training starts on Monday."));
    }

    #[test]
    fn llm_rag_answer_uses_retrieved_citations_and_live_strategy() {
        let hits = vec![
            test_retrieval_hit("chunk-a", 0, "Training starts on Monday."),
            test_retrieval_hit("chunk-b", 1, "Mentors review progress Friday."),
        ];
        let chat = ModelChatResp {
            conversation_id: None,
            answer: "Training starts on Monday.".to_owned(),
            route_id: "runtime.llm".to_owned(),
            model: Some("deepseek-test".to_owned()),
            latency_ms: 12,
            usage: ModelChatUsage::default(),
        };

        let answer = rag_answer_from_model_chat(chat, &hits);

        assert_eq!(answer.answer, "Training starts on Monday.");
        assert_eq!(answer.trace.answer_strategy, "llm_grounded");
        assert_eq!(answer.trace.retrieval_hit_count, 2);
        assert_eq!(answer.citations.len(), 2);
        assert_eq!(answer.citations[0].chunk_id, "chunk-a");
    }

    fn runtime_config_with_llm() -> novex_model::ModelRuntimeConfig {
        let env = [
            ("LLM_API_KEY", "sk-fake-llm-secret-508d"),
            ("LLM_BASE_URL", "https://api.deepseek.com"),
            ("LLM_MODEL", "deepseek-v4-flash"),
        ];
        novex_model::ModelRuntimeConfig::from_env_map(|key| {
            env.iter()
                .find_map(|(env_key, value)| (*env_key == key).then(|| (*value).to_owned()))
        })
    }

    #[test]
    fn rag_answer_mode_requires_llm_when_strict_env_is_enabled() {
        let config = novex_model::ModelRuntimeConfig::from_env_map(|_| None);

        let mode = rag_answer_mode_from_config(&config, true);

        assert!(matches!(mode, RagAnswerMode::RequireLlm));
    }

    #[test]
    fn rag_answer_mode_uses_llm_when_route_is_configured() {
        let config = runtime_config_with_llm();

        let mode = rag_answer_mode_from_config(&config, false);

        assert!(matches!(mode, RagAnswerMode::UseLlm));
    }

    #[test]
    fn rag_answer_mode_keeps_extract_fallback_without_llm_or_strict() {
        let config = novex_model::ModelRuntimeConfig::from_env_map(|_| None);

        let mode = rag_answer_mode_from_config(&config, false);

        assert!(matches!(mode, RagAnswerMode::ExtractiveFallback));
    }

    fn test_indexed_rag_chunk(chunk_uid: &str, chunk_index: usize, text: &str) -> IndexedRagChunk {
        let hit = test_retrieval_hit(chunk_uid, chunk_index, text);
        IndexedRagChunk {
            chunk_db_id: 1000 + chunk_index as i64,
            document_id: 42,
            embedding_vector: Some(local_embedding_vector(text)),
            chunk: hit.chunk,
        }
    }

    #[test]
    fn strict_rag_dependency_mode_reads_truthy_env_values() {
        assert!(parse_bool_env_flag(Some("1")));
        assert!(parse_bool_env_flag(Some("true")));
        assert!(parse_bool_env_flag(Some("TRUE")));
        assert!(parse_bool_env_flag(Some("yes")));
        assert!(parse_bool_env_flag(Some("on")));
        assert!(!parse_bool_env_flag(Some("0")));
        assert!(!parse_bool_env_flag(Some("false")));
        assert!(!parse_bool_env_flag(None));
    }

    #[tokio::test]
    async fn strict_retrieval_fails_without_milvus_config() {
        let chunks = vec![test_indexed_rag_chunk(
            "chunk-a",
            0,
            "Training starts on Monday.",
        )];

        let err = hybrid_retrieve_indexed_chunks_with_milvus_or_local_strict(
            "When does training start?",
            42,
            7,
            None,
            &chunks,
            5,
            true,
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("Milvus"));
    }

    #[test]
    fn strict_rerank_requires_runtime_route() {
        let config = novex_model::ModelRuntimeConfig::from_env_map(|_| None);

        assert!(rerank_route_for_mode(&config, true).is_err());
        assert!(rerank_route_for_mode(&config, false).is_ok());
    }

    #[test]
    fn rag_trace_record_uses_explicit_live_model_routes() {
        let command = RagAskCommand {
            question: "When does training start?".to_owned(),
            limit: 3,
        };
        let answer = RagAnswer {
            answer: "Training starts on Monday.".to_owned(),
            citations: vec![],
            trace: novex_rag::RagTraceSnapshot {
                retrieval_hit_count: 1,
                answer_strategy: "llm_grounded".to_owned(),
            },
        };
        let routes = RagModelRoutes {
            embedding_model_route: "runtime.embedding".to_owned(),
            rerank_model_route: "runtime.reranker".to_owned(),
            answer_model_route: "runtime.llm".to_owned(),
        };

        let trace = rag_trace_record(
            1,
            42,
            99,
            7,
            &command,
            &answer,
            &[],
            &routes,
            chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc(),
        );

        assert_eq!(trace.answer_strategy, "llm_grounded");
        assert_eq!(trace.embedding_model_route.as_deref(), Some("runtime.embedding"));
        assert_eq!(trace.rerank_model_route.as_deref(), Some("runtime.reranker"));
        assert_eq!(trace.answer_model_route.as_deref(), Some("runtime.llm"));
    }

    #[test]
    fn rag_rerank_scores_reorder_candidates_and_keep_unscored_tail() {
        fn hit(chunk_uid: &str, chunk_index: usize, score: f32) -> RetrievalHit {
            let citation = CitationRef {
                document_id: "42".to_owned(),
                chunk_id: chunk_uid.to_owned(),
                page_no: None,
                section_path: vec![],
            };
            RetrievalHit {
                rank: chunk_index + 1,
                score,
                citation: citation.clone(),
                chunk: RagDocumentChunk {
                    document_id: "42".to_owned(),
                    chunk_id: chunk_uid.to_owned(),
                    chunk_index,
                    text: format!("chunk {chunk_index}"),
                    semantic_search_text: format!("chunk {chunk_index}"),
                    token_count: 2,
                    citation,
                    metadata: ChunkMetadata::default(),
                },
            }
        }
        let hits = vec![
            hit("42:0", 0, 0.20),
            hit("42:1", 1, 0.18),
            hit("42:2", 2, 0.15),
        ];
        let scores = vec![
            crate::application::ai::model_service::ModelRerankScore {
                index: 2,
                score: 0.91,
            },
            crate::application::ai::model_service::ModelRerankScore {
                index: 0,
                score: 0.72,
            },
        ];

        let reranked = rerank_retrieval_hits(&hits, &scores, 3);

        assert_eq!(reranked[0].chunk.chunk_id, "42:2");
        assert_eq!(reranked[0].rank, 1);
        assert!((reranked[0].score - 0.91).abs() < f32::EPSILON);
        assert_eq!(reranked[1].chunk.chunk_id, "42:0");
        assert_eq!(reranked[1].rank, 2);
        assert_eq!(reranked[2].chunk.chunk_id, "42:1");
        assert_eq!(reranked[2].rank, 3);
        assert!((reranked[2].score - 0.18).abs() < f32::EPSILON);
    }

    #[test]
    fn rag_ask_default_chunks_keep_sentence_context() {
        let command = normalize_document_upload_command(DocumentUploadCommand {
            name: "onboarding.txt".to_owned(),
            content: "Training starts on Monday. Mentors review progress every Friday.".to_owned(),
            ..DocumentUploadCommand::default()
        })
        .unwrap();
        let chunks = document_upload_chunks(42, &command);
        let hits = keyword_retrieve("When does training start?", &chunks, 5);

        let answer = build_extractive_answer("When does training start?", &hits);

        assert!(answer.answer.contains("Training starts on Monday."));
    }
}
