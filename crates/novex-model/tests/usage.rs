use novex_model::*;

#[test]
fn model_usage_normalizes_provider_token_aliases_and_estimates_text_tokens() {
    let body = serde_json::json!({
        "usage": {
            "input_tokens": "11",
            "outputTokens": 7
        }
    });

    let usage = normalize_model_provider_usage(&body);

    assert_eq!(usage.prompt_tokens, Some(11));
    assert_eq!(usage.completion_tokens, Some(7));
    assert_eq!(usage.total_tokens, Some(18));
    assert_eq!(usage.accounting_counts().total_tokens, 18);
    assert_eq!(estimate_model_text_tokens("hello world"), 2);
    assert_eq!(estimate_model_text_tokens("你好"), 2);
}
