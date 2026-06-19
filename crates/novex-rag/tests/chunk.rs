use novex_rag::*;

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
