use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedDocument {
    pub document_id: String,
    pub text: String,
    pub line_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentChunk {
    pub document_id: String,
    pub chunk_id: String,
    pub chunk_index: usize,
    pub text: String,
    pub token_count: usize,
    pub citation: CitationRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalHit {
    pub rank: usize,
    pub score: f32,
    pub chunk: DocumentChunk,
    pub citation: CitationRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RagTraceSnapshot {
    pub retrieval_hit_count: usize,
    pub answer_strategy: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RagAnswer {
    pub answer: String,
    pub citations: Vec<CitationRef>,
    pub trace: RagTraceSnapshot,
}

pub fn parse_plain_text(document_id: impl Into<String>, text: &str) -> ParsedDocument {
    let text = text.trim().replace("\r\n", "\n");
    let line_count = text.lines().filter(|line| !line.trim().is_empty()).count();
    ParsedDocument {
        document_id: document_id.into(),
        text,
        line_count,
    }
}

pub fn chunk_text(
    document: &ParsedDocument,
    max_chars: usize,
    overlap_chars: usize,
) -> Vec<DocumentChunk> {
    let text = document.text.trim();
    if text.is_empty() {
        return vec![];
    }

    let max_chars = max_chars.max(1);
    let overlap_chars = overlap_chars.min(max_chars.saturating_sub(1));
    let chars = text.chars().collect::<Vec<_>>();
    let step = max_chars.saturating_sub(overlap_chars).max(1);
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < chars.len() {
        let end = (start + max_chars).min(chars.len());
        let chunk_text = chars[start..end]
            .iter()
            .collect::<String>()
            .trim()
            .to_owned();
        if !chunk_text.is_empty() {
            let chunk_index = chunks.len();
            let chunk_id = format!("{}:{chunk_index}", document.document_id);
            let token_count = tokenize(&chunk_text).len();
            chunks.push(DocumentChunk {
                document_id: document.document_id.clone(),
                chunk_id: chunk_id.clone(),
                chunk_index,
                text: chunk_text,
                token_count,
                citation: CitationRef {
                    document_id: document.document_id.clone(),
                    chunk_id,
                    page_no: None,
                    section_path: vec![],
                },
            });
        }
        if end == chars.len() {
            break;
        }
        start += step;
    }

    chunks
}

pub fn keyword_retrieve(query: &str, chunks: &[DocumentChunk], limit: usize) -> Vec<RetrievalHit> {
    if limit == 0 {
        return vec![];
    }
    let query_tokens = tokenize(query).into_iter().collect::<HashSet<_>>();
    if query_tokens.is_empty() {
        return vec![];
    }

    let mut scored = chunks
        .iter()
        .filter_map(|chunk| {
            let chunk_tokens = tokenize(&chunk.text).into_iter().collect::<HashSet<_>>();
            let overlap = query_tokens.intersection(&chunk_tokens).count();
            if overlap == 0 {
                return None;
            }
            let score = overlap as f32 / query_tokens.len() as f32;
            Some((score, chunk.clone()))
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

pub fn build_extractive_answer(question: &str, hits: &[RetrievalHit]) -> RagAnswer {
    if hits.is_empty() {
        return RagAnswer {
            answer: format!("No relevant context found for: {}", question.trim()),
            citations: vec![],
            trace: RagTraceSnapshot {
                retrieval_hit_count: 0,
                answer_strategy: "extractive".to_owned(),
            },
        };
    }

    let answer = hits
        .iter()
        .take(3)
        .map(|hit| first_sentence(&hit.chunk.text))
        .collect::<Vec<_>>()
        .join("\n");
    let mut seen = HashSet::new();
    let citations = hits
        .iter()
        .filter_map(|hit| {
            if seen.insert(hit.citation.chunk_id.clone()) {
                Some(hit.citation.clone())
            } else {
                None
            }
        })
        .collect();

    RagAnswer {
        answer,
        citations,
        trace: RagTraceSnapshot {
            retrieval_hit_count: hits.len(),
            answer_strategy: "extractive".to_owned(),
        },
    }
}

fn first_sentence(text: &str) -> String {
    let text = text.trim();
    text.split_inclusive(['.', '!', '?', '。', '！', '？'])
        .next()
        .unwrap_or(text)
        .trim()
        .to_owned()
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if character.is_alphanumeric() {
            for lower in character.to_lowercase() {
                current.push(lower);
            }
            continue;
        }
        if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
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
        assert_eq!(DocumentParseStatus::default(), DocumentParseStatus::Pending);
    }

    #[test]
    fn chunk_text_splits_non_empty_text_into_ordered_chunks() {
        let parsed = parse_plain_text("doc-1", "Alpha beta gamma delta epsilon zeta eta theta.");
        let chunks = chunk_text(&parsed, 24, 4);

        assert!(chunks.len() > 1);
        assert_eq!(chunks[0].chunk_index, 0);
        assert_eq!(chunks[1].chunk_index, 1);
        assert_eq!(chunks[0].chunk_id, "doc-1:0");
        assert_eq!(chunks[0].citation.document_id, "doc-1");
        assert!(!chunks[0].text.is_empty());
    }

    #[test]
    fn keyword_retrieve_returns_ranked_hits_with_citations() {
        let parsed = parse_plain_text(
            "doc-2",
            "Onboarding policy covers training and mentors.\nExpense policy covers reimbursements.",
        );
        let chunks = chunk_text(&parsed, 48, 0);

        let hits = keyword_retrieve("onboarding training", &chunks, 2);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rank, 1);
        assert!(hits[0].score > 0.0);
        assert!(hits[0].chunk.text.contains("Onboarding"));
        assert_eq!(hits[0].citation.document_id, "doc-2");
    }

    #[test]
    fn build_extractive_answer_returns_answer_and_citations() {
        let parsed = parse_plain_text(
            "doc-3",
            "Training starts on Monday. Mentors review progress weekly.",
        );
        let chunks = chunk_text(&parsed, 80, 0);
        let hits = keyword_retrieve("When does training start?", &chunks, 3);

        let answer = build_extractive_answer("When does training start?", &hits);

        assert!(answer.answer.contains("Training starts on Monday"));
        assert_eq!(answer.citations.len(), 1);
        assert_eq!(answer.trace.retrieval_hit_count, 1);
        assert_eq!(answer.trace.answer_strategy, "extractive");
    }
}
