use novex_model::*;

#[test]
fn runtime_config_maps_user_env_to_masked_routes() {
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
        ("RIGHT_CODE_DRAW_API_KEY", "sk-fake-draw-secret-2064"),
        ("RIGHT_CODE_DRAW_BASE_URL", "https://www.right.codes/draw"),
    ];
    let config = ModelRuntimeConfig::from_env_map(|key| {
        env.iter()
            .find_map(|(env_key, value)| (*env_key == key).then(|| (*value).to_owned()))
    });

    let summary = config.summary();

    assert!(summary.missing_env.is_empty());
    assert_eq!(summary.routes.len(), 4);

    let llm = summary.route(ModelRuntimeTarget::Llm).unwrap();
    assert_eq!(llm.provider, ModelProviderType::DeepSeek);
    assert_eq!(llm.kind, ModelKind::Llm);
    assert_eq!(llm.model.as_deref(), Some("deepseek-v4-flash"));
    assert_eq!(llm.endpoint, "https://api.deepseek.com/chat/completions");
    assert_eq!(llm.masked_api_key, "sk-****508d");
    assert_eq!(
        llm.purposes,
        vec![
            ModelRoutePurpose::Chat,
            ModelRoutePurpose::RagAnswer,
            ModelRoutePurpose::EvalJudge,
            ModelRoutePurpose::CodeAgent,
            ModelRoutePurpose::GuardianReview,
        ]
    );

    let embedding = summary.route(ModelRuntimeTarget::Embedding).unwrap();
    assert_eq!(
        embedding.endpoint,
        "https://dashscope.aliyuncs.com/compatible-mode/v1/embeddings"
    );
    assert_eq!(embedding.masked_api_key, "sk-****ffff");

    let reranker = summary.route(ModelRuntimeTarget::Reranker).unwrap();
    assert_eq!(
        reranker.endpoint,
        "https://dashscope.aliyuncs.com/compatible-api/v1/reranks"
    );

    let draw = summary.route(ModelRuntimeTarget::Draw).unwrap();
    assert_eq!(draw.provider, ModelProviderType::RightCodeDraw);
    assert_eq!(draw.kind, ModelKind::MediaGeneration);
    assert_eq!(draw.model, None);
    assert_eq!(draw.endpoint, "https://www.right.codes/draw");

    let debug = format!("{config:?}");
    assert!(!debug.contains("sk-fake-llm-secret-508d"));
    assert!(debug.contains("sk-****508d"));
}

#[test]
fn runtime_config_reports_missing_env_without_creating_partial_routes() {
    let config = ModelRuntimeConfig::from_env_map(|key| {
        (key == "LLM_API_KEY").then(|| "sk-fake-llm-secret-508d".to_owned())
    });

    let summary = config.summary();

    assert!(summary.routes.is_empty());
    assert_eq!(
        summary.missing_env,
        vec![
            "LLM_BASE_URL",
            "LLM_MODEL",
            "EMBEDDING_API_KEY",
            "EMBEDDING_BASE_URL",
            "EMBEDDING_MODEL",
            "RERANKER_API_KEY",
            "RERANKER_BASE_URL",
            "RERANKER_MODEL",
            "RIGHT_CODE_DRAW_API_KEY",
            "RIGHT_CODE_DRAW_BASE_URL",
        ]
    );
}

#[test]
fn dynamic_route_constructor_preserves_registry_route_id() {
    let route = ModelRuntimeRoute::new(
        "tenant42.rag_answer",
        ModelRuntimeTarget::Llm,
        ModelKind::Llm,
        ModelProviderType::OpenAiCompatible,
        Some("qwen-private".to_owned()),
        "https://llm.internal/v1",
        "https://llm.internal/v1/chat/completions",
        "sk-fake-private-secret-0001",
        vec![ModelRoutePurpose::RagAnswer],
        vec!["LLM_PRIVATE_KEY".to_owned()],
    )
    .unwrap();

    let summary = route.summary();

    assert_eq!(summary.route_id, "tenant42.rag_answer");
    assert_eq!(summary.target, ModelRuntimeTarget::Llm);
    assert_eq!(summary.provider, ModelProviderType::OpenAiCompatible);
    assert_eq!(summary.model.as_deref(), Some("qwen-private"));
    assert_eq!(summary.masked_api_key, "sk-****0001");
    assert_eq!(summary.env_keys, vec!["LLM_PRIVATE_KEY"]);
    assert!(!format!("{route:?}").contains("sk-fake-private-secret-0001"));
}

#[test]
fn guardian_review_route_purpose_uses_default_llm_route() {
    let env = [
        ("LLM_API_KEY", "sk-fake-llm-secret-508d"),
        ("LLM_BASE_URL", "https://api.deepseek.com"),
        ("LLM_MODEL", "deepseek-v4-flash"),
    ];
    let config = ModelRuntimeConfig::from_env_map(|key| {
        env.iter()
            .find_map(|(env_key, value)| (*env_key == key).then(|| (*value).to_owned()))
    });
    let summary = config.summary();
    let llm = summary.route(ModelRuntimeTarget::Llm).unwrap();

    assert!(llm.purposes.contains(&ModelRoutePurpose::GuardianReview));
    assert_eq!(
        llm.purpose_route_ids
            .get("guardian_review")
            .map(String::as_str),
        Some("runtime.llm")
    );
}
