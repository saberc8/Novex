use crate::util::{json_f64_field, json_string_field, non_negative};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageCostInput {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub vector_count: i64,
}

pub fn estimate_model_cost_cents(cost_spec: &Value, input: &ModelUsageCostInput) -> f64 {
    let unit = json_string_field(cost_spec, &["unit"])
        .unwrap_or_default()
        .to_ascii_lowercase();
    let request_cost = non_negative(input.request_count)
        * json_f64_field(
            cost_spec,
            &["requestCents", "request_cents", "centsPerRequest"],
        )
        .unwrap_or_default();

    let cost = match unit.as_str() {
        "token" | "tokens" => token_cost_cents(cost_spec, input) + request_cost,
        "request" | "requests" => request_cost,
        "vector" | "vectors" => {
            non_negative(input.vector_count)
                * json_f64_field(
                    cost_spec,
                    &["vectorCents", "vector_cents", "centsPerVector"],
                )
                .unwrap_or_default()
        }
        _ => request_cost,
    };

    if cost.is_finite() {
        cost.max(0.0)
    } else {
        0.0
    }
}

fn token_cost_cents(cost_spec: &Value, input: &ModelUsageCostInput) -> f64 {
    let prompt_rate = json_f64_field(
        cost_spec,
        &[
            "promptCentsPer1kTokens",
            "promptTokenCentsPer1k",
            "inputCentsPer1kTokens",
            "inputTokenCentsPer1k",
        ],
    );
    let completion_rate = json_f64_field(
        cost_spec,
        &[
            "completionCentsPer1kTokens",
            "completionTokenCentsPer1k",
            "outputCentsPer1kTokens",
            "outputTokenCentsPer1k",
        ],
    );
    if prompt_rate.is_some() || completion_rate.is_some() {
        return non_negative(input.prompt_tokens) * prompt_rate.unwrap_or_default() / 1000.0
            + non_negative(input.completion_tokens) * completion_rate.unwrap_or_default() / 1000.0;
    }

    non_negative(input.total_tokens)
        * json_f64_field(
            cost_spec,
            &["totalCentsPer1kTokens", "totalTokenCentsPer1k"],
        )
        .unwrap_or_default()
        / 1000.0
}
