use novex_model::*;

#[test]
fn dynamic_route_parsers_accept_registry_values() {
    assert_eq!(
        ModelRoutePurpose::parse("rag_answer"),
        Some(ModelRoutePurpose::RagAnswer)
    );
    assert_eq!(
        ModelRoutePurpose::parse("guardian_review"),
        Some(ModelRoutePurpose::GuardianReview)
    );
    assert_eq!(
        ModelRoutePurpose::parse("rerank"),
        Some(ModelRoutePurpose::Rerank)
    );
    assert_eq!(ModelRoutePurpose::Chat.as_str(), "chat");
    assert_eq!(
        ModelRoutePurpose::GuardianReview.as_str(),
        "guardian_review"
    );
    assert_eq!(
        ModelKind::parse("media_generation"),
        Some(ModelKind::MediaGeneration)
    );
    assert_eq!(
        ModelProviderType::parse("openai-compatible"),
        Some(ModelProviderType::OpenAiCompatible)
    );
    assert_eq!(
        ModelProviderType::parse("deep-seek"),
        Some(ModelProviderType::DeepSeek)
    );
}
