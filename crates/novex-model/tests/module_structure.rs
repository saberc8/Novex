use std::fs;
use std::path::Path;

use novex_model::{
    estimate_model_cost_cents, estimate_model_text_tokens, evaluate_model_route_policy,
    mask_api_key, normalize_model_provider_usage, ModelEmbeddingVector, ModelKind,
    ModelMediaImageGenerationResp, ModelProviderStreamChunk, ModelProviderType, ModelRerankScore,
    ModelRoutePolicyInput, ModelRoutePurpose, ModelRuntimeConfig, ModelRuntimeRoute,
    ModelRuntimeTarget, ModelTokenUsage, ModelUsageCostInput,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_model_modules() {
    let lib = crate_file("src/lib.rs");

    for module in [
        "cost", "policy", "provider", "route", "taxonomy", "usage", "util",
    ] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum ModelKind",
        "pub struct ModelRuntimeConfig",
        "pub struct ModelTokenUsage",
        "pub fn normalize_model_provider_usage",
        "pub fn evaluate_model_route_policy",
        "pub fn mask_api_key",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn model_domain_modules_exist() {
    for module in [
        "src/cost.rs",
        "src/policy.rs",
        "src/provider.rs",
        "src/route.rs",
        "src/taxonomy.rs",
        "src/usage.rs",
        "src/util.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_taxonomy_and_route_contracts() {
    assert_eq!(
        ModelKind::parse("media_generation"),
        Some(ModelKind::MediaGeneration)
    );
    assert_eq!(
        ModelProviderType::parse("openai-compatible"),
        Some(ModelProviderType::OpenAiCompatible)
    );
    assert_eq!(
        ModelRoutePurpose::parse("guardian_review"),
        Some(ModelRoutePurpose::GuardianReview)
    );
    assert_eq!(
        ModelRuntimeTarget::parse("rerank"),
        Some(ModelRuntimeTarget::Reranker)
    );

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
    assert_eq!(summary.masked_api_key, "sk-****0001");
    assert_eq!(mask_api_key("sk-fake-private-secret-0001"), "sk-****0001");

    let config = ModelRuntimeConfig::from_env_map(|key| {
        (key == "LLM_API_KEY").then(|| "sk-fake-llm-secret-508d".to_owned())
    });
    assert!(config.routes().is_empty());
    assert!(config.missing_env().contains(&"LLM_BASE_URL".to_owned()));
}

#[test]
fn root_facade_preserves_usage_cost_policy_and_provider_contracts() {
    let usage = normalize_model_provider_usage(&serde_json::json!({
        "usage": {"input_tokens": "11", "outputTokens": 7}
    }));
    assert_eq!(usage.accounting_counts().total_tokens, 18);
    assert_eq!(estimate_model_text_tokens("hello world"), 2);
    assert_eq!(
        ModelTokenUsage::default().accounting_counts().total_tokens,
        0
    );

    let cost_cents = estimate_model_cost_cents(
        &serde_json::json!({
            "unit": "token",
            "promptCentsPer1kTokens": 0.2,
            "completionCentsPer1kTokens": 0.8,
            "requestCents": 0.05
        }),
        &ModelUsageCostInput {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
            request_count: 1,
            vector_count: 0,
        },
    );
    assert!((cost_cents - 0.65).abs() < 0.000_001);

    let status = evaluate_model_route_policy(ModelRoutePolicyInput {
        network_zone: "private",
        fallback_network_zone: Some("public"),
        fallback_policy: &serde_json::json!({"enabled": true}),
        route_policy: &serde_json::Value::Null,
    });
    assert_eq!(
        status.violations,
        vec!["cross_zone_fallback_not_allowed".to_owned()]
    );

    let stream = ModelProviderStreamChunk {
        index: 0,
        content: "delta".to_owned(),
        provider_event: Some("message.delta".to_owned()),
    };
    assert_eq!(stream.content, "delta");

    let media = ModelMediaImageGenerationResp {
        provider_payload: serde_json::json!({"id": "img-1"}),
        asset_url: "https://cdn.example.com/img.png".to_owned(),
        provider_asset_id: Some("img-1".to_owned()),
    };
    assert_eq!(media.provider_asset_id.as_deref(), Some("img-1"));

    assert_eq!(
        ModelRerankScore {
            index: 1,
            score: 0.9
        }
        .index,
        1
    );
    assert_eq!(
        ModelEmbeddingVector {
            index: 2,
            vector: vec![0.1, 0.2],
        }
        .vector
        .len(),
        2
    );
}
