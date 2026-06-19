use std::fs;
use std::path::Path;

use novex_rag::{
    build_extractive_answer, chunk_document, keyword_retrieve, parse_document_content,
    parse_milvus_search_hits, BoundingBox, ChunkMetadata, ChunkSegmentType, DisplayCapability,
    MilvusCreateCollectionRequest, MilvusMetricType, MilvusSearchRequest, MilvusUpsertRequest,
    MilvusUpsertRow, RagModelRoutes, ResourceVisibility, LOCAL_ANSWER_ROUTE, LOCAL_EMBEDDING_ROUTE,
    LOCAL_RERANK_ROUTE,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_rag_modules() {
    let lib = crate_file("src/lib.rs");

    for module in [
        "answer",
        "chunk",
        "knowledge",
        "milvus",
        "model_routes",
        "parse",
        "retrieval",
    ] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub struct DocumentChunk",
        "pub struct MilvusSearchRequest",
        "pub fn parse_document_content",
        "pub fn chunk_document",
        "pub fn keyword_retrieve",
        "pub fn build_extractive_answer",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn rag_domain_modules_exist() {
    for module in [
        "src/answer.rs",
        "src/chunk.rs",
        "src/knowledge.rs",
        "src/milvus.rs",
        "src/model_routes.rs",
        "src/parse.rs",
        "src/retrieval.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_public_rag_contracts() {
    assert_eq!(ResourceVisibility::default(), ResourceVisibility::Private);
    assert_eq!(
        RagModelRoutes::local().embedding_model_route,
        LOCAL_EMBEDDING_ROUTE
    );
    assert_eq!(
        RagModelRoutes::local().rerank_model_route,
        LOCAL_RERANK_ROUTE
    );
    assert_eq!(
        RagModelRoutes::local().answer_model_route,
        LOCAL_ANSWER_ROUTE
    );

    let parsed = parse_document_content(
        "doc-facade",
        "architecture.md",
        "text/markdown",
        "# Retrieval\n[[page: 2]]\nHybrid retrieval keeps citations anchored.",
    );
    let chunks = chunk_document(&parsed, 80, 0);
    let hits = keyword_retrieve("retrieval citations", &chunks, 3);
    let answer = build_extractive_answer("What keeps citations anchored?", &hits);

    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].metadata.segment_type, ChunkSegmentType::Text);
    assert_eq!(
        chunks[0].metadata.display_capability,
        DisplayCapability::PreciseAnchor
    );
    assert_eq!(chunks[0].metadata.page_no, Some(2));
    assert!(answer.answer.contains("Hybrid retrieval"));
    assert_eq!(answer.trace.retrieval_hit_count, 1);
}

#[test]
fn root_facade_preserves_public_milvus_contracts() {
    let search = MilvusSearchRequest::new("novex_t1_dataset_2", vec![0.1, 0.2], 5, 1, 2)
        .with_document_ids(vec![9, 7, 9])
        .with_metric_type(MilvusMetricType::Cosine);
    assert_eq!(
        search.filter_expression(),
        "tenant_id == 1 and dataset_id == 2 and document_id in [7, 9]"
    );
    assert_eq!(
        search.to_rest_search_body()["searchParams"]["metric_type"],
        "COSINE"
    );

    let create =
        MilvusCreateCollectionRequest::new("novex_t1_dataset_2", 3, MilvusMetricType::Cosine);
    assert_eq!(
        create.to_rest_create_body()["schema"]["fields"][0]["fieldName"],
        "id"
    );

    let upsert = MilvusUpsertRequest::new(
        "novex_t1_dataset_2",
        vec![MilvusUpsertRow {
            id: 10,
            tenant_id: 1,
            dataset_id: 2,
            document_id: 7,
            chunk_uid: "doc:0".to_owned(),
            chunk_index: 0,
            embedding: vec![0.1, 0.2, 0.3],
            semantic_search_text: "retrieval citation".to_owned(),
            segment_type: "text".to_owned(),
            content_role: "canonical".to_owned(),
        }],
    );
    assert_eq!(
        upsert.to_rest_upsert_body()["data"][0]["chunk_uid"],
        "doc:0"
    );

    let hits = parse_milvus_search_hits(&serde_json::json!({
        "data": [{"distance": 0.5, "chunk_uid": "doc:0", "chunk_db_id": 10}]
    }));
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].chunk_uid, "doc:0");
    assert_eq!(hits[0].chunk_db_id, Some(10));
}

#[test]
fn root_facade_preserves_anchor_metadata_types() {
    let metadata = ChunkMetadata {
        segment_type: ChunkSegmentType::Image,
        bbox: Some(BoundingBox {
            x: 1,
            y: 2,
            width: 30,
            height: 40,
        }),
        ..ChunkMetadata::default()
    };

    assert_eq!(metadata.segment_type.as_str(), "image");
    assert_eq!(metadata.display_capability.as_str(), "text_only");
    assert_eq!(metadata.bbox.as_ref().map(|bbox| bbox.width), Some(30));
}
