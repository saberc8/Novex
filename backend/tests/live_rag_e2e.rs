use std::env;

use backend_rust::application::ai::knowledge_service::{
    DatasetCommand, DocumentUploadCommand, KnowledgeService, RagAskCommand,
};
use sqlx::{postgres::PgPoolOptions, Row};

#[tokio::test]
#[ignore = "requires backend/.env, Postgres, Milvus, embedding, rerank, and LLM providers"]
async fn live_rag_uses_embedding_milvus_rerank_and_llm() {
    if env::var("NOVEX_LIVE_RAG_TEST").ok().as_deref() != Some("1") {
        eprintln!("NOVEX_LIVE_RAG_TEST=1 not set; skipping live RAG smoke");
        return;
    }

    default_local_milvus_env_if_missing();
    require_env("DATABASE_URL");
    require_env("EMBEDDING_API_KEY");
    require_env("EMBEDDING_BASE_URL");
    require_env("EMBEDDING_MODEL");
    require_env("RERANKER_API_KEY");
    require_env("RERANKER_BASE_URL");
    require_env("RERANKER_MODEL");
    require_env("LLM_API_KEY");
    require_env("LLM_BASE_URL");
    require_env("LLM_MODEL");

    env::set_var("NOVEX_REQUIRE_LIVE_RAG", "1");
    env::set_var("NOVEX_REQUIRE_MILVUS", "1");

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL checked");
    let db = PgPoolOptions::new()
        .max_connections(3)
        .connect(&database_url)
        .await
        .expect("connect live RAG database");
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("run backend migrations");

    let service = KnowledgeService::new(db.clone());
    let unique = chrono::Utc::now().timestamp_millis();
    let tenant_id = unique;
    let user_id = unique + 10;
    let fact = format!(
        "Novex live RAG training starts at 09:30 on Monday in Room Orion. Evidence marker: LIVE-RAG-E2E-{unique}."
    );

    let dataset_id = service
        .create_dataset_for_tenant(
            tenant_id,
            user_id,
            DatasetCommand {
                name: format!("live-rag-e2e-{unique}"),
                description: "Live RAG E2E smoke fixture".to_owned(),
                ..DatasetCommand::default()
            },
        )
        .await
        .expect("create live RAG dataset");

    service
        .upload_text_document_for_tenant(
            tenant_id,
            user_id,
            dataset_id,
            DocumentUploadCommand {
                name: format!("live-rag-e2e-{unique}.txt"),
                content: fact,
                ..DocumentUploadCommand::default()
            },
        )
        .await
        .expect("upload and index live RAG document");

    let response = service
        .ask_dataset_for_tenant(
            tenant_id,
            user_id,
            dataset_id,
            RagAskCommand {
                question: "When and where does Novex live RAG training start?".to_owned(),
                limit: 5,
            },
        )
        .await
        .expect("ask live RAG dataset");

    assert_eq!(response.answer_strategy, "llm_grounded");
    assert!(
        !response.answer.trim().is_empty(),
        "LLM answer should be non-empty"
    );
    assert!(
        response.answer.contains("09")
            || response.answer.to_ascii_lowercase().contains("monday")
            || response.answer.to_ascii_lowercase().contains("orion"),
        "answer should mention the indexed fact, got: {}",
        response.answer
    );
    assert!(
        !response.citations.is_empty(),
        "live RAG answer should include citations"
    );
    assert!(
        response.retrieval_hit_count > 0,
        "live RAG answer should have retrieval hits"
    );

    let trace = sqlx::query(
        r#"
SELECT answer_strategy, embedding_model_route, rerank_model_route, answer_model_route,
       retrieval_hit_count
FROM ai_rag_trace
WHERE tenant_id = $1 AND id = $2;
"#,
    )
    .bind(tenant_id)
    .bind(response.trace_id)
    .fetch_one(&db)
    .await
    .expect("fetch persisted RAG trace");

    assert_eq!(
        trace.try_get::<String, _>("answer_strategy").unwrap(),
        "llm_grounded"
    );
    assert_eq!(
        trace
            .try_get::<Option<String>, _>("embedding_model_route")
            .unwrap()
            .as_deref(),
        Some("runtime.embedding")
    );
    assert_eq!(
        trace
            .try_get::<Option<String>, _>("rerank_model_route")
            .unwrap()
            .as_deref(),
        Some("runtime.reranker")
    );
    assert_eq!(
        trace
            .try_get::<Option<String>, _>("answer_model_route")
            .unwrap()
            .as_deref(),
        Some("runtime.llm")
    );
    assert!(
        trace
            .try_get::<i32, _>("retrieval_hit_count")
            .expect("retrieval hit count")
            > 0
    );
}

fn require_env(key: &str) {
    assert!(
        env::var(key)
            .ok()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        "{key} must be configured for NOVEX_LIVE_RAG_TEST=1"
    );
}

fn default_local_milvus_env_if_missing() {
    if env::var("MILVUS_ENDPOINT")
        .or_else(|_| env::var("NOVEX_MILVUS_ENDPOINT"))
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
    {
        return;
    }
    env::set_var("MILVUS_ENDPOINT", "http://localhost:19530");
    env::set_var("MILVUS_TOKEN", "root:Milvus");
}

