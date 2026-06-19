use crate::knowledge::{BoundingBox, ParsedDocument, SourceBlock};

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
