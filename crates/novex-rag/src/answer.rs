use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::knowledge::{CitationRef, RetrievalHit};

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
