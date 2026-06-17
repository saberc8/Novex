use novex_model::{normalize_model_provider_usage, ModelProviderStreamChunk, ModelTokenUsage};
use serde_json::Value;

use crate::ModelProviderClientError;

#[derive(Debug, Clone, PartialEq)]
pub struct ModelChatProviderOutput {
    pub answer: String,
    pub usage: ModelTokenUsage,
    pub provider_response_id: Option<String>,
    pub provider_response_status: Option<String>,
    pub delta_chunks: Vec<ModelProviderStreamChunk>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelChatStreamCompletionBuilder {
    pub completed: bool,
    pub completed_answer: Option<String>,
    pub usage: ModelTokenUsage,
    pub provider_response_id: Option<String>,
    pub provider_response_status: Option<String>,
    pub delta_chunks: Vec<ModelProviderStreamChunk>,
    pub next_chunk_index: usize,
}

impl ModelChatStreamCompletionBuilder {
    pub fn observe_done(&mut self) {
        self.completed = true;
    }

    pub fn observe_sse_value(&mut self, value: &Value) -> Vec<ModelProviderStreamChunk> {
        let response_payload = model_chat_response_payload_from_sse_value(value);
        if let Some(response_id) = model_provider_response_id_from_payload(response_payload) {
            self.provider_response_id = Some(response_id);
        }
        if let Some(response_status) = model_provider_response_status_from_payload(response_payload)
        {
            self.provider_response_status = Some(response_status);
        }
        let usage_payload = if response_payload.get("usage").is_some() {
            response_payload
        } else {
            value
        };
        if usage_payload.get("usage").is_some() {
            self.usage = normalize_model_provider_usage(usage_payload);
        }
        let (chunks, next_index) =
            model_chat_provider_delta_chunks_from_sse_value(value, self.next_chunk_index);
        self.next_chunk_index = next_index;
        self.delta_chunks.extend(chunks.iter().cloned());
        if model_chat_sse_value_is_terminal(value) {
            self.completed = true;
            self.completed_answer = model_chat_answer_from_provider_body(response_payload);
        }
        chunks
    }

    pub fn finish(self) -> Result<ModelChatProviderOutput, ModelProviderClientError> {
        if !self.completed {
            return Err(ModelProviderClientError::BadResponse(
                "LLM chat SSE 响应在完成前结束".to_owned(),
            ));
        }

        let answer = self
            .delta_chunks
            .iter()
            .map(|chunk| chunk.content.as_str())
            .collect::<String>();
        let answer = if answer.is_empty() {
            self.completed_answer.unwrap_or_default()
        } else {
            answer
        };
        if answer.is_empty() {
            return Err(ModelProviderClientError::BadResponse(
                "LLM chat SSE 响应为空".to_owned(),
            ));
        }

        Ok(ModelChatProviderOutput {
            answer,
            usage: self.usage,
            provider_response_id: self.provider_response_id,
            provider_response_status: self.provider_response_status,
            delta_chunks: self.delta_chunks,
        })
    }

    pub fn provider_response_id(&self) -> Option<String> {
        self.provider_response_id.clone()
    }

    pub fn provider_response_status(&self) -> Option<String> {
        self.provider_response_status.clone()
    }
}

pub fn parse_model_chat_provider_output_from_text(
    body_text: &str,
) -> Result<ModelChatProviderOutput, ModelProviderClientError> {
    let trimmed = body_text.trim();
    if let Ok(body) = serde_json::from_str::<Value>(trimmed) {
        parse_model_chat_provider_output_from_body(&body)
    } else {
        parse_model_chat_provider_output_from_sse_text(trimmed)
    }
}

pub fn parse_model_chat_provider_output_from_body(
    body: &Value,
) -> Result<ModelChatProviderOutput, ModelProviderClientError> {
    let answer = model_chat_answer_from_provider_body(body)
        .ok_or_else(|| ModelProviderClientError::BadResponse("LLM 响应为空".to_owned()))?;
    Ok(ModelChatProviderOutput {
        answer,
        usage: normalize_model_provider_usage(body),
        provider_response_id: model_provider_response_id_from_payload(body),
        provider_response_status: model_provider_response_status_from_payload(body),
        delta_chunks: vec![],
    })
}

pub fn parse_model_chat_provider_output_from_sse_text(
    body_text: &str,
) -> Result<ModelChatProviderOutput, ModelProviderClientError> {
    let mut builder = ModelChatStreamCompletionBuilder::default();

    for data in model_chat_sse_data_payloads(body_text) {
        if data == "[DONE]" {
            builder.observe_done();
            continue;
        }
        let value = serde_json::from_str::<Value>(&data).map_err(|_| {
            ModelProviderClientError::BadResponse("LLM chat SSE 响应不是合法 JSON".to_owned())
        })?;
        builder.observe_sse_value(&value);
    }

    builder.finish()
}

fn model_chat_response_payload_from_sse_value(value: &Value) -> &Value {
    value.get("response").unwrap_or(value)
}

pub(crate) fn model_chat_sse_data_payloads(body_text: &str) -> Vec<String> {
    let normalized = body_text.replace("\r\n", "\n");
    normalized
        .split("\n\n")
        .filter_map(model_chat_sse_record_data_payload)
        .collect()
}

pub fn model_chat_sse_record_data_payload(record: &str) -> Option<String> {
    let data = record
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");
    (!data.trim().is_empty()).then(|| data.trim().to_owned())
}

fn model_chat_provider_delta_chunks_from_sse_value(
    value: &Value,
    next_chunk_index: usize,
) -> (Vec<ModelProviderStreamChunk>, usize) {
    let provider_event = model_chat_provider_event_name(value);
    if let Some(content) = model_chat_responses_delta_content_from_value(value) {
        return (
            vec![ModelProviderStreamChunk {
                index: next_chunk_index,
                content,
                provider_event,
            }],
            next_chunk_index + 1,
        );
    }

    let Some(choices) = value.get("choices").and_then(Value::as_array) else {
        return (vec![], next_chunk_index);
    };

    let mut chunks = Vec::new();
    let mut index = next_chunk_index;
    for choice in choices {
        if let Some(content) = model_chat_delta_content_from_choice(choice) {
            chunks.push(ModelProviderStreamChunk {
                index,
                content,
                provider_event: provider_event.clone(),
            });
            index += 1;
        }
    }

    (chunks, index)
}

fn model_chat_responses_delta_content_from_value(value: &Value) -> Option<String> {
    if value.get("type").and_then(Value::as_str) != Some("response.output_text.delta") {
        return None;
    }
    value
        .get("delta")
        .and_then(model_chat_delta_text_from_value)
}

fn model_chat_delta_content_from_choice(choice: &Value) -> Option<String> {
    ["/delta/content", "/message/content", "/text"]
        .into_iter()
        .filter_map(|pointer| choice.pointer(pointer))
        .find_map(model_chat_delta_text_from_value)
}

fn model_chat_delta_text_from_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.is_empty() => Some(text.to_owned()),
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .and_then(Value::as_str)
                })
                .collect::<String>();
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}

fn model_chat_provider_event_name(value: &Value) -> Option<String> {
    ["object", "type"]
        .into_iter()
        .filter_map(|key| value.get(key))
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .next()
}

fn model_chat_sse_value_is_terminal(value: &Value) -> bool {
    if value.get("type").and_then(Value::as_str) == Some("response.completed") {
        return true;
    }

    value
        .get("choices")
        .and_then(Value::as_array)
        .is_some_and(|choices| {
            choices.iter().any(|choice| {
                choice
                    .get("finish_reason")
                    .is_some_and(|finish_reason| !finish_reason.is_null())
            })
        })
}

fn model_chat_answer_from_provider_body(body: &Value) -> Option<String> {
    for pointer in [
        "/choices/0/message/content",
        "/choices/0/text",
        "/output_text",
    ] {
        if let Some(value) = body.pointer(pointer) {
            if let Some(answer) = model_chat_text_from_value(value) {
                return Some(answer);
            }
        }
    }
    model_chat_responses_output_text_from_body(body)
}

fn model_chat_responses_output_text_from_body(body: &Value) -> Option<String> {
    let output_items = body.get("output").and_then(Value::as_array)?;
    let text = output_items
        .iter()
        .filter_map(|item| {
            item.get("content")
                .and_then(model_chat_text_from_value)
                .or_else(|| item.get("text").and_then(model_chat_text_from_value))
        })
        .collect::<Vec<_>>()
        .join("\n");
    non_empty_model_chat_text(&text)
}

fn model_chat_text_from_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => non_empty_model_chat_text(text),
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|part| !part.is_empty())
                })
                .collect::<Vec<_>>()
                .join("\n");
            non_empty_model_chat_text(&text)
        }
        _ => None,
    }
}

fn non_empty_model_chat_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

pub fn model_provider_response_id_from_payloads(
    request: &Value,
    response: &Value,
) -> Option<String> {
    [request, response]
        .into_iter()
        .find_map(model_provider_response_id_from_payload)
}

pub fn model_provider_response_id_from_payload(payload: &Value) -> Option<String> {
    [
        payload.get("providerResponseId"),
        payload.get("responseId"),
        payload.get("id"),
    ]
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .find_map(normalize_model_provider_response_id)
}

pub fn normalize_model_provider_response_id(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.contains('/') || value.contains('?') || value.contains('#') {
        return None;
    }
    Some(value.to_owned())
}

pub(crate) fn model_provider_response_status_from_payload(payload: &Value) -> Option<String> {
    [
        payload.get("providerResponseStatus"),
        payload.get("responseStatus"),
        payload.get("status"),
    ]
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(str::to_owned)
    .next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn chat_body_parser_maps_openai_and_responses_output_shapes() {
        let openai_body = json!({
            "id": "chatcmpl_123",
            "status": "completed",
            "choices": [
                { "message": { "content": "hello from chat completions" } }
            ],
            "usage": {
                "prompt_tokens": 4,
                "completion_tokens": 5,
                "total_tokens": 9
            }
        });
        let responses_body = json!({
            "id": "resp_123",
            "status": "completed",
            "output": [
                {
                    "type": "message",
                    "content": [
                        { "type": "output_text", "text": "hello from responses" }
                    ]
                }
            ]
        });

        let openai_output = parse_model_chat_provider_output_from_body(&openai_body).unwrap();
        let responses_output = parse_model_chat_provider_output_from_body(&responses_body).unwrap();

        assert_eq!(openai_output.answer, "hello from chat completions");
        assert_eq!(
            openai_output.provider_response_id.as_deref(),
            Some("chatcmpl_123")
        );
        assert_eq!(
            openai_output.provider_response_status.as_deref(),
            Some("completed")
        );
        assert_eq!(openai_output.usage.prompt_tokens, Some(4));
        assert_eq!(openai_output.usage.completion_tokens, Some(5));
        assert_eq!(openai_output.usage.total_tokens, Some(9));
        assert_eq!(responses_output.answer, "hello from responses");
        assert_eq!(
            responses_output.provider_response_id.as_deref(),
            Some("resp_123")
        );
    }

    #[test]
    fn chat_sse_parser_assembles_delta_chunks_and_metadata() {
        let sse = concat!(
            "data: {\"id\":\"chatcmpl_123\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl_123\",\"choices\":[{\"delta\":{\"content\":\" world\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":2,\"total_tokens\":4}}\n\n",
        );

        let output = parse_model_chat_provider_output_from_sse_text(sse).unwrap();

        assert_eq!(output.answer, "Hello world");
        assert_eq!(output.provider_response_id.as_deref(), Some("chatcmpl_123"));
        assert_eq!(output.delta_chunks.len(), 2);
        assert_eq!(output.delta_chunks[0].index, 0);
        assert_eq!(output.delta_chunks[0].content, "Hello");
        assert_eq!(output.delta_chunks[1].index, 1);
        assert_eq!(output.delta_chunks[1].content, " world");
        assert_eq!(output.usage.total_tokens, Some(4));
    }
}
