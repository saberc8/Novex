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
    parse_document_content(document_id, "", "text/plain", text)
}

pub fn parse_document_content(
    document_id: impl Into<String>,
    source_file_name: &str,
    content_type: &str,
    text: &str,
) -> ParsedDocument {
    let text = text.trim().replace("\r\n", "\n");
    let content_type = normalize_content_type(content_type);
    let source_file_name = non_empty_string(source_file_name);
    let line_count = text.lines().filter(|line| !line.trim().is_empty()).count();
    let blocks = if is_table_document(source_file_name.as_deref(), &content_type) {
        parse_table_blocks(&text)
    } else {
        parse_text_blocks(&text)
    };

    ParsedDocument {
        document_id: document_id.into(),
        text,
        content_type,
        source_title: None,
        source_file_name,
        blocks,
        line_count,
    }
}

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

fn normalize_content_type(content_type: &str) -> String {
    let content_type = content_type.trim();
    if content_type.is_empty() {
        "text/plain".to_owned()
    } else {
        content_type.to_ascii_lowercase()
    }
}

fn non_empty_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

fn is_table_document(source_file_name: Option<&str>, content_type: &str) -> bool {
    let content_type = content_type.to_ascii_lowercase();
    if content_type.contains("csv")
        || content_type.contains("spreadsheet")
        || content_type.contains("excel")
        || content_type.contains("tab-separated-values")
    {
        return true;
    }

    source_file_name
        .map(|name| {
            let name = name.to_ascii_lowercase();
            name.ends_with(".csv")
                || name.ends_with(".tsv")
                || name.ends_with(".xls")
                || name.ends_with(".xlsx")
        })
        .unwrap_or(false)
}

fn parse_table_blocks(text: &str) -> Vec<SourceBlock> {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return vec![];
    }

    let header = lines[0];
    let table_header = split_table_cells(header);
    vec![SourceBlock::table(lines.join("\n"), table_header)]
}

fn split_table_cells(line: &str) -> Vec<String> {
    let delimiter = if line.contains('\t') {
        '\t'
    } else if line.contains('|') && !line.contains(',') {
        '|'
    } else {
        ','
    };

    line.split(delimiter)
        .map(|cell| cell.trim().trim_matches('|').to_owned())
        .filter(|cell| !cell.is_empty())
        .collect()
}

fn parse_text_blocks(text: &str) -> Vec<SourceBlock> {
    let mut blocks = Vec::new();
    let mut paragraph = Vec::new();
    let mut section_path = Vec::new();
    let mut page_no = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            push_text_paragraph(&mut blocks, &mut paragraph, page_no, &section_path);
            continue;
        }

        if let Some((level, title)) = markdown_heading(trimmed) {
            push_text_paragraph(&mut blocks, &mut paragraph, page_no, &section_path);
            let target_len = level.saturating_sub(1);
            section_path.truncate(target_len);
            section_path.push(title);
            continue;
        }

        if let Some(page) = page_marker(trimmed) {
            push_text_paragraph(&mut blocks, &mut paragraph, page_no, &section_path);
            page_no = Some(page);
            continue;
        }

        if let Some(block) = image_marker(trimmed, page_no, &section_path) {
            push_text_paragraph(&mut blocks, &mut paragraph, page_no, &section_path);
            blocks.push(block);
            continue;
        }

        paragraph.push(trimmed.to_owned());
    }

    push_text_paragraph(&mut blocks, &mut paragraph, page_no, &section_path);
    if blocks.is_empty() && !text.trim().is_empty() {
        blocks.push(SourceBlock::text(
            text.trim().to_owned(),
            page_no,
            section_path,
        ));
    }
    blocks
}

fn push_text_paragraph(
    blocks: &mut Vec<SourceBlock>,
    paragraph: &mut Vec<String>,
    page_no: Option<i32>,
    section_path: &[String],
) {
    if paragraph.is_empty() {
        return;
    }
    let text = paragraph.join("\n").trim().to_owned();
    paragraph.clear();
    if text.is_empty() {
        return;
    }
    blocks.push(SourceBlock::text(text, page_no, section_path.to_vec()));
}

fn markdown_heading(line: &str) -> Option<(usize, String)> {
    let level = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if level == 0 || level > 6 {
        return None;
    }
    let title = line.get(level..)?.trim();
    if title.is_empty() {
        return None;
    }
    Some((level, title.to_owned()))
}

fn page_marker(line: &str) -> Option<i32> {
    if line.chars().count() > 48 {
        return None;
    }
    let lower = line.to_ascii_lowercase();
    if !lower.contains("page") && !line.contains('页') {
        return None;
    }
    let digits = line
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<i32>().ok().filter(|page| *page > 0)
}

fn image_marker(
    line: &str,
    current_page_no: Option<i32>,
    section_path: &[String],
) -> Option<SourceBlock> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("[[image:") || !trimmed.ends_with("]]") {
        return None;
    }

    let payload = trimmed.get("[[image:".len()..trimmed.len().saturating_sub(2))?;
    let image_key = marker_field(payload, &["key", "image_key", "access_key"]);
    let caption = marker_tail_field(payload, "caption")
        .or_else(|| marker_tail_field(payload, "alt"))
        .or_else(|| image_key.clone())
        .unwrap_or_else(|| "Image evidence".to_owned());
    let page_no = marker_field(payload, &["page", "page_no"])
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|page| *page > 0)
        .or(current_page_no);
    let bbox = marker_field(payload, &["bbox", "coordinates"]).and_then(|value| parse_bbox(&value));
    let image_access_keys = image_key.into_iter().collect::<Vec<_>>();

    Some(SourceBlock::image(
        caption,
        page_no,
        section_path.to_vec(),
        image_access_keys,
        bbox,
    ))
}

fn marker_field(payload: &str, names: &[&str]) -> Option<String> {
    payload.split_whitespace().find_map(|token| {
        let (name, value) = token.split_once('=')?;
        if names
            .iter()
            .any(|expected| name.eq_ignore_ascii_case(expected))
        {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if value.is_empty() {
                None
            } else {
                Some(value.to_owned())
            }
        } else {
            None
        }
    })
}

fn marker_tail_field(payload: &str, name: &str) -> Option<String> {
    let key = format!("{name}=");
    let start = payload.to_ascii_lowercase().find(&key)?;
    let value = payload.get(start + key.len()..)?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.trim_matches('"').trim_matches('\'').to_owned())
    }
}

fn parse_bbox(value: &str) -> Option<BoundingBox> {
    let numbers = value
        .split(',')
        .filter_map(|part| part.trim().parse::<i32>().ok())
        .collect::<Vec<_>>();
    if numbers.len() != 4 {
        return None;
    }
    Some(BoundingBox {
        x: numbers[0],
        y: numbers[1],
        width: numbers[2],
        height: numbers[3],
    })
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
            let search_text = if chunk.semantic_search_text.trim().is_empty() {
                &chunk.text
            } else {
                &chunk.semantic_search_text
            };
            let chunk_tokens = tokenize(search_text).into_iter().collect::<HashSet<_>>();
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

    #[test]
    fn chunk_document_preserves_table_headers_for_csv() {
        let parsed = parse_document_content(
            "doc-table",
            "training.csv",
            "text/csv",
            "employee,deadline,status\nAlice,Friday,done\nBob,Monday,pending",
        );

        let chunks = chunk_document(&parsed, 64, 0);

        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(|chunk| {
            chunk.metadata.segment_type == ChunkSegmentType::Table
                && chunk.metadata.table_header == vec!["employee", "deadline", "status"]
        }));
        assert!(chunks[0].text.contains("employee,deadline,status"));
        assert!(chunks[0].semantic_search_text.contains("training.csv"));
        assert!(chunks[0].semantic_search_text.contains("deadline"));
    }

    #[test]
    fn chunk_document_prefers_sentence_boundaries_for_text_blocks() {
        let parsed = parse_document_content(
            "doc-sentences",
            "policy.txt",
            "text/plain",
            "Training starts on Monday. Mentors review progress every Friday. Expenses are approved by finance.",
        );

        let chunks = chunk_document(&parsed, 48, 0);

        assert!(chunks.len() >= 2);
        assert_eq!(chunks[0].text, "Training starts on Monday.");
        assert_eq!(chunks[1].text, "Mentors review progress every Friday.");
        assert!(chunks.iter().all(|chunk| !chunk.text.ends_with("Frid")));
    }

    #[test]
    fn chunk_document_keeps_table_header_when_large_row_is_split() {
        let parsed = parse_document_content(
            "doc-large-table",
            "faq.csv",
            "text/csv",
            "question,answer,status\nHow to onboard,Complete security training before meeting the mentor and filing the first progress report,active",
        );

        let chunks = chunk_document(&parsed, 64, 0);

        assert!(chunks.len() > 1);
        assert!(chunks
            .iter()
            .all(|chunk| chunk.text.starts_with("question,answer,status\n")));
        assert!(chunks
            .iter()
            .all(|chunk| chunk.metadata.table_header == vec!["question", "answer", "status"]));
        assert!(chunks.iter().all(|chunk| chunk
            .semantic_search_text
            .contains("question answer status")));
    }

    #[test]
    fn parse_document_content_extracts_image_marker_anchor_metadata() {
        let parsed = parse_document_content(
            "doc-image",
            "architecture.md",
            "text/markdown",
            "# 检索链路\n[[page: 2]]\n[[image: key=img/search-flow.png bbox=10,20,300,180 caption=系统架构图显示 hybrid recall 和 rerank 链路]]",
        );

        let chunks = chunk_document(&parsed, 200, 0);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].metadata.segment_type, ChunkSegmentType::Image);
        assert_eq!(chunks[0].metadata.page_no, Some(2));
        assert_eq!(
            chunks[0].metadata.image_access_keys,
            vec!["img/search-flow.png"]
        );
        assert_eq!(
            chunks[0].metadata.bbox,
            Some(BoundingBox {
                x: 10,
                y: 20,
                width: 300,
                height: 180,
            })
        );
        assert!(chunks[0].semantic_search_text.contains("系统架构图"));
        assert!(chunks[0].semantic_search_text.contains("检索链路"));
    }

    #[test]
    fn semantic_search_text_filters_latex_uuid_and_image_placeholder() {
        let metadata = ChunkMetadata {
            source_title: Some("Onboarding benefits".to_owned()),
            source_file_name: Some("benefits.pdf".to_owned()),
            segment_type: ChunkSegmentType::Image,
            ..ChunkMetadata::default()
        };

        let semantic_text = build_semantic_search_text(
            "[image fallback caption]\n\\frac{x}{y} \\succ 550e8400-e29b-41d4-a716-446655440000",
            &metadata,
        );

        assert!(semantic_text.contains("Onboarding benefits"));
        assert!(semantic_text.contains("benefits.pdf"));
        assert!(!semantic_text.contains("\\frac"));
        assert!(!semantic_text.contains("\\succ"));
        assert!(!semantic_text.contains("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!semantic_text.to_ascii_lowercase().contains("fallback"));
    }

    #[test]
    fn chunk_document_keeps_markdown_section_and_page_anchor() {
        let parsed = parse_document_content(
            "doc-md",
            "handbook.md",
            "text/markdown",
            "# 入职培训\n[[page: 3]]\n第一天需要完成安全培训和导师见面。",
        );

        let chunks = chunk_document(&parsed, 200, 0);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].citation.page_no, Some(3));
        assert_eq!(chunks[0].citation.section_path, vec!["入职培训"]);
        assert_eq!(chunks[0].metadata.page_no, Some(3));
        assert_eq!(chunks[0].metadata.section_path, vec!["入职培训"]);
        assert_eq!(
            chunks[0].metadata.display_capability,
            DisplayCapability::PreciseAnchor
        );
    }
}
