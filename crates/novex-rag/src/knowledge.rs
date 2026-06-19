use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeResourceKind {
    Dataset,
    Document,
    Chunk,
    Citation,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetStatus {
    #[default]
    Draft,
    Published,
    Archived,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceVisibility {
    #[default]
    Private,
    Tenant,
    Public,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalMode {
    Vector,
    Keyword,
    #[default]
    Hybrid,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentParseStatus {
    #[default]
    Pending,
    Parsing,
    Parsed,
    Failed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionStatus {
    #[default]
    Pending,
    Chunking,
    Embedding,
    Indexed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CitationRef {
    pub document_id: String,
    pub chunk_id: String,
    pub page_no: Option<i32>,
    pub section_path: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkSegmentType {
    #[default]
    Text,
    Table,
    Image,
}

impl ChunkSegmentType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Table => "table",
            Self::Image => "image",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentRole {
    #[default]
    Canonical,
    SummaryFaq,
    TestCase,
}

impl ContentRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Canonical => "canonical",
            Self::SummaryFaq => "summary_faq",
            Self::TestCase => "test_case",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayCapability {
    PreciseAnchor,
    RowOnly,
    #[default]
    TextOnly,
}

impl DisplayCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PreciseAnchor => "precise_anchor",
            Self::RowOnly => "row_only",
            Self::TextOnly => "text_only",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundingBox {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBlock {
    pub text: String,
    pub segment_type: ChunkSegmentType,
    pub page_no: Option<i32>,
    pub section_path: Vec<String>,
    pub table_header: Vec<String>,
    pub image_access_keys: Vec<String>,
    pub bbox: Option<BoundingBox>,
}

impl SourceBlock {
    pub fn text(text: impl Into<String>, page_no: Option<i32>, section_path: Vec<String>) -> Self {
        Self {
            text: text.into(),
            segment_type: ChunkSegmentType::Text,
            page_no,
            section_path,
            table_header: vec![],
            image_access_keys: vec![],
            bbox: None,
        }
    }

    pub fn table(text: impl Into<String>, table_header: Vec<String>) -> Self {
        Self {
            text: text.into(),
            segment_type: ChunkSegmentType::Table,
            page_no: None,
            section_path: vec![],
            table_header,
            image_access_keys: vec![],
            bbox: None,
        }
    }

    pub fn image(
        text: impl Into<String>,
        page_no: Option<i32>,
        section_path: Vec<String>,
        image_access_keys: Vec<String>,
        bbox: Option<BoundingBox>,
    ) -> Self {
        Self {
            text: text.into(),
            segment_type: ChunkSegmentType::Image,
            page_no,
            section_path,
            table_header: vec![],
            image_access_keys,
            bbox,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkMetadata {
    pub source_title: Option<String>,
    pub source_file_name: Option<String>,
    pub source_content_type: Option<String>,
    pub segment_type: ChunkSegmentType,
    pub segment_index: usize,
    pub page_no: Option<i32>,
    pub section_path: Vec<String>,
    pub table_header: Vec<String>,
    pub image_access_keys: Vec<String>,
    pub bbox: Option<BoundingBox>,
    pub content_role: ContentRole,
    pub display_capability: DisplayCapability,
}

impl Default for ChunkMetadata {
    fn default() -> Self {
        Self {
            source_title: None,
            source_file_name: None,
            source_content_type: None,
            segment_type: ChunkSegmentType::Text,
            segment_index: 0,
            page_no: None,
            section_path: vec![],
            table_header: vec![],
            image_access_keys: vec![],
            bbox: None,
            content_role: ContentRole::Canonical,
            display_capability: DisplayCapability::TextOnly,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedDocument {
    pub document_id: String,
    pub text: String,
    pub content_type: String,
    pub source_title: Option<String>,
    pub source_file_name: Option<String>,
    pub blocks: Vec<SourceBlock>,
    pub line_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentChunk {
    pub document_id: String,
    pub chunk_id: String,
    pub chunk_index: usize,
    pub text: String,
    pub semantic_search_text: String,
    pub token_count: usize,
    pub citation: CitationRef,
    pub metadata: ChunkMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalHit {
    pub rank: usize,
    pub score: f32,
    pub chunk: DocumentChunk,
    pub citation: CitationRef,
}
