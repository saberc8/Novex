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
pub enum RetrievalMode {
    Vector,
    Keyword,
    Hybrid,
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
}
