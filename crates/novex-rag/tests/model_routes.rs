use novex_rag::*;

#[test]
fn rag_model_routes_use_runtime_route_ids_when_available() {
    let env = [
        ("LLM_API_KEY", "sk-fake-llm-secret-508d"),
        ("LLM_BASE_URL", "https://api.deepseek.com"),
        ("LLM_MODEL", "deepseek-v4-flash"),
        ("EMBEDDING_API_KEY", "sk-fake-embedding-secret-ffff"),
        (
            "EMBEDDING_BASE_URL",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
        ),
        ("EMBEDDING_MODEL", "text-embedding-v4"),
        ("RERANKER_API_KEY", "sk-fake-reranker-secret-ffff"),
        (
            "RERANKER_BASE_URL",
            "https://dashscope.aliyuncs.com/compatible-api/v1",
        ),
        ("RERANKER_MODEL", "qwen3-rerank"),
    ];
    let config = novex_model::ModelRuntimeConfig::from_env_map(|key| {
        env.iter()
            .find_map(|(env_key, value)| (*env_key == key).then(|| (*value).to_owned()))
    });

    let routes = RagModelRoutes::from_runtime_config(&config);

    assert_eq!(routes.embedding_model_route, "runtime.embedding");
    assert_eq!(routes.rerank_model_route, "runtime.reranker");
    assert_eq!(routes.answer_model_route, "runtime.llm");
}

#[test]
fn rag_model_routes_fall_back_to_local_route_ids_when_runtime_missing() {
    let config = novex_model::ModelRuntimeConfig::from_env_map(|_| None);

    let routes = RagModelRoutes::from_runtime_config(&config);

    assert_eq!(routes.embedding_model_route, LOCAL_EMBEDDING_ROUTE);
    assert_eq!(routes.rerank_model_route, LOCAL_RERANK_ROUTE);
    assert_eq!(routes.answer_model_route, LOCAL_ANSWER_ROUTE);
}
