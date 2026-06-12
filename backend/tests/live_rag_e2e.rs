use std::env;

use backend_rust::application::ai::knowledge_service::{
    DatasetCommand, DocumentUploadCommand, KnowledgeService, RagAskCommand,
};
use sqlx::{postgres::PgPoolOptions, Row};

#[tokio::test]
#[ignore = "requires infra/.env.poc, Postgres, Milvus, embedding, rerank, and LLM providers"]
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
    let test_database = LiveTestDatabase::create(&database_url).await;
    let db = test_database.pool().clone();
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("run backend migrations");

    let unique = chrono::Utc::now().timestamp_millis();
    let tenant_id = unique;
    let user_id = unique + 10;
    seed_live_dynamic_model_routes(&db, tenant_id, user_id, unique).await;
    let service = KnowledgeService::new(db.clone());
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
                ..RagAskCommand::default()
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
        Some("live.dynamic.embedding")
    );
    assert_eq!(
        trace
            .try_get::<Option<String>, _>("rerank_model_route")
            .unwrap()
            .as_deref(),
        Some("live.dynamic.rerank")
    );
    assert_eq!(
        trace
            .try_get::<Option<String>, _>("answer_model_route")
            .unwrap()
            .as_deref(),
        Some("live.dynamic.rag_answer")
    );
    assert!(
        trace
            .try_get::<i32, _>("retrieval_hit_count")
            .expect("retrieval hit count")
            > 0
    );

    test_database.drop_database().await;
}

async fn seed_live_dynamic_model_routes(
    db: &sqlx::PgPool,
    tenant_id: i64,
    user_id: i64,
    unique: i64,
) {
    let llm_base_url = env::var("LLM_BASE_URL").expect("LLM_BASE_URL checked");
    let llm_model = env::var("LLM_MODEL").expect("LLM_MODEL checked");
    let embedding_base_url = env::var("EMBEDDING_BASE_URL").expect("EMBEDDING_BASE_URL checked");
    let embedding_model = env::var("EMBEDDING_MODEL").expect("EMBEDDING_MODEL checked");
    let reranker_base_url = env::var("RERANKER_BASE_URL").expect("RERANKER_BASE_URL checked");
    let reranker_model = env::var("RERANKER_MODEL").expect("RERANKER_MODEL checked");

    seed_live_model_route(
        db,
        LiveModelRouteSeed {
            tenant_id,
            user_id,
            id_base: unique * 100 + 1,
            provider_code: "live-dynamic-llm-provider",
            provider_name: "Live Dynamic LLM Provider",
            provider_type: "deep-seek",
            deployment_code: "live-dynamic-llm-deployment",
            deployment_name: "Live Dynamic LLM Deployment",
            endpoint: &llm_base_url,
            api_path: Some("/chat/completions"),
            profile_code: "live-dynamic-llm-profile",
            profile_name: "Live Dynamic LLM Profile",
            model_name: &llm_model,
            model_kind: "llm",
            credential_code: "live-dynamic-llm-credential",
            credential_ref: "env:LLM_API_KEY",
            route_code: "live.dynamic.rag_answer",
            route_purpose: "rag_answer",
        },
    )
    .await;

    seed_live_model_route(
        db,
        LiveModelRouteSeed {
            tenant_id,
            user_id,
            id_base: unique * 100 + 11,
            provider_code: "live-dynamic-embedding-provider",
            provider_name: "Live Dynamic Embedding Provider",
            provider_type: "dash-scope",
            deployment_code: "live-dynamic-embedding-deployment",
            deployment_name: "Live Dynamic Embedding Deployment",
            endpoint: &embedding_base_url,
            api_path: Some("/embeddings"),
            profile_code: "live-dynamic-embedding-profile",
            profile_name: "Live Dynamic Embedding Profile",
            model_name: &embedding_model,
            model_kind: "embedding",
            credential_code: "live-dynamic-embedding-credential",
            credential_ref: "env:EMBEDDING_API_KEY",
            route_code: "live.dynamic.embedding",
            route_purpose: "embedding",
        },
    )
    .await;

    seed_live_model_route(
        db,
        LiveModelRouteSeed {
            tenant_id,
            user_id,
            id_base: unique * 100 + 21,
            provider_code: "live-dynamic-rerank-provider",
            provider_name: "Live Dynamic Rerank Provider",
            provider_type: "dash-scope",
            deployment_code: "live-dynamic-rerank-deployment",
            deployment_name: "Live Dynamic Rerank Deployment",
            endpoint: &reranker_base_url,
            api_path: Some("/reranks"),
            profile_code: "live-dynamic-rerank-profile",
            profile_name: "Live Dynamic Rerank Profile",
            model_name: &reranker_model,
            model_kind: "rerank",
            credential_code: "live-dynamic-rerank-credential",
            credential_ref: "env:RERANKER_API_KEY",
            route_code: "live.dynamic.rerank",
            route_purpose: "rerank",
        },
    )
    .await;
}

struct LiveModelRouteSeed<'a> {
    tenant_id: i64,
    user_id: i64,
    id_base: i64,
    provider_code: &'a str,
    provider_name: &'a str,
    provider_type: &'a str,
    deployment_code: &'a str,
    deployment_name: &'a str,
    endpoint: &'a str,
    api_path: Option<&'a str>,
    profile_code: &'a str,
    profile_name: &'a str,
    model_name: &'a str,
    model_kind: &'a str,
    credential_code: &'a str,
    credential_ref: &'a str,
    route_code: &'a str,
    route_purpose: &'a str,
}

async fn seed_live_model_route(db: &sqlx::PgPool, seed: LiveModelRouteSeed<'_>) {
    let provider_id = seed.id_base;
    let deployment_id = seed.id_base + 1;
    let profile_id = seed.id_base + 2;
    let credential_id = seed.id_base + 3;
    let route_id = seed.id_base + 4;

    sqlx::query(
        r#"
INSERT INTO ai_model_provider
    (id, tenant_id, code, name, provider_type, protocol, status, metadata, create_user, create_time)
VALUES
    ($1, $2, $3, $4, $5, 'openai-compatible', 1, '{"source":"live-test"}'::jsonb, $6, NOW());
"#,
    )
    .bind(provider_id)
    .bind(seed.tenant_id)
    .bind(seed.provider_code)
    .bind(seed.provider_name)
    .bind(seed.provider_type)
    .bind(seed.user_id)
    .execute(db)
    .await
    .expect("seed live model provider");

    sqlx::query(
        r#"
INSERT INTO ai_model_deployment
    (id, tenant_id, provider_id, code, name, endpoint, api_path, network_zone, timeout_ms, status, metadata, create_user, create_time)
VALUES
    ($1, $2, $3, $4, $5, $6, $7, 'public', 30000, 1, '{"source":"live-test"}'::jsonb, $8, NOW());
"#,
    )
    .bind(deployment_id)
    .bind(seed.tenant_id)
    .bind(provider_id)
    .bind(seed.deployment_code)
    .bind(seed.deployment_name)
    .bind(seed.endpoint)
    .bind(seed.api_path)
    .bind(seed.user_id)
    .execute(db)
    .await
    .expect("seed live model deployment");

    sqlx::query(
        r#"
INSERT INTO ai_model_profile
    (id, tenant_id, deployment_id, code, name, model_name, model_kind, capabilities, limits, embedding_spec, rerank_spec, cost_spec, fallback_policy, status, create_user, create_time)
VALUES
    ($1, $2, $3, $4, $5, $6, $7, '{}'::jsonb, '{"timeoutMs":30000}'::jsonb, '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, 1, $8, NOW());
"#,
    )
    .bind(profile_id)
    .bind(seed.tenant_id)
    .bind(deployment_id)
    .bind(seed.profile_code)
    .bind(seed.profile_name)
    .bind(seed.model_name)
    .bind(seed.model_kind)
    .bind(seed.user_id)
    .execute(db)
    .await
    .expect("seed live model profile");

    sqlx::query(
        r#"
INSERT INTO ai_model_credential
    (id, tenant_id, provider_id, deployment_id, code, scope_type, scope_id, credential_ref, masked_value, status, metadata, create_user, create_time)
VALUES
    ($1, $2, $3, $4, $5, 'platform', 'live-test', $6, $6, 1, '{"source":"live-test"}'::jsonb, $7, NOW());
"#,
    )
    .bind(credential_id)
    .bind(seed.tenant_id)
    .bind(provider_id)
    .bind(deployment_id)
    .bind(seed.credential_code)
    .bind(seed.credential_ref)
    .bind(seed.user_id)
    .execute(db)
    .await
    .expect("seed live model credential");

    sqlx::query(
        r#"
INSERT INTO ai_model_route
    (id, tenant_id, code, route_purpose, model_profile_id, credential_id, priority, status, policy, create_user, create_time)
VALUES
    ($1, $2, $3, $4, $5, $6, 1, 1, '{"source":"live-test","dynamic":true}'::jsonb, $7, NOW());
"#,
    )
    .bind(route_id)
    .bind(seed.tenant_id)
    .bind(seed.route_code)
    .bind(seed.route_purpose)
    .bind(profile_id)
    .bind(credential_id)
    .bind(seed.user_id)
    .execute(db)
    .await
    .expect("seed live model route");
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
    env::set_var("MILVUS_ENDPOINT", "http://localhost:19540");
    env::set_var("MILVUS_TOKEN", "root:Milvus");
}

struct LiveTestDatabase {
    admin_url: String,
    name: String,
    pool: sqlx::PgPool,
}

impl LiveTestDatabase {
    async fn create(admin_url: &str) -> Self {
        let name = format!("novex_live_rag_{}", chrono::Utc::now().timestamp_millis());
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(admin_url)
            .await
            .expect("connect admin database");
        sqlx::query(&format!(r#"CREATE DATABASE "{}";"#, name))
            .execute(&admin)
            .await
            .expect("create live RAG test database");
        admin.close().await;

        let database_url = database_url_with_database(admin_url, &name);
        let pool = PgPoolOptions::new()
            .max_connections(3)
            .connect(&database_url)
            .await
            .expect("connect live RAG test database");

        Self {
            admin_url: admin_url.to_owned(),
            name,
            pool,
        }
    }

    fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }

    async fn drop_database(self) {
        self.pool.close().await;
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&self.admin_url)
            .await
            .expect("connect admin database for cleanup");
        sqlx::query(&format!(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}';",
            self.name
        ))
        .execute(&admin)
        .await
        .ok();
        sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{}";"#, self.name))
            .execute(&admin)
            .await
            .expect("drop live RAG test database");
        admin.close().await;
    }
}

fn database_url_with_database(database_url: &str, database: &str) -> String {
    let mut url = url::Url::parse(database_url).expect("DATABASE_URL must be parseable");
    url.set_path(database);
    url.to_string()
}
