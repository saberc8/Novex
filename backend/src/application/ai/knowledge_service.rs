use chrono::Utc;
use novex_rag::{chunk_text, parse_plain_text, DocumentChunk as RagDocumentChunk};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::{
    application::system::{ensure_max_chars, format_datetime, format_optional_datetime},
    infrastructure::persistence::ai_knowledge_repository::{
        AiKnowledgeRepository, ChunkSaveRecord, DatasetFilter, DatasetRecord, DatasetSaveRecord,
        DocumentFilter, DocumentRecord, DocumentSaveRecord, ParserJobSaveRecord,
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
const DEFAULT_CHUNK_MAX_CHARS: usize = 24;
const DEFAULT_CHUNK_OVERLAP_CHARS: usize = 4;
const DOCUMENT_PARSE_STATUS_PARSED: i16 = 3;
const DOCUMENT_INGESTION_STATUS_INDEXED: i16 = 4;
const PARSER_JOB_TYPE_TEXT: i16 = 1;
const PARSER_JOB_STATUS_SUCCEEDED: i16 = 3;
const CHUNK_EMBEDDING_STATUS_INDEXED: i16 = 4;

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

fn citation_value(citation: &novex_rag::CitationRef) -> Value {
    json!({
        "documentId": citation.document_id,
        "chunkId": citation.chunk_id,
        "pageNo": citation.page_no,
        "sectionPath": citation.section_path,
    })
}

fn sha256_hex(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
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
            content: "Alpha beta gamma delta epsilon zeta eta theta.".to_owned(),
            ..DocumentUploadCommand::default()
        };
        let command = normalize_document_upload_command(command).unwrap();

        let chunks = document_upload_chunks(42, &command);

        assert!(chunks.len() > 1);
        assert_eq!(chunks[0].document_id, "42");
        assert_eq!(chunks[0].chunk_id, "42:0");
        assert_eq!(chunks[0].chunk_index, 0);
    }
}
