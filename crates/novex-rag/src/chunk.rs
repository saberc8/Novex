use std::collections::HashSet;

use crate::knowledge::{
    ChunkMetadata, ChunkSegmentType, CitationRef, ContentRole, DisplayCapability, DocumentChunk,
    ParsedDocument, SourceBlock,
};

pub fn chunk_text(
    document: &ParsedDocument,
    max_chars: usize,
    overlap_chars: usize,
) -> Vec<DocumentChunk> {
    chunk_document(document, max_chars, overlap_chars)
}

pub fn chunk_document(
    document: &ParsedDocument,
    max_chars: usize,
    overlap_chars: usize,
) -> Vec<DocumentChunk> {
    if document.text.trim().is_empty() {
        return vec![];
    }

    let max_chars = max_chars.max(1);
    let overlap_chars = overlap_chars.min(max_chars.saturating_sub(1));
    let mut chunks = Vec::new();

    for (segment_index, block) in document.blocks.iter().enumerate() {
        let parts = match block.segment_type {
            ChunkSegmentType::Table => split_table_block(block, max_chars),
            ChunkSegmentType::Text | ChunkSegmentType::Image => {
                split_text_block(&block.text, max_chars, overlap_chars)
            }
        };

        for part in parts {
            if part.trim().is_empty() {
                continue;
            }
            push_document_chunk(document, block, segment_index, part, &mut chunks);
        }
    }

    chunks
}

pub fn build_semantic_search_text(raw_text: &str, metadata: &ChunkMetadata) -> String {
    let mut parts = Vec::new();
    let mut seen = HashSet::new();
    append_search_part(metadata.source_title.as_deref(), &mut parts, &mut seen);
    append_search_part(metadata.source_file_name.as_deref(), &mut parts, &mut seen);
    if !metadata.section_path.is_empty() {
        append_search_part(
            Some(&metadata.section_path.join(" / ")),
            &mut parts,
            &mut seen,
        );
    }
    if !metadata.table_header.is_empty() {
        append_search_part(
            Some(&metadata.table_header.join(" ")),
            &mut parts,
            &mut seen,
        );
    }
    let cleaned_text = clean_search_text(raw_text);
    append_search_part(Some(&cleaned_text), &mut parts, &mut seen);
    parts.join("\n")
}

fn split_table_block(block: &SourceBlock, max_chars: usize) -> Vec<String> {
    let lines = block
        .text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return vec![];
    }

    let header_line = lines[0];
    let mut chunks = Vec::new();
    let mut current = header_line.to_owned();
    for row in lines.iter().skip(1) {
        let single_row_chunk = format!("{header_line}\n{row}");
        if single_row_chunk.chars().count() > max_chars {
            if current != header_line {
                chunks.push(std::mem::replace(&mut current, header_line.to_owned()));
            }
            chunks.extend(split_oversized_table_row(header_line, row, max_chars));
            continue;
        }

        let candidate = format!("{current}\n{row}");
        if current != header_line && candidate.chars().count() > max_chars {
            chunks.push(std::mem::replace(&mut current, header_line.to_owned()));
        }
        current.push('\n');
        current.push_str(row);
    }
    if current != header_line || lines.len() == 1 {
        chunks.push(current);
    }
    chunks
}

fn split_text_block(text: &str, max_chars: usize, overlap_chars: usize) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return vec![];
    }

    if text.chars().count() <= max_chars {
        return vec![text.to_owned()];
    }

    let sentence_units = split_sentence_units(text);
    if sentence_units.len() > 1 {
        let mut chunks = Vec::new();
        let mut current = String::new();
        for sentence in sentence_units {
            if sentence.chars().count() > max_chars {
                if !current.is_empty() {
                    chunks.push(std::mem::take(&mut current));
                }
                chunks.extend(split_by_chars(&sentence, max_chars, overlap_chars));
                continue;
            }

            if current.is_empty() {
                current = sentence;
                continue;
            }

            let candidate = join_text_units(&current, &sentence);
            if candidate.chars().count() <= max_chars {
                current = candidate;
            } else {
                chunks.push(std::mem::replace(&mut current, sentence));
            }
        }
        if !current.is_empty() {
            chunks.push(current);
        }
        return chunks;
    }

    split_by_chars(text, max_chars, overlap_chars)
}

fn split_oversized_table_row(header_line: &str, row: &str, max_chars: usize) -> Vec<String> {
    let prefix = format!("{header_line}\n");
    let prefix_len = prefix.chars().count();
    let row_budget = max_chars.saturating_sub(prefix_len).max(1);
    split_by_chars(row, row_budget, 0)
        .into_iter()
        .map(|part| format!("{prefix}{part}"))
        .collect()
}

fn split_sentence_units(text: &str) -> Vec<String> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut units = Vec::new();
    let mut current = String::new();

    for (index, character) in chars.iter().enumerate() {
        current.push(*character);
        if is_sentence_boundary(&chars, index) {
            let unit = current.trim();
            if !unit.is_empty() {
                units.push(unit.to_owned());
            }
            current.clear();
        }
    }

    let unit = current.trim();
    if !unit.is_empty() {
        units.push(unit.to_owned());
    }
    units
}

fn is_sentence_boundary(chars: &[char], index: usize) -> bool {
    let character = chars[index];
    if matches!(character, '。' | '！' | '？' | '!' | '?') {
        return true;
    }
    if character != '.' {
        return false;
    }
    chars
        .get(index + 1)
        .map(|next| next.is_whitespace())
        .unwrap_or(true)
}

fn join_text_units(left: &str, right: &str) -> String {
    if left.ends_with('\n') || right.starts_with('\n') {
        format!("{left}{right}")
    } else {
        format!("{left} {right}")
    }
}

fn split_by_chars(text: &str, max_chars: usize, overlap_chars: usize) -> Vec<String> {
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
            chunks.push(chunk_text);
        }
        if end == chars.len() {
            break;
        }
        start += step;
    }

    chunks
}

fn push_document_chunk(
    document: &ParsedDocument,
    block: &SourceBlock,
    segment_index: usize,
    text: String,
    chunks: &mut Vec<DocumentChunk>,
) {
    let chunk_index = chunks.len();
    let chunk_id = format!("{}:{chunk_index}", document.document_id);
    let metadata = chunk_metadata(document, block, segment_index);
    let semantic_search_text = build_semantic_search_text(&text, &metadata);
    let token_count = tokenize(&semantic_search_text).len();
    chunks.push(DocumentChunk {
        document_id: document.document_id.clone(),
        chunk_id: chunk_id.clone(),
        chunk_index,
        text,
        semantic_search_text,
        token_count,
        citation: CitationRef {
            document_id: document.document_id.clone(),
            chunk_id,
            page_no: block.page_no,
            section_path: block.section_path.clone(),
        },
        metadata,
    });
}

fn chunk_metadata(
    document: &ParsedDocument,
    block: &SourceBlock,
    segment_index: usize,
) -> ChunkMetadata {
    ChunkMetadata {
        source_title: document.source_title.clone(),
        source_file_name: document.source_file_name.clone(),
        source_content_type: Some(document.content_type.clone()),
        segment_type: block.segment_type,
        segment_index,
        page_no: block.page_no,
        section_path: block.section_path.clone(),
        table_header: block.table_header.clone(),
        image_access_keys: block.image_access_keys.clone(),
        bbox: block.bbox.clone(),
        content_role: infer_content_role(&block.section_path, &block.text),
        display_capability: display_capability(block),
    }
}

fn infer_content_role(section_path: &[String], text: &str) -> ContentRole {
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

fn display_capability(block: &SourceBlock) -> DisplayCapability {
    if block.page_no.is_some() || block.bbox.is_some() {
        DisplayCapability::PreciseAnchor
    } else if block.segment_type == ChunkSegmentType::Table {
        DisplayCapability::RowOnly
    } else {
        DisplayCapability::TextOnly
    }
}

fn append_search_part(value: Option<&str>, parts: &mut Vec<String>, seen: &mut HashSet<String>) {
    let normalized = value.map(normalize_search_line).unwrap_or_default();
    if normalized.is_empty() {
        return;
    }
    let dedup_key = normalized.to_ascii_lowercase();
    if seen.insert(dedup_key) {
        parts.push(normalized);
    }
}

fn clean_search_text(text: &str) -> String {
    text.lines()
        .filter(|line| !is_low_value_image_caption(line))
        .map(remove_latex_commands)
        .flat_map(|line| {
            line.split_whitespace()
                .map(|token| token.trim_matches(|character: char| character.is_ascii_punctuation()))
                .filter(|token| !token.is_empty())
                .filter(|token| !is_uuid_like(token))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_low_value_image_caption(line: &str) -> bool {
    let lower = line.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return true;
    }
    (lower.contains("image") || lower.contains("图片"))
        && (lower.contains("fallback")
            || lower.contains("placeholder")
            || lower.contains("占位")
            || lower == "[image]"
            || lower == "<image>")
}

fn remove_latex_commands(text: &str) -> String {
    let mut output = String::new();
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\\' {
            while matches!(chars.peek(), Some(next) if next.is_alphabetic()) {
                chars.next();
            }
            output.push(' ');
            continue;
        }
        if matches!(character, '{' | '}' | '[' | ']') {
            output.push(' ');
        } else {
            output.push(character);
        }
    }
    output
}

fn is_uuid_like(token: &str) -> bool {
    let token = token.trim();
    if token.len() != 36 {
        return false;
    }
    token
        .chars()
        .enumerate()
        .all(|(index, character)| match index {
            8 | 13 | 18 | 23 => character == '-',
            _ => character.is_ascii_hexdigit(),
        })
}

fn normalize_search_line(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_owned()
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current_ascii = String::new();
    for character in text.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            current_ascii.push(character);
            continue;
        }
        if !current_ascii.is_empty() {
            tokens.push(std::mem::take(&mut current_ascii));
        }
        if is_cjk_character(character) {
            tokens.push(character.to_string());
        }
    }
    if !current_ascii.is_empty() {
        tokens.push(current_ascii);
    }
    tokens
}

fn is_cjk_character(character: char) -> bool {
    matches!(
        character as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF
    )
}
