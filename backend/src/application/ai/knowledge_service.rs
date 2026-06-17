use std::{
    collections::{HashMap, HashSet},
    env,
    time::Duration,
};

use chrono::{NaiveDateTime, Utc};
use futures_util::future::join_all;
#[cfg(test)]
use novex_model::ModelRuntimeTarget;
use novex_model::{ModelRoutePurpose, ModelRuntimeConfig};
use novex_rag::{
    build_extractive_answer, build_semantic_search_text, chunk_document, keyword_retrieve,
    parse_document_content, parse_milvus_search_hits, BoundingBox, ChunkMetadata, ChunkSegmentType,
    CitationRef, ContentRole, DisplayCapability, DocumentChunk as RagDocumentChunk,
    MilvusCreateCollectionRequest, MilvusMetricType, MilvusSearchHit, MilvusSearchRequest,
    MilvusUpsertRequest, MilvusUpsertRow, RagAnswer, RagModelRoutes, RetrievalHit,
    LOCAL_EMBEDDING_ROUTE,
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
        ParserJobSaveRecord, ParserOutboxSaveRecord, RagTraceHitSaveRecord, RagTraceSaveRecord,
        VectorCollectionRecord,
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
const PARSER_OUTBOX_EVENT_PARSE_REQUESTED: &str = "parser.job.requested";
const PARSER_OUTBOX_STATUS_PENDING: i16 = 1;
const PARSER_OUTBOX_MAX_ATTEMPTS: i32 = 5;
const CHUNK_EMBEDDING_STATUS_INDEXED: i16 = 4;
const DEFAULT_RAG_LIMIT: usize = 5;
const MAX_RAG_LIMIT: usize = 24;
const RERANK_CANDIDATE_MULTIPLIER: usize = 4;
const MAX_RERANK_CANDIDATES: usize = 80;
const DEFAULT_RETRIEVAL_PLAN_QUERIES: usize = 4;
const MAX_RETRIEVAL_PLAN_QUERIES: usize = 8;
const MAX_RETRIEVAL_PLAN_REQUIRED_SECTIONS: usize = 10;
const MAX_RETRIEVAL_PLAN_OUTLINE_SECTIONS: usize = 300;
const DEFAULT_RAG_ANSWER_MAX_TOKENS: u32 = 4096;
const LARGE_DOCUMENT_CHUNK_THRESHOLD: usize = 800;
const LARGE_DOCUMENT_TOKEN_THRESHOLD: usize = 80_000;
const LARGE_DOCUMENT_TARGET_HIT_LIMIT: usize = 20;
const LARGE_DOCUMENT_NOTES_BATCH_SIZE: usize = 5;
const LARGE_DOCUMENT_MAX_NOTE_BATCHES: usize = 4;
const LOCAL_EMBEDDING_DIMENSION: usize = 64;
const MAX_LOCAL_RETRIEVAL_CHUNKS: i64 = 5000;
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

    fn create_collection_url(&self) -> String {
        format!("{}/v2/vectordb/collections/create", self.endpoint)
    }

    fn load_collection_url(&self) -> String {
        format!("{}/v2/vectordb/collections/load", self.endpoint)
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
    #[serde(default)]
    pub answer_model_route_id: Option<String>,
    #[serde(skip)]
    pub answer_instruction: Option<String>,
    #[serde(skip)]
    pub source_document_ids: Vec<i64>,
}

impl Default for RagAskCommand {
    fn default() -> Self {
        Self {
            question: String::new(),
            limit: DEFAULT_RAG_LIMIT,
            answer_model_route_id: None,
            answer_instruction: None,
            source_document_ids: Vec::new(),
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
    pub embedding_model_route: String,
    pub rerank_model_route: String,
    pub answer_model_route: String,
    pub answer_model: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RagRetrievalPlan {
    queries: Vec<String>,
    required_sections: Vec<String>,
}

impl RagRetrievalPlan {
    fn fallback(question: &str) -> Self {
        Self {
            queries: vec![question.trim().to_owned()]
                .into_iter()
                .filter(|query| !query.is_empty())
                .collect(),
            required_sections: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RagRetrievalPlanPayload {
    #[serde(default)]
    queries: Vec<String>,
    #[serde(default, alias = "required_sections")]
    required_sections: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RagGenerationMode {
    SinglePass,
    LargeDocumentLoop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RagGenerationProfile {
    mode: RagGenerationMode,
    target_hit_limit: usize,
    notes_batch_size: usize,
    max_note_batches: usize,
}

impl RagGenerationProfile {
    fn single_pass() -> Self {
        Self {
            mode: RagGenerationMode::SinglePass,
            target_hit_limit: 0,
            notes_batch_size: 0,
            max_note_batches: 0,
        }
    }

    fn large_document_loop() -> Self {
        Self {
            mode: RagGenerationMode::LargeDocumentLoop,
            target_hit_limit: LARGE_DOCUMENT_TARGET_HIT_LIMIT,
            notes_batch_size: LARGE_DOCUMENT_NOTES_BATCH_SIZE,
            max_note_batches: LARGE_DOCUMENT_MAX_NOTE_BATCHES,
        }
    }

    fn uses_loop(self) -> bool {
        self.mode == RagGenerationMode::LargeDocumentLoop
    }
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
    db: PgPool,
    repo: AiKnowledgeRepository,
}

impl KnowledgeService {
    pub fn new(db: PgPool) -> Self {
        Self {
            repo: AiKnowledgeRepository::new(db.clone()),
            db,
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

    pub async fn delete_dataset_for_tenant(
        &self,
        tenant_id: i64,
        dataset_id: i64,
    ) -> Result<i64, AppError> {
        if dataset_id <= 0 {
            return Err(AppError::bad_request("知识库 ID 不合法"));
        }
        if !self.repo.dataset_exists(tenant_id, dataset_id).await? {
            return Err(AppError::NotFound);
        }
        self.repo
            .delete_dataset_cascade(tenant_id, dataset_id)
            .await?;
        Ok(dataset_id)
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
        let model_runtime = ModelRuntimeService::for_tenant(self.db.clone(), tenant_id);
        enrich_chunk_records_with_runtime_embeddings(&mut chunk_records, &model_runtime).await?;

        self.repo
            .create_document_ingestion(&document, &parser_job, &[], &chunk_records)
            .await?;
        self.upsert_chunks_to_milvus_after_ingestion(tenant_id, dataset_id, &chunk_records)
            .await?;
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
        let model_runtime = ModelRuntimeService::for_tenant(self.db.clone(), tenant_id);
        enrich_chunk_records_with_runtime_embeddings(&mut parts.chunks, &model_runtime).await?;
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
            .await?;
        Ok(document_id)
    }

    async fn upsert_chunks_to_milvus_after_ingestion(
        &self,
        tenant_id: i64,
        dataset_id: i64,
        chunks: &[ChunkSaveRecord],
    ) -> Result<(), AppError> {
        let strict = strict_live_rag_required();
        let Some(config) = MilvusSearchConfig::from_env() else {
            if strict {
                return Err(AppError::bad_request("Milvus 环境变量未配置完整"));
            }
            return Ok(());
        };
        let collection = match self.repo.get_vector_collection(tenant_id, dataset_id).await {
            Ok(Some(collection)) => collection,
            Ok(None) if strict => return Err(AppError::bad_request("Milvus collection 未就绪")),
            Ok(None) => return Ok(()),
            Err(err) => {
                if strict {
                    return Err(err);
                }
                tracing::warn!(error = %err, tenant_id, dataset_id, "Milvus collection lookup failed after ingestion");
                return Ok(());
            }
        };
        let Some(request) = milvus_upsert_request_for_collection(&collection, chunks) else {
            if strict {
                return Err(AppError::bad_request("Milvus 写入请求无法构造"));
            }
            return Ok(());
        };

        if let Err(err) = milvus_ensure_collection(&config, &collection).await {
            if strict {
                return Err(err);
            }
            tracing::warn!(error = %err, tenant_id, dataset_id, "Milvus collection ensure failed after ingestion");
            return Ok(());
        }

        if let Err(err) = milvus_upsert_chunks(&config, &request).await {
            if strict {
                return Err(err);
            }
            tracing::warn!(error = %err, tenant_id, dataset_id, "Milvus upsert failed after ingestion");
        }
        Ok(())
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
        let parser_outbox = parser_outbox_save_record(
            tenant_id,
            dataset_id,
            document_id,
            parser_job_id,
            user_id,
            &parser_request,
            now,
        );

        self.repo
            .create_document_parse_job(&document, &parser_job, &parser_outbox)
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
        let indexed_chunks = filter_indexed_chunks_by_document_ids(
            indexed_rag_chunks(chunk_records),
            &command.source_document_ids,
        );
        let vector_collection = self
            .repo
            .get_vector_collection(tenant_id, dataset_id)
            .await?;
        let generation_profile = rag_generation_profile(&indexed_chunks);
        let model_runtime = ModelRuntimeService::for_tenant(self.db.clone(), tenant_id);
        let retrieval_plan = build_rag_retrieval_plan(
            &command.question,
            &indexed_chunks,
            &model_runtime,
            command.answer_model_route_id.as_deref(),
        )
        .await;
        tracing::debug!(
            queries = ?retrieval_plan.queries,
            required_sections = ?retrieval_plan.required_sections,
            "RAG retrieval plan"
        );
        let hit_limit =
            retrieval_plan_hit_limit(&retrieval_plan, command.limit, &generation_profile);
        let candidate_limit = rerank_candidate_limit(hit_limit);
        let candidate_hits = retrieve_candidates_for_plan(
            &retrieval_plan,
            &model_runtime,
            tenant_id,
            dataset_id,
            vector_collection.as_ref(),
            &indexed_chunks,
            &command.source_document_ids,
            candidate_limit,
        )
        .await?;
        let hits = rerank_dataset_hits(
            &command.question,
            candidate_hits,
            hit_limit,
            &retrieval_plan,
            &model_runtime,
        )
        .await?;
        let indexed_hits = indexed_retrieval_hits(&hits, &indexed_chunks);
        let generated_answer = generate_rag_answer(
            &command.question,
            &hits,
            &model_runtime,
            command.answer_model_route_id.as_deref(),
            command.answer_instruction.as_deref(),
            &generation_profile,
        )
        .await?;
        let mut model_routes = rag_model_routes_for_runtime(&model_runtime).await;
        model_routes.answer_model_route = generated_answer.answer_model_route.clone();
        let answer = generated_answer.answer;
        let answer_model = generated_answer.answer_model;
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

        Ok(rag_ask_response(
            trace_id,
            answer,
            &model_routes,
            answer_model,
        ))
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

fn parser_outbox_save_record(
    tenant_id: i64,
    dataset_id: i64,
    document_id: i64,
    parser_job_id: i64,
    user_id: i64,
    parser_request: &Value,
    now: NaiveDateTime,
) -> ParserOutboxSaveRecord {
    let id = next_id();
    ParserOutboxSaveRecord {
        id,
        tenant_id,
        dataset_id,
        document_id,
        parser_job_id,
        event_type: PARSER_OUTBOX_EVENT_PARSE_REQUESTED.to_owned(),
        payload: json!({
            "outboxId": id,
            "tenantId": tenant_id,
            "datasetId": dataset_id,
            "documentId": document_id,
            "parserJobId": parser_job_id,
            "attempt": 1,
            "maxAttempts": PARSER_OUTBOX_MAX_ATTEMPTS,
            "parserRequest": parser_request,
        }),
        status: PARSER_OUTBOX_STATUS_PENDING,
        attempt_count: 0,
        user_id,
        now,
    }
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
    command.answer_model_route_id =
        normalize_optional_rag_model_route_id(command.answer_model_route_id)?;
    if command.limit == 0 {
        command.limit = DEFAULT_RAG_LIMIT;
    }
    command.limit = command.limit.min(MAX_RAG_LIMIT);
    command.source_document_ids = normalize_positive_ids(command.source_document_ids);
    if command.question.is_empty() {
        return Err(AppError::bad_request("问题不能为空"));
    }
    ensure_max_chars("问题", &command.question, 2000)?;
    Ok(command)
}

fn normalize_optional_rag_model_route_id(
    route_id: Option<String>,
) -> Result<Option<String>, AppError> {
    let route_id = route_id
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(route_id) = &route_id {
        ensure_max_chars("模型路由", route_id, 128)?;
    }
    Ok(route_id)
}

fn normalize_positive_ids(mut ids: Vec<i64>) -> Vec<i64> {
    ids.retain(|id| *id > 0);
    ids.sort_unstable();
    ids.dedup();
    ids
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

async fn enrich_chunk_records_with_runtime_embeddings(
    records: &mut [ChunkSaveRecord],
    model_runtime: &ModelRuntimeService,
) -> Result<(), AppError> {
    if records.is_empty() {
        return Ok(());
    }
    let strict = strict_live_rag_required();
    let Some(route) = model_runtime
        .resolve_route_for_purpose(ModelRoutePurpose::Embedding)
        .await?
    else {
        if strict {
            return Err(AppError::bad_request("Embedding 模型环境变量未配置完整"));
        }
        return Ok(());
    };
    let texts = records
        .iter()
        .map(|record| record.semantic_search_text.clone())
        .collect::<Vec<_>>();
    match ModelRuntimeService::embed_texts(&route, &texts).await {
        Ok(vectors) => {
            apply_embedding_vectors_to_chunk_records(
                records,
                &route.summary().route_id,
                "runtime",
                &vectors,
            );
            Ok(())
        }
        Err(err) if strict => Err(err),
        Err(_) => Ok(()),
    }
}

#[cfg(test)]
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

fn filter_indexed_chunks_by_document_ids(
    chunks: Vec<IndexedRagChunk>,
    document_ids: &[i64],
) -> Vec<IndexedRagChunk> {
    if document_ids.is_empty() {
        return chunks;
    }
    let allowed = document_ids.iter().copied().collect::<HashSet<_>>();
    chunks
        .into_iter()
        .filter(|chunk| allowed.contains(&chunk.document_id))
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
    model_runtime: &ModelRuntimeService,
    tenant_id: i64,
    dataset_id: i64,
    vector_collection: Option<&VectorCollectionRecord>,
    indexed_chunks: &[IndexedRagChunk],
    document_ids: &[i64],
    limit: usize,
) -> Result<Vec<RetrievalHit>, AppError> {
    hybrid_retrieve_indexed_chunks_with_milvus_or_local_strict(
        question,
        model_runtime,
        tenant_id,
        dataset_id,
        vector_collection,
        indexed_chunks,
        document_ids,
        limit,
        strict_live_rag_required(),
    )
    .await
}

async fn hybrid_retrieve_indexed_chunks_with_milvus_or_local_strict(
    question: &str,
    model_runtime: &ModelRuntimeService,
    tenant_id: i64,
    dataset_id: i64,
    vector_collection: Option<&VectorCollectionRecord>,
    indexed_chunks: &[IndexedRagChunk],
    document_ids: &[i64],
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

    let (query_embeddings, runtime_query_embedding) =
        retrieval_query_embeddings(question, model_runtime).await;
    if strict && runtime_query_embedding.is_none() {
        return Err(AppError::bad_request(
            "Runtime Embedding 模型未配置或调用失败",
        ));
    }

    let milvus_query_vector = runtime_query_embedding
        .clone()
        .or_else(|| query_embeddings.last().cloned());

    match (milvus_config, vector_collection, milvus_query_vector) {
        (Some(config), Some(collection), Some(query_vector)) => {
            let Some(request) = milvus_search_request_for_collection(
                tenant_id,
                dataset_id,
                collection,
                query_vector,
                document_ids,
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
                        return Err(AppError::bad_request("Milvus 检索未返回可用的已索引 chunk"));
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

async fn retrieval_query_embeddings(
    question: &str,
    model_runtime: &ModelRuntimeService,
) -> (Vec<Vec<f32>>, Option<Vec<f32>>) {
    let mut query_embeddings = vec![local_embedding_vector(question)];
    if let Some(runtime_embedding) = runtime_query_embedding(question, model_runtime).await {
        let runtime_query_embedding = Some(runtime_embedding.clone());
        query_embeddings.push(runtime_embedding);
        return (query_embeddings, runtime_query_embedding);
    }
    (query_embeddings, None)
}

async fn runtime_query_embedding(
    question: &str,
    model_runtime: &ModelRuntimeService,
) -> Option<Vec<f32>> {
    let route = model_runtime
        .resolve_route_for_purpose(ModelRoutePurpose::Embedding)
        .await
        .ok()??;
    let mut vectors = ModelRuntimeService::embed_texts(&route, &[question.to_owned()])
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
    document_ids: &[i64],
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
        .with_document_ids(document_ids.to_vec())
        .with_metric_type(milvus_metric_type(&collection.metric_type)),
    )
}

fn milvus_create_collection_request_for_collection(
    collection: &VectorCollectionRecord,
) -> Option<MilvusCreateCollectionRequest> {
    if collection.dimension <= 0
        || collection.status != VECTOR_COLLECTION_STATUS_READY
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

    Some(MilvusCreateCollectionRequest::new(
        provider_collection,
        collection.dimension as usize,
        milvus_metric_type(&collection.metric_type),
    ))
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

async fn milvus_ensure_collection(
    config: &MilvusSearchConfig,
    collection: &VectorCollectionRecord,
) -> Result<(), AppError> {
    let request = milvus_create_collection_request_for_collection(collection)
        .ok_or_else(|| AppError::bad_request("Milvus collection 创建请求无法构造"))?;
    let body = request.to_rest_create_body();
    let (status, payload) = milvus_post_json_payload(
        config,
        &config.create_collection_url(),
        &body,
        "Milvus collection 创建",
    )
    .await?;
    milvus_accept_idempotent_response(status, &payload, "Milvus collection 创建", &["exist"])?;
    milvus_load_collection(config, &request.collection_name).await
}

async fn milvus_load_collection(
    config: &MilvusSearchConfig,
    collection_name: &str,
) -> Result<(), AppError> {
    let body = json!({ "collectionName": collection_name });
    let (status, payload) = milvus_post_json_payload(
        config,
        &config.load_collection_url(),
        &body,
        "Milvus collection 加载",
    )
    .await?;
    milvus_accept_idempotent_response(status, &payload, "Milvus collection 加载", &["loaded"])
}

async fn milvus_post_json_payload(
    config: &MilvusSearchConfig,
    url: &str,
    body: &Value,
    operation: &str,
) -> Result<(reqwest::StatusCode, Value), AppError> {
    let client = milvus_http_client()?;
    let mut builder = client.post(url).json(body);
    if let Some(token) = config.token.as_deref() {
        builder = builder.bearer_auth(token);
    }

    let response = builder
        .send()
        .await
        .map_err(|err| AppError::bad_request(format!("{operation}请求失败: {err}")))?;
    let status = response.status();
    let payload = response.json::<Value>().await.unwrap_or(Value::Null);
    Ok((status, payload))
}

fn milvus_accept_idempotent_response(
    status: reqwest::StatusCode,
    payload: &Value,
    operation: &str,
    idempotent_needles: &[&str],
) -> Result<(), AppError> {
    let message = milvus_payload_message(payload).unwrap_or_else(|| "unknown error".to_owned());
    if !status.is_success() {
        if milvus_message_contains_any(&message, idempotent_needles) {
            return Ok(());
        }
        return Err(AppError::bad_request(format!(
            "{operation}请求失败: HTTP {status}"
        )));
    }

    if let Some(code) = payload.get("code").and_then(Value::as_i64) {
        if code != 0 && !milvus_message_contains_any(&message, idempotent_needles) {
            return Err(AppError::bad_request(format!("{operation}失败: {message}")));
        }
    }

    Ok(())
}

fn milvus_payload_message(payload: &Value) -> Option<String> {
    payload
        .get("message")
        .or_else(|| payload.get("msg"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn milvus_message_contains_any(message: &str, needles: &[&str]) -> bool {
    let message = message.to_ascii_lowercase();
    needles
        .iter()
        .any(|needle| message.contains(&needle.to_ascii_lowercase()))
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

async fn build_rag_retrieval_plan(
    question: &str,
    indexed_chunks: &[IndexedRagChunk],
    model_runtime: &ModelRuntimeService,
    answer_model_route_id: Option<&str>,
) -> RagRetrievalPlan {
    let fallback = RagRetrievalPlan::fallback(question);
    if fallback.queries.is_empty() {
        return fallback;
    }
    let outline = retrieval_plan_outline(indexed_chunks);
    if outline.is_empty() {
        return fallback;
    }

    let query_limit = retrieval_plan_query_limit(question);
    let command =
        retrieval_plan_chat_command(question, &outline, query_limit, answer_model_route_id);
    match model_runtime
        .chat_completion_for_purpose(ModelRoutePurpose::RagAnswer, command)
        .await
    {
        Ok(chat) => retrieval_plan_from_model_answer(&chat.answer, question),
        Err(err) => {
            tracing::warn!(error = %err, "RAG retrieval planner fell back to original question");
            fallback
        }
    }
}

fn retrieval_plan_chat_command(
    question: &str,
    outline: &str,
    query_limit: usize,
    answer_model_route_id: Option<&str>,
) -> ModelChatCommand {
    ModelChatCommand {
        conversation_id: None,
        route_id: answer_model_route_id.map(str::to_owned),
        messages: vec![
            ModelChatMessage {
                role: "system".to_owned(),
                content: format!(
                    "You are a retrieval planner for a grounded RAG system. Return strict JSON only. Do not answer the user. Produce at most {query_limit} focused search queries and at most 10 requiredSections that identify the document sections, topics, stages, modules, APIs, metrics, or constraints needed to answer. Use the provided section outline when helpful. If the question asks about a range, list, comparison, summary across stages, or multiple sections, include every matching section heading visible in the outline in requiredSections. Keep each string short. JSON shape: {{\"queries\":[\"...\"],\"requiredSections\":[\"...\"]}}."
                ),
            },
            ModelChatMessage {
                role: "user".to_owned(),
                content: format!(
                    "Question:\n{}\n\nAvailable section outline:\n{}\n\nReturn retrieval plan JSON only.",
                    question.trim(),
                    outline
                ),
            },
        ],
        file_contexts: vec![],
        response_format: None,
        temperature: Some(0.0),
        max_tokens: Some(1200),
        request_metadata: None,
        provider_call_context: None,
    }
}

fn retrieval_plan_outline(indexed_chunks: &[IndexedRagChunk]) -> String {
    let mut seen = HashSet::new();
    let mut lines = Vec::new();
    for chunk in indexed_chunks {
        let Some(line) = retrieval_plan_outline_line(&chunk.chunk) else {
            continue;
        };
        let key = normalized_section_match_text(&line);
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        lines.push(format!("- {line}"));
        if lines.len() >= MAX_RETRIEVAL_PLAN_OUTLINE_SECTIONS {
            break;
        }
    }
    lines.join("\n")
}

fn retrieval_plan_outline_line(chunk: &RagDocumentChunk) -> Option<String> {
    if !chunk.metadata.section_path.is_empty() {
        return Some(chunk.metadata.section_path.join(" / "));
    }
    let semantic = chunk.semantic_search_text.trim();
    semantic
        .lines()
        .find(|line| line.contains(" / ") || line.contains(':') || line.contains('：'))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
}

fn retrieval_plan_from_model_answer(answer: &str, question: &str) -> RagRetrievalPlan {
    let Some(json_text) = extract_json_object(answer) else {
        return RagRetrievalPlan::fallback(question);
    };
    let Ok(payload) = serde_json::from_str::<RagRetrievalPlanPayload>(&json_text) else {
        return RagRetrievalPlan::fallback(question);
    };
    normalize_retrieval_plan_payload(question, payload)
}

fn extract_json_object(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed.to_owned());
    }
    let fenced = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim);
    if let Some(value) = fenced {
        if value.starts_with('{') && value.ends_with('}') {
            return Some(value.to_owned());
        }
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    (end > start).then(|| trimmed[start..=end].to_owned())
}

fn normalize_retrieval_plan_payload(
    question: &str,
    payload: RagRetrievalPlanPayload,
) -> RagRetrievalPlan {
    let query_limit = retrieval_plan_query_limit(question);
    let mut queries = Vec::new();
    push_unique_plan_text(&mut queries, question, query_limit);
    for query in payload.queries {
        push_unique_plan_text(&mut queries, &query, query_limit);
    }

    let mut required_sections = Vec::new();
    for section in payload.required_sections {
        push_unique_plan_text(
            &mut required_sections,
            &section,
            MAX_RETRIEVAL_PLAN_REQUIRED_SECTIONS,
        );
    }
    for section in &required_sections {
        push_unique_plan_text(&mut queries, section, query_limit);
    }

    if queries.is_empty() {
        return RagRetrievalPlan::fallback(question);
    }
    RagRetrievalPlan {
        queries,
        required_sections,
    }
}

fn retrieval_plan_query_limit(question: &str) -> usize {
    if question_needs_extended_retrieval(question) {
        MAX_RETRIEVAL_PLAN_QUERIES
    } else {
        DEFAULT_RETRIEVAL_PLAN_QUERIES
    }
}

fn question_needs_extended_retrieval(question: &str) -> bool {
    let normalized = question.trim().to_ascii_lowercase();
    let multi_section_terms = [
        "比较", "对比", "分别", "逐个", "每个", "各个", "所有", "全部", "清单", "列表", "compare",
        "each", "every", "all", "range", "from ",
    ];
    if multi_section_terms
        .iter()
        .any(|term| normalized.contains(term))
    {
        return true;
    }

    let has_digit = normalized.chars().any(|ch| ch.is_ascii_digit());
    if !has_digit {
        return false;
    }
    let range_markers = ["到", "至", "~", "～", "..", "-", "－", "—"];
    if range_markers
        .iter()
        .any(|marker| normalized.contains(marker))
    {
        return true;
    }

    let list_markers = ["、", ",", "，", "/", "和"];
    normalized.chars().filter(|ch| ch.is_ascii_digit()).count() >= 2
        && list_markers
            .iter()
            .any(|marker| normalized.contains(marker))
}

fn push_unique_plan_text(items: &mut Vec<String>, value: &str, limit: usize) {
    if items.len() >= limit {
        return;
    }
    let normalized = value.trim().chars().take(160).collect::<String>();
    if normalized.is_empty() {
        return;
    }
    let key = normalized_section_match_text(&normalized);
    if key.is_empty()
        || items
            .iter()
            .any(|item| normalized_section_match_text(item) == key)
    {
        return;
    }
    items.push(normalized);
}

fn rag_generation_profile(indexed_chunks: &[IndexedRagChunk]) -> RagGenerationProfile {
    let total_tokens = indexed_chunks
        .iter()
        .map(|chunk| chunk.chunk.token_count)
        .sum::<usize>();
    if indexed_chunks.len() >= LARGE_DOCUMENT_CHUNK_THRESHOLD
        || total_tokens >= LARGE_DOCUMENT_TOKEN_THRESHOLD
    {
        RagGenerationProfile::large_document_loop()
    } else {
        RagGenerationProfile::single_pass()
    }
}

fn retrieval_plan_hit_limit(
    plan: &RagRetrievalPlan,
    requested_limit: usize,
    generation_profile: &RagGenerationProfile,
) -> usize {
    requested_limit
        .max(plan.required_sections.len())
        .max(generation_profile.target_hit_limit)
        .min(MAX_RAG_LIMIT)
}

async fn retrieve_candidates_for_plan(
    plan: &RagRetrievalPlan,
    model_runtime: &ModelRuntimeService,
    tenant_id: i64,
    dataset_id: i64,
    vector_collection: Option<&VectorCollectionRecord>,
    indexed_chunks: &[IndexedRagChunk],
    document_ids: &[i64],
    limit: usize,
) -> Result<Vec<RetrievalHit>, AppError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let queries = if plan.queries.is_empty() {
        Vec::new()
    } else {
        plan.queries.clone()
    };
    let mut hit_sets = Vec::new();
    for query in queries {
        let hits = hybrid_retrieve_indexed_chunks_with_milvus_or_local(
            &query,
            model_runtime,
            tenant_id,
            dataset_id,
            vector_collection,
            indexed_chunks,
            document_ids,
            limit,
        )
        .await?;
        hit_sets.push(hits);
    }
    let candidates = rank_fusion_retrieval_hits(hit_sets, limit);
    Ok(expand_candidates_with_required_sections(
        plan,
        candidates,
        indexed_chunks,
        limit,
    ))
}

fn rank_fusion_retrieval_hits(hit_sets: Vec<Vec<RetrievalHit>>, limit: usize) -> Vec<RetrievalHit> {
    let mut merged = HashMap::<String, (RetrievalHit, f32)>::new();
    for hits in hit_sets {
        for (index, hit) in hits.into_iter().enumerate() {
            let contribution = 1.0 / (60.0 + index as f32 + 1.0);
            let key = hit.chunk.chunk_id.clone();
            merged
                .entry(key)
                .and_modify(|(existing, score)| {
                    *score += contribution;
                    if hit.score > existing.score {
                        *existing = hit.clone();
                    }
                })
                .or_insert((hit, contribution));
        }
    }
    let mut hits = merged
        .into_values()
        .map(|(mut hit, score)| {
            hit.score = score;
            hit
        })
        .collect::<Vec<_>>();
    hits.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.chunk.chunk_index.cmp(&right.chunk.chunk_index))
    });
    hits.truncate(limit);
    rank_retrieval_hits(hits)
}

fn expand_candidates_with_required_sections(
    plan: &RagRetrievalPlan,
    candidates: Vec<RetrievalHit>,
    indexed_chunks: &[IndexedRagChunk],
    limit: usize,
) -> Vec<RetrievalHit> {
    if limit == 0 || plan.required_sections.is_empty() {
        return rank_retrieval_hits(candidates.into_iter().take(limit).collect());
    }

    let mut selected = Vec::new();
    let mut used = HashSet::new();
    for section in &plan.required_sections {
        if selected.len() >= limit {
            break;
        }
        let hit = best_required_section_candidate(section, &used, &candidates, indexed_chunks);
        if let Some(hit) = hit {
            used.insert(hit.chunk.chunk_id.clone());
            selected.push(hit);
        }
    }

    for hit in candidates {
        if selected.len() >= limit {
            break;
        }
        if used.insert(hit.chunk.chunk_id.clone()) {
            selected.push(hit);
        }
    }

    rank_retrieval_hits(selected)
}

fn best_required_section_candidate(
    section: &str,
    used: &HashSet<String>,
    candidates: &[RetrievalHit],
    indexed_chunks: &[IndexedRagChunk],
) -> Option<RetrievalHit> {
    let mut best = candidates
        .iter()
        .filter(|hit| !used.contains(hit.chunk.chunk_id.as_str()))
        .filter(|hit| hit_matches_required_section(hit, section))
        .cloned()
        .max_by(compare_required_section_hits);

    for indexed_chunk in indexed_chunks {
        if used.contains(indexed_chunk.chunk.chunk_id.as_str()) {
            continue;
        }
        let hit = retrieval_hit_from_indexed_chunk(indexed_chunk, 0.0);
        if !hit_matches_required_section(&hit, section) {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|current| compare_required_section_hits(&hit, current).is_gt())
        {
            best = Some(hit);
        }
    }

    best
}

fn compare_required_section_hits(left: &RetrievalHit, right: &RetrievalHit) -> std::cmp::Ordering {
    required_section_hit_quality(left)
        .partial_cmp(&required_section_hit_quality(right))
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| right.chunk.chunk_index.cmp(&left.chunk.chunk_index))
}

fn retrieval_hit_from_indexed_chunk(indexed_chunk: &IndexedRagChunk, score: f32) -> RetrievalHit {
    RetrievalHit {
        rank: 0,
        score,
        citation: indexed_chunk.chunk.citation.clone(),
        chunk: indexed_chunk.chunk.clone(),
    }
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
    plan: &RagRetrievalPlan,
    model_runtime: &ModelRuntimeService,
) -> Result<Vec<RetrievalHit>, AppError> {
    if hits.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let strict = strict_live_rag_required();
    let route = model_runtime
        .resolve_route_for_purpose(ModelRoutePurpose::Rerank)
        .await?;
    let Some(route) = route else {
        return if strict {
            Err(AppError::bad_request("Rerank 模型环境变量未配置完整"))
        } else {
            Ok(ensure_required_section_coverage(
                plan,
                rerank_retrieval_hits(&hits, &[], limit),
                &hits,
                limit,
            ))
        };
    };
    let documents = hits.iter().map(rerank_document_text).collect::<Vec<_>>();

    let reranked = match ModelRuntimeService::rerank_documents(&route, question, &documents).await {
        Ok(scores) if !scores.is_empty() => rerank_retrieval_hits(&hits, &scores, limit),
        Ok(_) if strict => return Err(AppError::bad_request("Rerank 模型响应为空")),
        Err(err) if strict => return Err(err),
        _ => rerank_retrieval_hits(&hits, &[], limit),
    };
    Ok(ensure_required_section_coverage(
        plan, reranked, &hits, limit,
    ))
}

#[cfg(test)]
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

fn ensure_required_section_coverage(
    plan: &RagRetrievalPlan,
    reranked: Vec<RetrievalHit>,
    candidates: &[RetrievalHit],
    limit: usize,
) -> Vec<RetrievalHit> {
    if limit == 0 {
        return Vec::new();
    }
    if plan.required_sections.is_empty() {
        return rank_retrieval_hits(reranked.into_iter().take(limit).collect());
    }

    let mut selected = Vec::new();
    let mut used = HashSet::new();
    for section in &plan.required_sections {
        if selected.len() >= limit {
            break;
        }
        let hit = candidates
            .iter()
            .filter(|hit| !used.contains(hit.chunk.chunk_id.as_str()))
            .filter(|hit| hit_matches_required_section(hit, section))
            .max_by(|left, right| compare_required_section_hits(left, right));
        if let Some(hit) = hit {
            used.insert(hit.chunk.chunk_id.clone());
            selected.push(hit.clone());
        }
    }

    for hit in reranked {
        if selected.len() >= limit {
            break;
        }
        if used.insert(hit.chunk.chunk_id.clone()) {
            selected.push(hit);
        }
    }

    rank_retrieval_hits(selected)
}

fn hit_matches_required_section(hit: &RetrievalHit, section: &str) -> bool {
    let section = normalized_section_match_text(section);
    if section.is_empty() {
        return false;
    }
    normalized_section_match_text(&rerank_document_text(hit)).contains(&section)
        || normalized_section_match_text(&hit.chunk.text).contains(&section)
        || hit
            .chunk
            .metadata
            .section_path
            .iter()
            .any(|path| normalized_section_match_text(path).contains(&section))
}

fn required_section_hit_quality(hit: &RetrievalHit) -> f32 {
    let text = rerank_document_text(hit);
    let normalized_len = normalized_section_match_text(&text).chars().count();
    let token_score = hit.chunk.token_count.min(300) as f32;
    let text_score = (normalized_len.min(1200) as f32) / 20.0;
    token_score + text_score + hit.score.max(0.0)
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

fn normalized_section_match_text(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_ascii_alphanumeric() || is_cjk_character(*character))
        .collect()
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RagAnswerMode {
    UseLlm,
    RequireLlm,
    ExtractiveFallback,
}

#[derive(Debug, Clone, PartialEq)]
struct GeneratedRagAnswer {
    answer: RagAnswer,
    answer_model_route: String,
    answer_model: Option<String>,
}

#[cfg(test)]
fn rag_answer_mode_from_config(config: &ModelRuntimeConfig, strict: bool) -> RagAnswerMode {
    if config.route(ModelRuntimeTarget::Llm).is_some() {
        RagAnswerMode::UseLlm
    } else if strict {
        RagAnswerMode::RequireLlm
    } else {
        RagAnswerMode::ExtractiveFallback
    }
}

async fn generate_rag_answer(
    question: &str,
    hits: &[RetrievalHit],
    model_runtime: &ModelRuntimeService,
    answer_model_route_id: Option<&str>,
    answer_instruction: Option<&str>,
    generation_profile: &RagGenerationProfile,
) -> Result<GeneratedRagAnswer, AppError> {
    let strict = strict_live_rag_required();
    let route = model_runtime
        .resolve_route_for_purpose_with_route_id(
            ModelRoutePurpose::RagAnswer,
            answer_model_route_id,
        )
        .await?;
    match (route, strict) {
        (Some(_), _) => {
            let answer_result = if generation_profile.uses_loop() && hits.len() > 1 {
                generate_large_document_rag_answer(
                    question,
                    hits,
                    model_runtime,
                    answer_model_route_id,
                    answer_instruction,
                    generation_profile,
                )
                .await
            } else {
                chat_completion_for_rag_answer_with_retry(
                    model_runtime,
                    rag_answer_chat_command(
                        question,
                        hits,
                        DEFAULT_RAG_ANSWER_MAX_TOKENS,
                        answer_model_route_id,
                        answer_instruction,
                    ),
                    "single-pass answer",
                )
                .await
                .map(|chat| (chat, "llm_grounded".to_owned()))
            };
            let (chat, answer_strategy) = match answer_result {
                Ok(result) => result,
                Err(err) if !strict => {
                    tracing::warn!(error = %err, "RAG LLM answer failed; falling back to extractive answer");
                    let mut answer = build_extractive_answer(question, hits);
                    answer.trace.answer_strategy = "extractive_fallback_after_llm_error".to_owned();
                    return Ok(GeneratedRagAnswer {
                        answer,
                        answer_model_route: novex_rag::LOCAL_ANSWER_ROUTE.to_owned(),
                        answer_model: None,
                    });
                }
                Err(err) => return Err(err),
            };
            let answer_model_route = chat.route_id.clone();
            let answer_model = chat.model.clone();
            let answer = rag_answer_from_model_chat(chat, hits, &answer_strategy);
            if answer.answer.trim().is_empty() {
                return Err(AppError::bad_request("LLM RAG 回答为空"));
            }
            Ok(GeneratedRagAnswer {
                answer,
                answer_model_route,
                answer_model,
            })
        }
        (None, true) => Err(AppError::bad_request("LLM 模型环境变量未配置完整")),
        (None, false) => Ok(GeneratedRagAnswer {
            answer: build_extractive_answer(question, hits),
            answer_model_route: novex_rag::LOCAL_ANSWER_ROUTE.to_owned(),
            answer_model: None,
        }),
    }
}

async fn generate_large_document_rag_answer(
    question: &str,
    hits: &[RetrievalHit],
    model_runtime: &ModelRuntimeService,
    answer_model_route_id: Option<&str>,
    answer_instruction: Option<&str>,
    generation_profile: &RagGenerationProfile,
) -> Result<(ModelChatResp, String), AppError> {
    let batches = large_document_hit_batches(hits, generation_profile);
    if batches.len() <= 1 {
        let chat = chat_completion_for_rag_answer_with_retry(
            model_runtime,
            rag_answer_chat_command(
                question,
                hits,
                DEFAULT_RAG_ANSWER_MAX_TOKENS,
                answer_model_route_id,
                answer_instruction,
            ),
            "large-document single-batch answer",
        )
        .await?;
        return Ok((chat, "llm_grounded".to_owned()));
    }

    let note_futures = batches.iter().enumerate().map(|(index, batch)| {
        let model_runtime = model_runtime.clone();
        let command = large_document_notes_chat_command(
            question,
            batch,
            index + 1,
            batches.len(),
            answer_model_route_id,
            answer_instruction,
        );
        async move {
            chat_completion_for_rag_answer_with_retry(
                &model_runtime,
                command,
                "large-document notes batch",
            )
            .await
            .map(|chat| chat.answer.trim().to_owned())
        }
    });
    let mut notes = Vec::new();
    for result in join_all(note_futures).await {
        match result {
            Ok(note) if !note.is_empty() => notes.push(note),
            Ok(_) => {}
            Err(err) => {
                tracing::warn!(error = %err, "RAG large-document notes batch failed");
            }
        }
    }

    let chat = chat_completion_for_rag_answer_with_retry(
        model_runtime,
        large_document_final_answer_chat_command(
            question,
            hits,
            &notes,
            answer_model_route_id,
            answer_instruction,
        ),
        "large-document final answer",
    )
    .await?;
    Ok((chat, "llm_grounded_loop".to_owned()))
}

async fn chat_completion_for_rag_answer_with_retry(
    model_runtime: &ModelRuntimeService,
    command: ModelChatCommand,
    label: &str,
) -> Result<ModelChatResp, AppError> {
    let mut last_error = None;
    for attempt in 1..=2 {
        match model_runtime
            .chat_completion_for_purpose(ModelRoutePurpose::RagAnswer, command.clone())
            .await
        {
            Ok(chat) => return Ok(chat),
            Err(err) => {
                tracing::warn!(
                    attempt,
                    error = %err,
                    "RAG LLM chat attempt failed: {label}"
                );
                last_error = Some(err);
                if attempt < 2 {
                    tokio::time::sleep(Duration::from_millis(800)).await;
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| AppError::bad_request("LLM RAG 回答为空")))
}

fn large_document_hit_batches<'a>(
    hits: &'a [RetrievalHit],
    generation_profile: &RagGenerationProfile,
) -> Vec<&'a [RetrievalHit]> {
    if hits.is_empty() {
        return Vec::new();
    }
    let batch_size = generation_profile.notes_batch_size.max(1);
    let max_batches = generation_profile.max_note_batches.max(1);
    hits.chunks(batch_size).take(max_batches).collect()
}

fn large_document_notes_chat_command(
    question: &str,
    hits: &[RetrievalHit],
    batch_index: usize,
    batch_count: usize,
    answer_model_route_id: Option<&str>,
    answer_instruction: Option<&str>,
) -> ModelChatCommand {
    let mut system_prompt = "You are one step in a large-document grounded RAG loop. Do not write the final answer. Extract the facts, narrative hooks, tensions, concrete examples, and useful citation labels from this batch only. Keep it compact, faithful, and reusable by a later final-writing step. If a chunk looks like OCR noise or malformed diagram text, mark it as low-value instead of relying on it."
        .to_owned();
    if let Some(instruction) = answer_instruction
        .map(str::trim)
        .filter(|instruction| !instruction.is_empty())
    {
        system_prompt.push_str("\n\nSkill instruction for evidence selection:\n");
        system_prompt.push_str(instruction);
    }

    ModelChatCommand {
        conversation_id: None,
        route_id: answer_model_route_id.map(str::to_owned),
        messages: vec![
            ModelChatMessage {
                role: "system".to_owned(),
                content: system_prompt,
            },
            ModelChatMessage {
                role: "user".to_owned(),
                content: format!(
                    "Original user request:\n{}\n\nBatch {batch_index}/{batch_count} context:\n{}\n\nReturn concise grounded notes with citation labels.",
                    question.trim(),
                    rag_context_sections(hits)
                ),
            },
        ],
        file_contexts: vec![],
        response_format: None,
        temperature: Some(0.1),
        max_tokens: Some(1200),
        request_metadata: None,
        provider_call_context: None,
    }
}

fn large_document_final_answer_chat_command(
    question: &str,
    hits: &[RetrievalHit],
    notes: &[String],
    answer_model_route_id: Option<&str>,
    answer_instruction: Option<&str>,
) -> ModelChatCommand {
    let mut system_prompt = "You are the final step of a large-document grounded RAG loop. Use the consolidated notes and selected citations to produce a complete answer. Prefer a finished, publishable response over a short fragment. Preserve user-requested writing style. Do not expose internal notes. Avoid raw citation labels unless they are useful for factual support; if the user asks for a publishable article, make citations unobtrusive and do not let them break the prose."
        .to_owned();
    if let Some(instruction) = answer_instruction
        .map(str::trim)
        .filter(|instruction| !instruction.is_empty())
    {
        system_prompt.push_str("\n\nSkill instruction:\n");
        system_prompt.push_str(instruction);
    }

    ModelChatCommand {
        conversation_id: None,
        route_id: answer_model_route_id.map(str::to_owned),
        messages: vec![
            ModelChatMessage {
                role: "system".to_owned(),
                content: system_prompt,
            },
            ModelChatMessage {
                role: "user".to_owned(),
                content: large_document_final_context_message(question, hits, notes),
            },
        ],
        file_contexts: vec![],
        response_format: None,
        temperature: Some(0.2),
        max_tokens: Some(DEFAULT_RAG_ANSWER_MAX_TOKENS),
        request_metadata: None,
        provider_call_context: None,
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
    answer_model_route_id: Option<&str>,
    answer_instruction: Option<&str>,
) -> ModelChatCommand {
    let mut system_prompt = "Only answer from the provided context. You may synthesize, compare, summarize, and judge whether a plan is reasonable when the judgment can be grounded in the provided context. Prefer a clear, useful answer over a terse fragment. If the context lacks enough evidence, say what is missing instead of inventing facts. Mention the supporting citation labels like [1] only when they directly support a sentence. Do not invent citations."
        .to_owned();
    if let Some(instruction) = answer_instruction
        .map(str::trim)
        .filter(|instruction| !instruction.is_empty())
    {
        system_prompt.push_str("\n\nSkill instruction:\n");
        system_prompt.push_str(instruction);
    }

    ModelChatCommand {
        conversation_id: None,
        route_id: answer_model_route_id.map(str::to_owned),
        messages: vec![
            ModelChatMessage {
                role: "system".to_owned(),
                content: system_prompt,
            },
            ModelChatMessage {
                role: "user".to_owned(),
                content: rag_context_message(question, hits),
            },
        ],
        file_contexts: vec![],
        response_format: None,
        temperature: Some(0.2),
        max_tokens: Some(max_tokens),
        request_metadata: None,
        provider_call_context: None,
    }
}

fn rag_context_message(question: &str, hits: &[RetrievalHit]) -> String {
    let mut message = format!("Question:\n{}\n\nContext:\n", question.trim());
    if hits.is_empty() {
        message.push_str("(no retrieved context)");
        return message;
    }

    message.push_str(&rag_context_sections(hits));
    message.push_str("\nAnswer with the facts supported by the context above.");
    message
}

fn rag_context_sections(hits: &[RetrievalHit]) -> String {
    let mut message = String::new();
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

    message
}

fn large_document_final_context_message(
    question: &str,
    hits: &[RetrievalHit],
    notes: &[String],
) -> String {
    let mut message = format!("Original user request:\n{}\n\n", question.trim());
    if notes.is_empty() {
        message.push_str("Consolidated notes:\n(no intermediate notes)\n\n");
    } else {
        message.push_str("Consolidated notes from document batches:\n");
        for (index, note) in notes.iter().enumerate() {
            message.push_str(&format!("\n[Batch note {}]\n{}\n", index + 1, note.trim()));
        }
        message.push('\n');
    }
    message.push_str("Citation map for final grounding:\n");
    message.push_str(&rag_context_sections(hits));
    message.push_str("\nWrite the final answer now.");
    message
}

fn rag_answer_from_model_chat(
    chat: ModelChatResp,
    hits: &[RetrievalHit],
    answer_strategy: &str,
) -> RagAnswer {
    RagAnswer {
        answer: chat.answer.trim().to_owned(),
        citations: citations_from_retrieval_hits(hits),
        trace: novex_rag::RagTraceSnapshot {
            retrieval_hit_count: hits.len(),
            answer_strategy: answer_strategy.to_owned(),
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

async fn rag_model_routes_for_runtime(model_runtime: &ModelRuntimeService) -> RagModelRoutes {
    let fallback = rag_model_routes();
    let embedding_model_route =
        resolved_route_id_for_purpose(model_runtime, ModelRoutePurpose::Embedding)
            .await
            .unwrap_or(fallback.embedding_model_route);
    let rerank_model_route =
        resolved_route_id_for_purpose(model_runtime, ModelRoutePurpose::Rerank)
            .await
            .unwrap_or(fallback.rerank_model_route);
    let answer_model_route =
        resolved_route_id_for_purpose(model_runtime, ModelRoutePurpose::RagAnswer)
            .await
            .unwrap_or(fallback.answer_model_route);

    RagModelRoutes {
        embedding_model_route,
        rerank_model_route,
        answer_model_route,
    }
}

async fn resolved_route_id_for_purpose(
    model_runtime: &ModelRuntimeService,
    purpose: ModelRoutePurpose,
) -> Option<String> {
    model_runtime
        .resolve_route_for_purpose(purpose)
        .await
        .ok()
        .flatten()
        .map(|route| route.route_id().to_owned())
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

fn rag_ask_response(
    trace_id: i64,
    answer: RagAnswer,
    model_routes: &RagModelRoutes,
    answer_model: Option<String>,
) -> RagAskResp {
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
        embedding_model_route: model_routes.embedding_model_route.clone(),
        rerank_model_route: model_routes.rerank_model_route.clone(),
        answer_model_route: model_routes.answer_model_route.clone(),
        answer_model,
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
    use sqlx::postgres::PgPoolOptions;

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
    fn parser_outbox_record_wraps_parser_worker_request_payload() {
        let command = normalize_document_parse_job_command(DocumentParseJobCommand {
            name: "training.pdf".to_owned(),
            file_id: Some(88),
            source_uri: "/file/knowledge/88.pdf".to_owned(),
            content_type: "application/pdf".to_owned(),
            source_hash: "file-hash".to_owned(),
            source_kind: "objectStorage".to_owned(),
        })
        .unwrap();
        let parser_request = parser_worker_request(DEFAULT_TENANT_ID, 7, 42, 99, &command);
        let now = Utc::now().naive_utc();

        let outbox =
            parser_outbox_save_record(DEFAULT_TENANT_ID, 7, 42, 99, 9, &parser_request, now);

        assert_eq!(outbox.tenant_id, DEFAULT_TENANT_ID);
        assert_eq!(outbox.dataset_id, 7);
        assert_eq!(outbox.document_id, 42);
        assert_eq!(outbox.parser_job_id, 99);
        assert_eq!(outbox.event_type, "parser.job.requested");
        assert_eq!(outbox.payload["parserRequest"], parser_request);
        assert_eq!(outbox.payload["maxAttempts"], 5);
        assert_eq!(outbox.payload["attempt"], 1);
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
        assert_eq!(
            config.create_collection_url(),
            "http://localhost:19530/v2/vectordb/collections/create"
        );
        assert_eq!(
            config.load_collection_url(),
            "http://localhost:19530/v2/vectordb/collections/load"
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

        let request =
            milvus_search_request_for_collection(42, 7, &collection, vec![0.1, 0.2], &[21, 22], 4)
                .expect("valid collection should build a milvus request");
        let body = request.to_rest_search_body();

        assert_eq!(body["collectionName"], "novex_t42_dataset_7");
        assert_eq!(
            body["filter"],
            "tenant_id == 42 and dataset_id == 7 and document_id in [21, 22]"
        );
        assert_eq!(body["searchParams"]["metric_type"], "IP");
        assert!(
            milvus_search_request_for_collection(42, 7, &collection, vec![0.1], &[], 4).is_none()
        );
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
    fn milvus_create_collection_request_uses_mapping_dimension_and_metric() {
        let collection = VectorCollectionRecord {
            id: 100,
            vector_backend: "milvus".to_owned(),
            provider_collection: "novex_t42_dataset_7".to_owned(),
            dimension: 1024,
            metric_type: "ip".to_owned(),
            status: VECTOR_COLLECTION_STATUS_READY,
        };

        let request = milvus_create_collection_request_for_collection(&collection)
            .expect("valid collection should build a milvus create request");
        let body = request.to_rest_create_body();

        assert_eq!(body["collectionName"], "novex_t42_dataset_7");
        assert!(body["schema"]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field["fieldName"] == "embedding"
                && field["elementTypeParams"]["dim"] == 1024));
        assert_eq!(body["indexParams"][0]["metricType"], "IP");
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

        let resp = rag_ask_response(
            42,
            answer,
            &RagModelRoutes {
                embedding_model_route: "runtime.embedding".to_owned(),
                rerank_model_route: "runtime.reranker".to_owned(),
                answer_model_route: "runtime.llm.rag_answer".to_owned(),
            },
            Some("deepseek-v4-flash".to_owned()),
        );

        assert_eq!(resp.trace_id, 42);
        assert_eq!(resp.answer, "Training starts on Monday.");
        assert_eq!(resp.citations.len(), 1);
        assert_eq!(resp.citations[0].document_id, "7");
        assert_eq!(resp.embedding_model_route, "runtime.embedding");
        assert_eq!(resp.rerank_model_route, "runtime.reranker");
        assert_eq!(resp.answer_model_route, "runtime.llm.rag_answer");
        assert_eq!(resp.answer_model.as_deref(), Some("deepseek-v4-flash"));
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

    #[test]
    fn rag_dynamic_model_calls_use_tenant_bound_runtime_service() {
        let source = include_str!("knowledge_service.rs");

        assert!(source.contains("ModelRuntimeService::for_tenant(self.db.clone(), tenant_id)"));
        assert!(source.contains("ModelRoutePurpose::Embedding"));
        assert!(source.contains("ModelRoutePurpose::Rerank"));
        assert!(source.contains("ModelRoutePurpose::RagAnswer"));
        let legacy_static_call = [
            "ModelRuntimeService::chat_completion",
            "(rag_answer_chat_command",
        ]
        .concat();
        assert!(!source.contains(&legacy_static_call));
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

        let command = rag_answer_chat_command("When does training start?", &hits, 512, None, None);

        assert_eq!(command.temperature, Some(0.2));
        assert_eq!(command.max_tokens, Some(512));
        assert_eq!(command.messages.len(), 2);
        assert!(command.messages[0]
            .content
            .contains("Only answer from the provided context"));
        assert!(command.messages[0]
            .content
            .contains("synthesize, compare, summarize, and judge"));
        assert!(command.messages[1]
            .content
            .contains("Question:\nWhen does training start?"));
        assert!(command.messages[1].content.contains("[1] chunk-a"));
        assert!(command.messages[1]
            .content
            .contains("Training starts on Monday."));
    }

    #[test]
    fn rag_answer_prompt_includes_selected_skill_instruction() {
        let hits = vec![test_retrieval_hit(
            "chunk-a",
            0,
            "Training starts on Monday.",
        )];

        let command = rag_answer_chat_command(
            "Build a quiz.",
            &hits,
            512,
            None,
            Some("Create three quiz questions and keep every answer cited."),
        );

        assert!(command.messages[0].content.contains("Skill instruction:"));
        assert!(command.messages[0]
            .content
            .contains("Create three quiz questions and keep every answer cited."));
    }

    #[test]
    fn rag_answer_default_token_budget_supports_long_form_skills() {
        let hits = vec![test_retrieval_hit(
            "chunk-a",
            0,
            "Training starts on Monday.",
        )];

        let command = rag_answer_chat_command(
            "Write a long-form article with this skill.",
            &hits,
            DEFAULT_RAG_ANSWER_MAX_TOKENS,
            None,
            Some("Write a complete article and do not stop mid sentence."),
        );

        assert!(command.max_tokens.unwrap_or_default() >= 4096);
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
            provider: "deep-seek".to_owned(),
            model: Some("deepseek-test".to_owned()),
            latency_ms: 12,
            usage: ModelChatUsage::default(),
            cost_cents: None,
            provider_attempts: vec![],
            provider_call_lease_id: None,
        };

        let answer = rag_answer_from_model_chat(chat, &hits, "llm_grounded");

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
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://novex:novex@localhost:5432/novex_test")
            .expect("lazy postgres pool");
        let model_runtime = ModelRuntimeService::for_tenant(db, 42);

        let err = hybrid_retrieve_indexed_chunks_with_milvus_or_local_strict(
            "When does training start?",
            &model_runtime,
            42,
            7,
            None,
            &chunks,
            &[],
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
    fn service_deletes_dataset_after_validating_tenant_scope() {
        let source = include_str!("knowledge_service.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .unwrap();

        for needle in [
            "delete_dataset_for_tenant",
            "dataset_id <= 0",
            "dataset_exists(tenant_id, dataset_id)",
            "delete_dataset_cascade(tenant_id, dataset_id)",
        ] {
            assert!(
                source.contains(needle),
                "{needle} missing from knowledge service"
            );
        }
    }

    #[test]
    fn rag_trace_record_uses_explicit_live_model_routes() {
        let command = RagAskCommand {
            question: "When does training start?".to_owned(),
            limit: 3,
            ..RagAskCommand::default()
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
        assert_eq!(
            trace.embedding_model_route.as_deref(),
            Some("runtime.embedding")
        );
        assert_eq!(
            trace.rerank_model_route.as_deref(),
            Some("runtime.reranker")
        );
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
    fn retrieval_plan_parser_normalizes_llm_json() {
        let plan = retrieval_plan_from_model_answer(
            r#"```json
            {
              "queries": ["知识库 MVP", "Agent Runtime", "客户交付模板"],
              "requiredSections": ["Foundation Skeleton", "Knowledge MVP", "Eval"]
            }
            ```"#,
            "总结 M0 到 M5 方案是否合理",
        );

        assert_eq!(
            plan.queries,
            vec![
                "总结 M0 到 M5 方案是否合理",
                "知识库 MVP",
                "Agent Runtime",
                "客户交付模板",
                "Foundation Skeleton",
                "Knowledge MVP",
                "Eval"
            ]
        );
        assert_eq!(
            plan.required_sections,
            vec!["Foundation Skeleton", "Knowledge MVP", "Eval"]
        );
    }

    #[test]
    fn retrieval_plan_caps_generic_summary_queries() {
        let plan = retrieval_plan_from_model_answer(
            r#"{
              "queries": [
                "Novex 本地链路验证文档 主题总结",
                "Novex AI Agent Foundation Architecture 主题 概述",
                "Novex 本地链路验证文档",
                "Novex AI Agent Foundation Architecture / 1. 定位",
                "Novex AI Agent Foundation Architecture / 2. 设计原则",
                "Novex AI Agent Foundation Architecture / 3. 总体架构",
                "Novex AI Agent Foundation Architecture / 22. 结论"
              ],
              "requiredSections": [
                "Novex 本地链路验证文档",
                "Novex AI Agent Foundation Architecture / 1. 定位",
                "Novex AI Agent Foundation Architecture / 2. 设计原则",
                "Novex AI Agent Foundation Architecture / 3. 总体架构",
                "Novex AI Agent Foundation Architecture / 22. 结论"
              ]
            }"#,
            "总结一下主题",
        );

        assert_eq!(plan.queries.len(), DEFAULT_RETRIEVAL_PLAN_QUERIES);
        assert_eq!(plan.queries[0], "总结一下主题");
        assert_eq!(plan.required_sections.len(), 5);
    }

    #[test]
    fn retrieval_plan_hit_limit_uses_required_sections() {
        let plan = RagRetrievalPlan {
            queries: vec!["总结方案是否合理".to_owned()],
            required_sections: vec![
                "Foundation".to_owned(),
                "Knowledge".to_owned(),
                "Runtime".to_owned(),
                "Eval".to_owned(),
                "Delivery".to_owned(),
                "Security".to_owned(),
            ],
        };

        let profile = RagGenerationProfile::single_pass();
        assert_eq!(retrieval_plan_hit_limit(&plan, 5, &profile), 6);
        assert_eq!(
            retrieval_plan_hit_limit(&RagRetrievalPlan::fallback("总结一下架构"), 5, &profile),
            5
        );
    }

    #[test]
    fn retrieval_plan_hit_limit_respects_small_document_explicit_limit() {
        let profile = RagGenerationProfile::single_pass();

        assert_eq!(
            retrieval_plan_hit_limit(&RagRetrievalPlan::fallback("只要两个证据"), 2, &profile),
            2
        );
    }

    #[test]
    fn large_document_retrieval_reads_enough_chunks_for_pdf_scale() {
        assert!(MAX_LOCAL_RETRIEVAL_CHUNKS >= 3000);
    }

    #[test]
    fn large_document_generation_profile_enables_loop_and_more_hits() {
        let chunks = (0..1482)
            .map(|index| {
                test_indexed_rag_chunk(
                    &format!("chunk-{index}"),
                    index,
                    "置身钉内 ONE 产品定位 让事找人 组织上下文 AI 工作信息流",
                )
            })
            .collect::<Vec<_>>();

        let profile = rag_generation_profile(&chunks);

        assert_eq!(profile.mode, RagGenerationMode::LargeDocumentLoop);
        assert!(profile.target_hit_limit >= 20);
        assert!(profile.notes_batch_size >= 4);
    }

    #[test]
    fn retrieval_plan_hit_limit_expands_large_document_context() {
        let profile = RagGenerationProfile {
            mode: RagGenerationMode::LargeDocumentLoop,
            target_hit_limit: 20,
            notes_batch_size: 5,
            max_note_batches: 4,
        };

        assert_eq!(
            retrieval_plan_hit_limit(&RagRetrievalPlan::fallback("写一篇公众号文章"), 5, &profile),
            20
        );
    }

    #[test]
    fn large_document_loop_batches_hits_for_section_notes() {
        let hits = (0..13)
            .map(|index| test_retrieval_hit(&format!("chunk-{index}"), index, "ONE 产品资料"))
            .collect::<Vec<_>>();
        let profile = RagGenerationProfile {
            mode: RagGenerationMode::LargeDocumentLoop,
            target_hit_limit: 20,
            notes_batch_size: 5,
            max_note_batches: 4,
        };

        let batches = large_document_hit_batches(&hits, &profile);

        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0][0].chunk.chunk_id, "chunk-0");
        assert_eq!(batches[2][2].chunk.chunk_id, "chunk-12");
    }

    #[test]
    fn large_document_notes_prompt_keeps_skill_instruction() {
        let hits = vec![test_retrieval_hit(
            "chunk-1",
            1,
            "ONE 是工作信息流入口，适合公众号故事化表达。",
        )];

        let command = large_document_notes_chat_command(
            "写一篇公众号文章",
            &hits,
            1,
            1,
            Some("runtime.llm.rag_answer"),
            Some("用故事化结构，先讲人和场景，再讲产品判断。"),
        );

        assert_eq!(command.route_id.as_deref(), Some("runtime.llm.rag_answer"));
        assert!(command.messages[0]
            .content
            .contains("Skill instruction for evidence selection"));
        assert!(command.messages[0].content.contains("故事化结构"));
    }

    #[test]
    fn retrieval_plan_outline_keeps_late_document_sections() {
        let chunks = (0..180)
            .map(|index| {
                let text = if index == 170 {
                    "Architecture / 16. 里程碑 / Eval"
                } else {
                    "Architecture / earlier section"
                };
                test_indexed_rag_chunk(&format!("chunk-{index}"), index, text)
            })
            .collect::<Vec<_>>();

        let outline = retrieval_plan_outline(&chunks);

        assert!(outline.contains("Architecture / 16. 里程碑 / Eval"));
    }

    #[test]
    fn required_section_coverage_keeps_planner_sections_after_rerank() {
        let candidates = vec![
            test_retrieval_hit("foundation", 0, "Foundation Skeleton 基建骨架"),
            test_retrieval_hit("knowledge", 1, "Knowledge MVP 知识库闭环"),
            test_retrieval_hit("runtime", 2, "Agent Runtime 工具循环"),
            test_retrieval_hit("eval-heading", 3, "Eval 交付："),
            test_retrieval_hit(
                "eval-content",
                4,
                "Eval eval dataset eval case eval runner RAG 指标 回归报告",
            ),
            test_retrieval_hit("delivery", 4, "Delivery Template 客户交付"),
            test_retrieval_hit("generic", 99, "泛化方案片段"),
        ];
        let reranked = vec![
            candidates[5].clone(),
            candidates[0].clone(),
            candidates[4].clone(),
        ];
        let plan = RagRetrievalPlan {
            queries: vec!["总结方案是否合理".to_owned()],
            required_sections: vec![
                "Foundation Skeleton".to_owned(),
                "Knowledge MVP".to_owned(),
                "Agent Runtime".to_owned(),
                "Eval".to_owned(),
                "Delivery Template".to_owned(),
            ],
        };

        let covered = ensure_required_section_coverage(&plan, reranked, &candidates, 5);
        let ids = covered
            .iter()
            .map(|hit| hit.chunk.chunk_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![
                "foundation",
                "knowledge",
                "runtime",
                "eval-content",
                "delivery"
            ]
        );
    }

    #[test]
    fn required_section_candidate_expansion_adds_informative_section_chunks() {
        let plan = RagRetrievalPlan {
            queries: vec!["总结方案是否合理".to_owned()],
            required_sections: vec!["M4: Eval".to_owned()],
        };
        let candidates = vec![
            test_retrieval_hit("eval-heading", 10, "Architecture / M4: Eval\n交付："),
            test_retrieval_hit("generic", 99, "泛化方案片段"),
        ];
        let indexed_chunks = vec![
            test_indexed_rag_chunk("eval-heading", 10, "Architecture / M4: Eval\n交付："),
            test_indexed_rag_chunk(
                "eval-content",
                11,
                "Architecture / M4: Eval\neval dataset。 eval case。 eval runner。 RAG 指标。 intent 指标。 tool 指标。 回归报告。",
            ),
            test_indexed_rag_chunk("generic", 99, "泛化方案片段"),
        ];

        let expanded =
            expand_candidates_with_required_sections(&plan, candidates, &indexed_chunks, 3);

        assert_eq!(expanded[0].chunk.chunk_id, "eval-content");
        assert!(expanded[0].chunk.text.contains("eval runner"));
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
