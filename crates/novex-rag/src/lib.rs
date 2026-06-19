mod answer;
mod chunk;
mod knowledge;
mod milvus;
mod model_routes;
mod parse;
mod retrieval;

use novex_ai_core::FoundationModule;

pub use answer::{build_extractive_answer, RagAnswer, RagTraceSnapshot};
pub use chunk::{build_semantic_search_text, chunk_document, chunk_text};
pub use knowledge::{
    BoundingBox, ChunkMetadata, ChunkSegmentType, CitationRef, ContentRole, DatasetStatus,
    DisplayCapability, DocumentChunk, DocumentParseStatus, IngestionStatus, KnowledgeResourceKind,
    ParsedDocument, ResourceVisibility, RetrievalHit, RetrievalMode, SourceBlock,
};
pub use milvus::{
    parse_milvus_search_hits, MilvusCreateCollectionRequest, MilvusMetricType, MilvusSearchHit,
    MilvusSearchRequest, MilvusUpsertRequest, MilvusUpsertRow,
};
pub use model_routes::{
    RagModelRoutes, LOCAL_ANSWER_ROUTE, LOCAL_EMBEDDING_ROUTE, LOCAL_RERANK_ROUTE,
};
pub use parse::{parse_document_content, parse_plain_text};
pub use retrieval::keyword_retrieve;

pub const CRATE_ID: &str = "novex-rag";

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "RAG",
        "ai-foundation",
        "Knowledge datasets, documents, chunks, retrieval, rerank, context, and citation boundaries.",
    )
}
