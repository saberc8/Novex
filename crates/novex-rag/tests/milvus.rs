use novex_rag::*;

#[test]
fn milvus_search_request_builds_safe_scalar_filter_and_rest_body() {
    let request = MilvusSearchRequest::new("novex_t42_dataset_7", vec![0.25, -0.5], 3, 42, 7)
        .with_document_ids(vec![21, 21, 22])
        .with_output_fields(vec!["chunk_uid", "chunk_db_id", "document_id"]);

    assert_eq!(
        request.filter_expression(),
        "tenant_id == 42 and dataset_id == 7 and document_id in [21, 22]"
    );

    let body = request.to_rest_search_body();
    assert_eq!(body["collectionName"], "novex_t42_dataset_7");
    assert_eq!(body["data"], serde_json::json!([[0.25, -0.5]]));
    assert_eq!(body["annsField"], "embedding");
    assert_eq!(body["filter"], request.filter_expression());
    assert_eq!(body["limit"], 3);
    assert_eq!(
        body["outputFields"],
        serde_json::json!(["chunk_uid", "chunk_db_id", "document_id"])
    );
    assert_eq!(body["searchParams"]["metric_type"], "COSINE");
}

#[test]
fn parse_milvus_search_hits_maps_output_fields_and_ignores_malformed_rows() {
    let response = serde_json::json!({
        "code": 0,
        "data": [
            {
                "id": 101,
                "distance": 0.91,
                "chunk_uid": "doc-a:0",
                "chunk_db_id": 9001,
                "document_id": 77
            },
            {
                "id": 102,
                "distance": 0.88,
                "entity": {
                    "chunkUid": "doc-a:1",
                    "chunkDbId": 9002,
                    "documentId": 77
                }
            },
            {
                "id": 103,
                "distance": 0.1
            }
        ]
    });

    let hits = parse_milvus_search_hits(&response);

    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].chunk_uid, "doc-a:0");
    assert_eq!(hits[0].chunk_db_id, Some(9001));
    assert_eq!(hits[0].document_id, Some(77));
    assert!((hits[0].score - 0.91).abs() < f32::EPSILON);
    assert_eq!(hits[1].chunk_uid, "doc-a:1");
    assert_eq!(hits[1].chunk_db_id, Some(9002));
}

#[test]
fn milvus_upsert_request_builds_entities_with_scalar_metadata_and_embedding() {
    let request = MilvusUpsertRequest::new(
        "novex_t42_dataset_7",
        vec![MilvusUpsertRow {
            id: 9001,
            tenant_id: 42,
            dataset_id: 7,
            document_id: 77,
            chunk_uid: "doc-a:0".to_owned(),
            chunk_index: 0,
            embedding: vec![0.25, -0.5],
            semantic_search_text: "onboarding training".to_owned(),
            segment_type: "text".to_owned(),
            content_role: "canonical".to_owned(),
        }],
    );

    let body = request.to_rest_upsert_body();

    assert_eq!(body["collectionName"], "novex_t42_dataset_7");
    assert_eq!(body["data"][0]["id"], 9001);
    assert_eq!(body["data"][0]["tenant_id"], 42);
    assert_eq!(body["data"][0]["dataset_id"], 7);
    assert_eq!(body["data"][0]["document_id"], 77);
    assert_eq!(body["data"][0]["chunk_uid"], "doc-a:0");
    assert_eq!(
        body["data"][0]["embedding"],
        serde_json::json!([0.25, -0.5])
    );
    assert_eq!(
        body["data"][0]["semantic_search_text"],
        "onboarding training"
    );
}

#[test]
fn milvus_create_collection_request_declares_rag_schema_and_index() {
    let request =
        MilvusCreateCollectionRequest::new("novex_t42_dataset_7", 3, MilvusMetricType::Cosine);

    let body = request.to_rest_create_body();

    assert_eq!(body["collectionName"], "novex_t42_dataset_7");
    assert_eq!(body["schema"]["autoID"], false);
    assert_eq!(body["schema"]["enableDynamicField"], false);
    assert!(body["schema"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["fieldName"] == "id" && field["isPrimary"] == true));
    assert!(body["schema"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["fieldName"] == "tenant_id" && field["dataType"] == "Int64"));
    assert!(body["schema"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["fieldName"] == "embedding"
            && field["dataType"] == "FloatVector"
            && field["elementTypeParams"]["dim"] == 3));
    assert_eq!(body["indexParams"][0]["fieldName"], "embedding");
    assert_eq!(body["indexParams"][0]["metricType"], "COSINE");
}
