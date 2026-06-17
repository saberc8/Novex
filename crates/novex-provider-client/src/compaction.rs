use novex_model::{normalize_model_provider_usage, ModelTokenUsage};
use serde_json::Value;

use crate::{
    model_chat_sse_data_payloads, model_provider_response_id_from_payload,
    model_provider_response_status_from_payload, ModelProviderClientError,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ModelChatCompactionProviderOutput {
    pub answer: String,
    pub usage: ModelTokenUsage,
    pub provider_response_id: Option<String>,
    pub provider_response_status: Option<String>,
}

pub fn parse_model_chat_compaction_provider_output_from_text(
    body_text: &str,
) -> Result<ModelChatCompactionProviderOutput, ModelProviderClientError> {
    let trimmed = body_text.trim();
    if let Ok(body) = serde_json::from_str::<Value>(trimmed) {
        parse_model_chat_compaction_provider_output_from_body(&body)
    } else {
        parse_model_chat_compaction_provider_output_from_sse_text(trimmed)
    }
}

pub fn parse_model_chat_compaction_provider_output_from_body(
    body: &Value,
) -> Result<ModelChatCompactionProviderOutput, ModelProviderClientError> {
    let output_items = body
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ModelProviderClientError::BadResponse("LLM compaction 响应缺少 output".to_owned())
        })?;
    let answer = model_chat_compaction_output_from_items(output_items.iter())?;
    Ok(ModelChatCompactionProviderOutput {
        answer,
        usage: normalize_model_provider_usage(body),
        provider_response_id: model_provider_response_id_from_payload(body),
        provider_response_status: model_provider_response_status_from_payload(body),
    })
}

pub fn parse_model_chat_compaction_provider_output_from_sse_text(
    body_text: &str,
) -> Result<ModelChatCompactionProviderOutput, ModelProviderClientError> {
    let mut output_items = Vec::new();
    let mut completed = false;
    let mut usage = ModelTokenUsage::default();
    let mut provider_response_id = None;
    let mut provider_response_status = None;
    for data in model_chat_sse_data_payloads(body_text) {
        if data == "[DONE]" {
            continue;
        }
        let value = serde_json::from_str::<Value>(&data).map_err(|_| {
            ModelProviderClientError::BadResponse("LLM compaction SSE 响应不是合法 JSON".to_owned())
        })?;
        if let Some(response) = value.get("response") {
            if let Some(response_id) = model_provider_response_id_from_payload(response) {
                provider_response_id = Some(response_id);
            }
            if let Some(response_status) = model_provider_response_status_from_payload(response) {
                provider_response_status = Some(response_status);
            }
        }
        match value.get("type").and_then(Value::as_str) {
            Some("response.output_item.done") => {
                if let Some(item) = value.get("item") {
                    output_items.push(item.clone());
                }
            }
            Some("response.completed") => {
                completed = true;
                usage = value
                    .pointer("/response/usage")
                    .map(normalize_model_provider_usage)
                    .unwrap_or_else(|| normalize_model_provider_usage(&value));
            }
            _ => {}
        }
    }

    if !completed {
        return Err(ModelProviderClientError::BadResponse(
            "LLM compaction SSE 响应在 response.completed 前结束".to_owned(),
        ));
    }

    let answer = model_chat_compaction_output_from_items(output_items.iter())?;
    Ok(ModelChatCompactionProviderOutput {
        answer,
        usage,
        provider_response_id,
        provider_response_status,
    })
}

fn model_chat_compaction_output_from_items<'a, I>(
    items: I,
) -> Result<String, ModelProviderClientError>
where
    I: IntoIterator<Item = &'a Value>,
{
    let mut output_item_count = 0usize;
    let mut compaction_count = 0usize;
    let mut answer = None;
    for item in items {
        output_item_count += 1;
        if let Some(compaction) = model_chat_compaction_output_item_text(item)? {
            compaction_count += 1;
            if answer.is_none() {
                answer = Some(compaction);
            }
        }
    }

    if compaction_count != 1 {
        return Err(ModelProviderClientError::BadResponse(format!(
            "LLM compaction 响应应包含 1 个 compaction 输出，实际 {compaction_count}/{output_item_count}"
        )));
    }

    answer
        .ok_or_else(|| ModelProviderClientError::BadResponse("LLM compaction 响应为空".to_owned()))
}

fn model_chat_compaction_output_item_text(
    item: &Value,
) -> Result<Option<String>, ModelProviderClientError> {
    match item.get("type").and_then(Value::as_str) {
        Some("compaction" | "compaction_summary") => item
            .get("encrypted_content")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|content| !content.is_empty())
            .map(|content| Some(content.to_owned()))
            .ok_or_else(|| {
                ModelProviderClientError::BadResponse(
                    "LLM compaction 输出缺少 encrypted_content".to_owned(),
                )
            }),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compaction_body_parser_extracts_single_encrypted_output() {
        let body = json!({
            "id": "resp_compact_123",
            "status": "completed",
            "output": [
                { "type": "message", "content": "ignored" },
                { "type": "compaction", "encrypted_content": "compact summary" }
            ],
            "usage": {
                "input_tokens": 6,
                "output_tokens": 2,
                "total_tokens": 8
            }
        });

        let output = parse_model_chat_compaction_provider_output_from_body(&body).unwrap();

        assert_eq!(output.answer, "compact summary");
        assert_eq!(
            output.provider_response_id.as_deref(),
            Some("resp_compact_123")
        );
        assert_eq!(
            output.provider_response_status.as_deref(),
            Some("completed")
        );
        assert_eq!(output.usage.prompt_tokens, Some(6));
        assert_eq!(output.usage.completion_tokens, Some(2));
        assert_eq!(output.usage.total_tokens, Some(8));
    }

    #[test]
    fn compaction_sse_parser_requires_completed_event() {
        let sse = concat!(
            "event: response.created\n",
            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_123\",\"status\":\"in_progress\"}}\n\n",
            "event: response.output_item.done\n",
            "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"compaction_summary\",\"encrypted_content\":\"compact summary\"}}\n\n",
            "event: response.completed\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_123\",\"status\":\"completed\",\"usage\":{\"input_tokens\":7,\"output_tokens\":3,\"total_tokens\":10}}}\n\n",
        );
        let incomplete = concat!(
            "event: response.output_item.done\n",
            "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"compaction\",\"encrypted_content\":\"compact summary\"}}\n\n",
        );

        let output = parse_model_chat_compaction_provider_output_from_sse_text(sse).unwrap();
        let error =
            parse_model_chat_compaction_provider_output_from_sse_text(incomplete).unwrap_err();

        assert_eq!(output.answer, "compact summary");
        assert_eq!(output.provider_response_id.as_deref(), Some("resp_123"));
        assert_eq!(
            output.provider_response_status.as_deref(),
            Some("completed")
        );
        assert_eq!(output.usage.total_tokens, Some(10));
        assert_eq!(
            error.to_string(),
            "LLM compaction SSE 响应在 response.completed 前结束"
        );
    }
}
