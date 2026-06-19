use crate::util::json_i64_field;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTokenUsage {
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
}

impl ModelTokenUsage {
    pub fn accounting_counts(&self) -> ModelTokenUsageCounts {
        let prompt_tokens = self.prompt_tokens.unwrap_or_default().max(0);
        let completion_tokens = self.completion_tokens.unwrap_or_default().max(0);
        let derived_total = prompt_tokens.saturating_add(completion_tokens);
        let total_tokens = self
            .total_tokens
            .unwrap_or(derived_total)
            .max(0)
            .max(derived_total);

        ModelTokenUsageCounts {
            prompt_tokens,
            completion_tokens,
            total_tokens,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTokenUsageCounts {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

pub fn normalize_model_provider_usage(body: &Value) -> ModelTokenUsage {
    let usage = body.get("usage").unwrap_or(body);
    let prompt_tokens = json_i64_field(
        usage,
        &[
            "prompt_tokens",
            "promptTokens",
            "input_tokens",
            "inputTokens",
        ],
    );
    let completion_tokens = json_i64_field(
        usage,
        &[
            "completion_tokens",
            "completionTokens",
            "output_tokens",
            "outputTokens",
        ],
    );
    let total_tokens = json_i64_field(usage, &["total_tokens", "totalTokens"]).or_else(|| {
        match (prompt_tokens, completion_tokens) {
            (Some(prompt_tokens), Some(completion_tokens)) => {
                Some(prompt_tokens.saturating_add(completion_tokens))
            }
            _ => None,
        }
    });

    ModelTokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
    }
}

pub fn estimate_model_text_tokens(text: &str) -> i32 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }

    let whitespace_count = trimmed.split_whitespace().count();
    if whitespace_count > 1 {
        whitespace_count.min(i32::MAX as usize) as i32
    } else {
        trimmed.chars().count().min(i32::MAX as usize) as i32
    }
}
