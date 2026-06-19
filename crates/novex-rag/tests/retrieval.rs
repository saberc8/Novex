use novex_rag::*;

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
fn keyword_retrieve_ranks_exact_chinese_phrase_over_noisy_overlap() {
    let chunks = vec![
        test_chunk(
            0,
            "AI 召集同学回看会议，比率口径准备中，确认概率指标在哪里提交到达，内容内容内容。",
        ),
        test_chunk(
            1,
            "不可能 N 角里明确提到 AI 召回率、准确率，要求在工作信息流场景里同时保证召回和答案质量。",
        ),
        test_chunk(
            2,
            "作者讨论 context 不平权，因为不同组织拥有的工作数据、权限、流程和协作语境不同。",
        ),
    ];

    let hits = keyword_retrieve("AI 召回率、准确率在文中哪里被提到？", &chunks, 3);

    assert_eq!(hits[0].chunk.chunk_index, 1);
}

#[test]
fn keyword_retrieve_ranks_date_fact_for_chinese_long_document_query() {
    let chunks = vec![
        test_chunk(
            0,
            "ONE 项目生意命令周报期限从前什么时间候选开会始终首席次数公司开放，信息很多但没有真实日期事实。",
        ),
        test_chunk(
            1,
            "ONE 生命周期：2025 年 4 月开始孕育，8 月 25 日发布会首次公开，DAU 巅峰稳定在 300 万左右。",
        ),
        test_chunk(
            2,
            "2025 年 AI 产品叙事从模型会聊天、总结、搜索，转向模型调工具、完成任务、进入真实工作流。",
        ),
    ];

    let hits = keyword_retrieve(
        "ONE 项目的生命周期从什么时候开始，什么时候首次公开？",
        &chunks,
        3,
    );

    assert_eq!(hits[0].chunk.chunk_index, 1);
}

#[test]
fn keyword_retrieve_normalizes_pdf_compatibility_cjk_forms() {
    let chunks = vec![
        test_chunk(0, "普通 PDF 背景，没有目标字形。"),
        test_chunk(1, "PDF 抽取出的兼容字形：⽣。"),
    ];

    let hits = keyword_retrieve("生", &chunks, 2);

    assert_eq!(hits[0].chunk.chunk_index, 1);
}

#[test]
fn keyword_retrieve_expands_numbered_label_ranges_in_cjk_queries() {
    let chunks = ["P0", "P1", "P2", "P3", "P4", "P5"]
        .into_iter()
        .enumerate()
        .map(|(index, label)| DocumentChunk {
            document_id: "doc-numbered".to_owned(),
            chunk_id: format!("doc-numbered:{index}"),
            chunk_index: index,
            text: format!("{label} 方案交付内容。"),
            semantic_search_text: format!(
                "Novex Architecture / 阶段计划 / {label}: 阶段方案\n{label} 方案交付内容。"
            ),
            token_count: 8,
            citation: CitationRef {
                document_id: "doc-numbered".to_owned(),
                chunk_id: format!("doc-numbered:{index}"),
                page_no: None,
                section_path: vec!["阶段计划".to_owned(), label.to_owned()],
            },
            metadata: ChunkMetadata::default(),
        })
        .chain(std::iter::once(DocumentChunk {
            document_id: "doc-numbered".to_owned(),
            chunk_id: "doc-numbered:generic".to_owned(),
            chunk_index: 99,
            text: "这是一个泛化方案片段，但不包含阶段编号。".to_owned(),
            semantic_search_text: "架构 / 泛化方案\n这是一个泛化方案片段，但不包含里程碑编号。"
                .to_owned(),
            token_count: 8,
            citation: CitationRef {
                document_id: "doc-numbered".to_owned(),
                chunk_id: "doc-numbered:generic".to_owned(),
                page_no: None,
                section_path: vec!["泛化方案".to_owned()],
            },
            metadata: ChunkMetadata::default(),
        }))
        .collect::<Vec<_>>();

    let hits = keyword_retrieve("按照这个方案，总结p0到p5的方案是否合理", &chunks, 6);
    let hit_ids = hits
        .iter()
        .map(|hit| hit.chunk.chunk_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        hit_ids,
        vec![
            "doc-numbered:0",
            "doc-numbered:1",
            "doc-numbered:2",
            "doc-numbered:3",
            "doc-numbered:4",
            "doc-numbered:5"
        ]
    );
}

#[test]
fn keyword_retrieve_expands_non_m_numbered_ranges() {
    let chunks = ["P1", "P2", "P3"]
        .into_iter()
        .enumerate()
        .map(|(index, phase)| DocumentChunk {
            document_id: "doc-phase".to_owned(),
            chunk_id: format!("doc-phase:{index}"),
            chunk_index: index,
            text: format!("{phase} 交付内容。"),
            semantic_search_text: format!(
                "Novex Architecture / 计划 / {phase}: 阶段方案\n{phase} 交付内容。"
            ),
            token_count: 8,
            citation: CitationRef {
                document_id: "doc-phase".to_owned(),
                chunk_id: format!("doc-phase:{index}"),
                page_no: None,
                section_path: vec!["计划".to_owned(), phase.to_owned()],
            },
            metadata: ChunkMetadata::default(),
        })
        .collect::<Vec<_>>();

    let hits = keyword_retrieve("总结p1到p3是否合理", &chunks, 3);
    let hit_ids = hits
        .iter()
        .map(|hit| hit.chunk.chunk_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(hit_ids, vec!["doc-phase:0", "doc-phase:1", "doc-phase:2"]);
}

fn test_chunk(index: usize, text: &str) -> DocumentChunk {
    DocumentChunk {
        document_id: "doc-chinese".to_owned(),
        chunk_id: format!("doc-chinese:{index}"),
        chunk_index: index,
        text: text.to_owned(),
        semantic_search_text: text.to_owned(),
        token_count: text.chars().count(),
        citation: CitationRef {
            document_id: "doc-chinese".to_owned(),
            chunk_id: format!("doc-chinese:{index}"),
            page_no: Some((index + 1) as i32),
            section_path: vec!["测试".to_owned()],
        },
        metadata: ChunkMetadata::default(),
    }
}
