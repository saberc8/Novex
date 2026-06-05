use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-rag";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeResourceKind {
    Dataset,
    Document,
    Chunk,
    Citation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetStatus {
    Draft,
    Published,
    Archived,
}

impl Default for DatasetStatus {
    fn default() -> Self {
        Self::Draft
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceVisibility {
    Private,
    Tenant,
    Public,
}

impl Default for ResourceVisibility {
    fn default() -> Self {
        Self::Private
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalMode {
    Vector,
    Keyword,
    Hybrid,
}

impl Default for RetrievalMode {
    fn default() -> Self {
        Self::Hybrid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentParseStatus {
    Pending,
    Parsing,
    Parsed,
    Failed,
}

impl Default for DocumentParseStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionStatus {
    Pending,
    Chunking,
    Embedding,
    Indexed,
    Failed,
}

impl Default for IngestionStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CitationRef {
    pub document_id: String,
    pub chunk_id: String,
    pub page_no: Option<i32>,
    pub section_path: Vec<String>,
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "RAG",
        "ai-foundation",
        "Knowledge datasets, documents, chunks, retrieval, rerank, context, and citation boundaries.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::FoundationStatus;

    #[test]
    fn module_describes_rag_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-rag");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }

    #[test]
    fn knowledge_metadata_defaults_match_m1_control_plane() {
        assert_eq!(DatasetStatus::default(), DatasetStatus::Draft);
        assert_eq!(ResourceVisibility::default(), ResourceVisibility::Private);
        assert_eq!(RetrievalMode::default(), RetrievalMode::Hybrid);
        assert_eq!(
            DocumentParseStatus::default(),
            DocumentParseStatus::Pending
        );
    }
}
