use std::collections::{HashMap, HashSet};

use unicode_normalization::UnicodeNormalization;

use crate::knowledge::{DocumentChunk, RetrievalHit};

pub fn keyword_retrieve(query: &str, chunks: &[DocumentChunk], limit: usize) -> Vec<RetrievalHit> {
    if limit == 0 {
        return vec![];
    }
    bm25_retrieve(query, chunks, limit)
}

const BM25_K1: f32 = 1.2;
const BM25_B: f32 = 0.75;

fn bm25_retrieve(query: &str, chunks: &[DocumentChunk], limit: usize) -> Vec<RetrievalHit> {
    let query_terms = bm25_query_terms(query);
    if query_terms.is_empty() {
        return vec![];
    }

    let documents = chunks
        .iter()
        .map(|chunk| {
            let search_text = if chunk.semantic_search_text.trim().is_empty() {
                &chunk.text
            } else {
                &chunk.semantic_search_text
            };
            let tokens = bm25_tokens(search_text);
            let term_frequencies = term_frequencies(&tokens);
            (chunk, tokens.len().max(1) as f32, term_frequencies)
        })
        .collect::<Vec<_>>();
    if documents.is_empty() {
        return vec![];
    }

    let document_count = documents.len() as f32;
    let average_document_len = documents
        .iter()
        .map(|(_, length, _)| *length)
        .sum::<f32>()
        .max(1.0)
        / document_count.max(1.0);
    let mut document_frequencies = HashMap::<String, usize>::new();
    for (_, _, frequencies) in &documents {
        for term in frequencies.keys() {
            *document_frequencies.entry(term.clone()).or_default() += 1;
        }
    }

    let mut scored = documents
        .into_iter()
        .filter_map(|(chunk, document_len, frequencies)| {
            let mut score = 0.0;
            for term in &query_terms {
                let Some(term_frequency) = frequencies.get(term).copied() else {
                    continue;
                };
                let document_frequency = document_frequencies.get(term).copied().unwrap_or(0);
                if document_frequency == 0 {
                    continue;
                }
                score += bm25_term_score(
                    term_frequency as f32,
                    document_frequency as f32,
                    document_count,
                    document_len,
                    average_document_len,
                );
            }

            if score <= 0.0 {
                return None;
            }
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

fn query_token_set(query: &str) -> HashSet<String> {
    let mut tokens = bm25_tokens(query).into_iter().collect::<HashSet<_>>();
    expand_numbered_range_tokens(query, &mut tokens);
    tokens
}

fn bm25_query_terms(query: &str) -> Vec<String> {
    let mut terms = query_token_set(query).into_iter().collect::<Vec<_>>();
    terms.sort();
    terms
}

fn term_frequencies(tokens: &[String]) -> HashMap<String, usize> {
    let mut frequencies = HashMap::new();
    for token in tokens {
        *frequencies.entry(token.clone()).or_default() += 1;
    }
    frequencies
}

fn bm25_term_score(
    term_frequency: f32,
    document_frequency: f32,
    document_count: f32,
    document_len: f32,
    average_document_len: f32,
) -> f32 {
    let idf = (((document_count - document_frequency + 0.5) / (document_frequency + 0.5)) + 1.0)
        .ln()
        .max(0.0);
    let normalized_len = document_len / average_document_len.max(1.0);
    let denominator = term_frequency + BM25_K1 * (1.0 - BM25_B + BM25_B * normalized_len);
    idf * (term_frequency * (BM25_K1 + 1.0)) / denominator.max(f32::EPSILON)
}

fn expand_numbered_range_tokens(query: &str, tokens: &mut HashSet<String>) {
    if !contains_range_indicator(query) {
        return;
    }
    let mut labels = tokens
        .iter()
        .filter_map(|token| numbered_label_token(token))
        .collect::<Vec<_>>();
    labels.sort_unstable();
    labels.dedup();

    for pair in labels.windows(2) {
        let [start, end] = pair else {
            continue;
        };
        if start.prefix != end.prefix
            || end.number <= start.number
            || end.number - start.number > 12
        {
            continue;
        }
        for number in start.number..=end.number {
            tokens.insert(format!("{}{number}", start.prefix));
        }
    }
}

fn contains_range_indicator(query: &str) -> bool {
    let query = query.to_ascii_lowercase();
    query.contains("到")
        || query.contains("至")
        || query.contains('-')
        || query.contains('~')
        || query.contains('～')
        || query.contains("to")
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NumberedLabel {
    prefix: String,
    number: u8,
}

fn numbered_label_token(token: &str) -> Option<NumberedLabel> {
    let split_at = token.find(|character: char| character.is_ascii_digit())?;
    let (prefix, number) = token.split_at(split_at);
    if prefix.is_empty()
        || !prefix
            .chars()
            .all(|character| character.is_ascii_alphabetic())
        || number.is_empty()
        || !number.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }
    Some(NumberedLabel {
        prefix: prefix.to_owned(),
        number: number.parse::<u8>().ok()?,
    })
}

fn bm25_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current_ascii = String::new();
    let mut current_cjk = Vec::new();

    for character in text.nfkc().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            flush_cjk_bm25_tokens(&mut current_cjk, &mut tokens);
            current_ascii.push(character);
            continue;
        }

        if !current_ascii.is_empty() {
            tokens.push(std::mem::take(&mut current_ascii));
        }

        if is_cjk_character(character) {
            current_cjk.push(character);
        } else {
            flush_cjk_bm25_tokens(&mut current_cjk, &mut tokens);
        }
    }

    if !current_ascii.is_empty() {
        tokens.push(current_ascii);
    }
    flush_cjk_bm25_tokens(&mut current_cjk, &mut tokens);
    tokens
}

fn flush_cjk_bm25_tokens(current_cjk: &mut Vec<char>, tokens: &mut Vec<String>) {
    if current_cjk.is_empty() {
        return;
    }

    for character in current_cjk.iter().copied() {
        if !is_low_value_cjk_unigram(character) {
            tokens.push(character.to_string());
        }
    }

    for pair in current_cjk.windows(2) {
        tokens.push(pair.iter().collect());
    }

    current_cjk.clear();
}

fn is_low_value_cjk_unigram(character: char) -> bool {
    matches!(
        character,
        '的' | '了'
            | '是'
            | '在'
            | '和'
            | '与'
            | '及'
            | '或'
            | '而'
            | '这'
            | '那'
            | '什'
            | '么'
            | '个'
            | '一'
            | '不'
            | '有'
            | '到'
            | '从'
            | '中'
            | '里'
            | '上'
            | '下'
            | '为'
            | '被'
            | '把'
    )
}

fn is_cjk_character(character: char) -> bool {
    matches!(
        character as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF
    )
}
