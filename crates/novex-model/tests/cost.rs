use novex_model::*;

#[test]
fn model_usage_cost_estimate_applies_token_cost_spec() {
    let cost_spec = serde_json::json!({
        "unit": "token",
        "promptCentsPer1kTokens": 0.2,
        "completionCentsPer1kTokens": 0.8,
        "requestCents": 0.05
    });
    let input = ModelUsageCostInput {
        prompt_tokens: 1000,
        completion_tokens: 500,
        total_tokens: 1500,
        request_count: 1,
        vector_count: 0,
    };

    let cost_cents = estimate_model_cost_cents(&cost_spec, &input);

    assert!((cost_cents - 0.65).abs() < 0.000_001);
}
