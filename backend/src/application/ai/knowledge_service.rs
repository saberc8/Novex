use std::collections::HashMap;

use chrono::{NaiveDateTime, Utc};
use novex_model::{ModelRuntimeConfig, ModelRuntimeTarget};
use novex_rag::{
    build_extractive_answer, chunk_text, keyword_retrieve, parse_plain_text, CitationRef,
    DocumentChunk as RagDocumentChunk, RagAnswer, RetrievalHit,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::{
    application::system::{ensure_max_chars, format_datetime, format_optional_datetime},
    infrastructure::persistence::ai_knowledge_repository::{
        AiKnowledgeRepository, ChunkRecord, ChunkSaveRecord, DatasetFilter, DatasetRecord,
        DatasetSaveRecord, DocumentFilter, DocumentRecord, DocumentSaveRecord, ParserJobSaveRecord,
        RagTraceHitSaveRecord, RagTraceSaveRecord,
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
const DOCUMENT_INGESTION_STATUS_INDEXED: i16 = 4;
const PARSER_JOB_TYPE_TEXT: i16 = 1;
const PARSER_JOB_STATUS_SUCCEEDED: i16 = 3;
const CHUNK_EMBEDDING_STATUS_INDEXED: i16 = 4;
const DEFAULT_RAG_LIMIT: usize = 5;
const MAX_RAG_LIMIT: usize = 10;
const MAX_LOCAL_RETRIEVAL_CHUNKS: i64 = 500;
const LOCAL_EMBEDDING_ROUTE: &str = "local-keyword";
const LOCAL_RERANK_ROUTE: &str = "none";
const LOCAL_ANSWER_ROUTE: &str = "local-extractive";

#[derive(Debug, Clone, PartialEq, Eq)]
struct RagModelRoutes {
    embedding_model_route: String,
    rerank_model_route: String,
    answer_model_route: String,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone)]
struct IndexedRagChunk {
    chunk_db_id: i64,
    document_id: i64,
    chunk: RagDocumentChunk,
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
        let page = query.page_query();
        let filter = DatasetFilter {
            tenant_id: DEFAULT_TENANT_ID,
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

    pub async fn create_dataset(
        &self,
        user_id: i64,
        command: DatasetCommand,
    ) -> Result<i64, AppError> {
        let command = normalize_dataset_command(command)?;
        let id = next_id();
        let record = dataset_save_record(id, user_id, &command);
        self.repo.create_dataset(&record).await?;
        Ok(id)
    }

    pub async fn list_documents(
        &self,
        dataset_id: i64,
        query: DocumentQuery,
    ) -> Result<PageResult<DocumentResp>, AppError> {
        if dataset_id <= 0 {
            return Err(AppError::bad_request("知识库 ID 不合法"));
        }
        if !self
            .repo
            .dataset_exists(DEFAULT_TENANT_ID, dataset_id)
            .await?
        {
            return Err(AppError::NotFound);
        }
        let page = query.page_query();
        let filter = DocumentFilter {
            tenant_id: DEFAULT_TENANT_ID,
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
        if dataset_id <= 0 {
            return Err(AppError::bad_request("知识库 ID 不合法"));
        }
        if !self
            .repo
            .dataset_exists(DEFAULT_TENANT_ID, dataset_id)
            .await?
        {
            return Err(AppError::NotFound);
        }
        let command = normalize_document_upload_command(command)?;
        let document_id = next_id();
        let chunks = document_upload_chunks(document_id, &command);
        let now = Utc::now().naive_utc();
        let document = DocumentSaveRecord {
            id: document_id,
            tenant_id: DEFAULT_TENANT_ID,
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
            tenant_id: DEFAULT_TENANT_ID,
            dataset_id,
            document_id,
            job_type: PARSER_JOB_TYPE_TEXT,
            status: PARSER_JOB_STATUS_SUCCEEDED,
            result_summary: json!({
                "parser": "novex-rag-local-text",
                "lineCount": command.content.lines().filter(|line| !line.trim().is_empty()).count(),
                "chunkCount": chunks.len()
            }),
            user_id,
            now,
        };
        let chunk_records = chunks
            .into_iter()
            .map(|chunk| ChunkSaveRecord {
                id: next_id(),
                tenant_id: DEFAULT_TENANT_ID,
                dataset_id,
                document_id,
                chunk_uid: chunk.chunk_id,
                chunk_index: chunk.chunk_index as i32,
                content: chunk.text,
                token_count: chunk.token_count as i32,
                citation: citation_value(&chunk.citation),
                embedding_status: CHUNK_EMBEDDING_STATUS_INDEXED,
                user_id,
                now,
            })
            .collect::<Vec<_>>();

        self.repo
            .create_document_ingestion(&document, &parser_job, &chunk_records)
            .await?;
        Ok(document_id)
    }

    pub async fn ask_dataset(
        &self,
        user_id: i64,
        dataset_id: i64,
        command: RagAskCommand,
    ) -> Result<RagAskResp, AppError> {
        if dataset_id <= 0 {
            return Err(AppError::bad_request("知识库 ID 不合法"));
        }
        if !self
            .repo
            .dataset_exists(DEFAULT_TENANT_ID, dataset_id)
            .await?
        {
            return Err(AppError::NotFound);
        }
        let command = normalize_rag_ask_command(command)?;
        let chunk_records = self
            .repo
            .list_indexed_chunks(DEFAULT_TENANT_ID, dataset_id, MAX_LOCAL_RETRIEVAL_CHUNKS)
            .await?;
        let indexed_chunks = indexed_rag_chunks(chunk_records);
        let rag_chunks = indexed_chunks
            .iter()
            .map(|chunk| chunk.chunk.clone())
            .collect::<Vec<_>>();
        let hits = keyword_retrieve(&command.question, &rag_chunks, command.limit);
        let indexed_hits = indexed_retrieval_hits(&hits, &indexed_chunks);
        let answer = build_extractive_answer(&command.question, &hits);
        let trace_id = next_id();
        let now = Utc::now().naive_utc();
        let trace = rag_trace_record(
            trace_id,
            user_id,
            dataset_id,
            &command,
            &answer,
            &indexed_hits,
            now,
        );
        let trace_hits =
            rag_trace_hit_records(trace_id, DEFAULT_TENANT_ID, dataset_id, &indexed_hits, now);

        self.repo.create_rag_trace(&trace, &trace_hits).await?;

        Ok(rag_ask_response(trace_id, answer))
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

fn document_upload_chunks(
    document_id: i64,
    command: &DocumentUploadCommand,
) -> Vec<RagDocumentChunk> {
    let parsed = parse_plain_text(document_id.to_string(), &command.content);
    chunk_text(
        &parsed,
        DEFAULT_CHUNK_MAX_CHARS,
        DEFAULT_CHUNK_OVERLAP_CHARS,
    )
}

fn indexed_rag_chunks(records: Vec<ChunkRecord>) -> Vec<IndexedRagChunk> {
    records
        .into_iter()
        .map(|record| {
            let citation =
                citation_from_value(record.document_id, &record.chunk_uid, &record.citation);
            IndexedRagChunk {
                chunk_db_id: record.id,
                document_id: record.document_id,
                chunk: RagDocumentChunk {
                    document_id: record.document_id.to_string(),
                    chunk_id: record.chunk_uid,
                    chunk_index: record.chunk_index.max(0) as usize,
                    text: record.content,
                    token_count: record.token_count.max(0) as usize,
                    citation,
                },
            }
        })
        .collect()
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

fn rag_trace_record(
    trace_id: i64,
    user_id: i64,
    dataset_id: i64,
    command: &RagAskCommand,
    answer: &RagAnswer,
    hits: &[IndexedRetrievalHit],
    now: NaiveDateTime,
) -> RagTraceSaveRecord {
    let model_routes = rag_model_routes();
    RagTraceSaveRecord {
        id: trace_id,
        tenant_id: DEFAULT_TENANT_ID,
        dataset_id,
        question: command.question.clone(),
        answer: answer.answer.clone(),
        answer_strategy: answer.trace.answer_strategy.clone(),
        retrieval_mode: RETRIEVAL_MODE_HYBRID,
        embedding_model_route: Some(model_routes.embedding_model_route),
        rerank_model_route: Some(model_routes.rerank_model_route),
        answer_model_route: Some(model_routes.answer_model_route),
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
    RagModelRoutes {
        embedding_model_route: runtime_route_id(config, ModelRuntimeTarget::Embedding)
            .unwrap_or_else(|| LOCAL_EMBEDDING_ROUTE.to_owned()),
        rerank_model_route: runtime_route_id(config, ModelRuntimeTarget::Reranker)
            .unwrap_or_else(|| LOCAL_RERANK_ROUTE.to_owned()),
        answer_model_route: runtime_route_id(config, ModelRuntimeTarget::Llm)
            .unwrap_or_else(|| LOCAL_ANSWER_ROUTE.to_owned()),
    }
}

fn runtime_route_id(config: &ModelRuntimeConfig, target: ModelRuntimeTarget) -> Option<String> {
    config.route(target).map(|route| route.summary().route_id)
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
    id: i64,
    user_id: i64,
    command: &'a DatasetCommand,
) -> DatasetSaveRecord<'a> {
    DatasetSaveRecord {
        id,
        tenant_id: DEFAULT_TENANT_ID,
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
        assert_eq!(routes.rerank_model_route, LOCAL_RERANK_ROUTE);
        assert_eq!(routes.answer_model_route, LOCAL_ANSWER_ROUTE);
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
