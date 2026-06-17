mod http;
mod media;
mod native_cancel;
mod rag;

pub(super) use http::{send_model_provider_http_request, ModelProviderHttpRequest};
pub(super) use media::{send_model_provider_media_image_request, ModelProviderMediaImageRequest};
pub(super) use native_cancel::{
    send_model_provider_native_cancel_request, ModelProviderNativeCancelRequest,
};
use novex_model::{normalize_model_provider_usage, ModelProviderStreamChunk, ModelTokenUsage};
pub(super) use novex_provider_client::{
    parse_model_provider_embedding_vectors, parse_model_provider_rerank_scores,
};
pub(super) use rag::{
    send_model_provider_embedding_request, send_model_provider_rerank_request,
    ModelProviderEmbeddingRequest, ModelProviderRerankRequest,
};
use serde_json::Value;

use crate::shared::error::AppError;

pub(super) async fn read_model_provider_response_text(
    response: reqwest::Response,
) -> Result<String, AppError> {
    response
        .text()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ModelChatProviderOutput {
    pub(super) answer: String,
    pub(super) usage: ModelTokenUsage,
    pub(super) provider_response_id: Option<String>,
    pub(super) provider_response_status: Option<String>,
    pub(super) delta_chunks: Vec<ModelProviderStreamChunk>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ModelChatCompactionProviderOutput {
    pub(super) answer: String,
    pub(super) usage: ModelTokenUsage,
    pub(super) provider_response_id: Option<String>,
    pub(super) provider_response_status: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct ModelChatStreamCompletionBuilder {
    pub(super) completed: bool,
    pub(super) completed_answer: Option<String>,
    pub(super) usage: ModelTokenUsage,
    pub(super) provider_response_id: Option<String>,
    pub(super) provider_response_status: Option<String>,
    pub(super) delta_chunks: Vec<ModelProviderStreamChunk>,
    pub(super) next_chunk_index: usize,
}

impl ModelChatStreamCompletionBuilder {
    pub(super) fn observe_done(&mut self) {
        self.completed = true;
    }

    pub(super) fn observe_sse_value(&mut self, value: &Value) -> Vec<ModelProviderStreamChunk> {
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

    pub(super) fn finish(self) -> Result<ModelChatProviderOutput, AppError> {
        if !self.completed {
            return Err(AppError::bad_request("LLM chat SSE 响应在完成前结束"));
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
            return Err(AppError::bad_request("LLM chat SSE 响应为空"));
        }

        Ok(ModelChatProviderOutput {
            answer,
            usage: self.usage,
            provider_response_id: self.provider_response_id,
            provider_response_status: self.provider_response_status,
            delta_chunks: self.delta_chunks,
        })
    }

    pub(super) fn provider_response_id(&self) -> Option<String> {
        self.provider_response_id.clone()
    }

    pub(super) fn provider_response_status(&self) -> Option<String> {
        self.provider_response_status.clone()
    }
}

pub(super) fn parse_model_chat_provider_output_from_text(
    body_text: &str,
) -> Result<ModelChatProviderOutput, AppError> {
    let trimmed = body_text.trim();
    if let Ok(body) = serde_json::from_str::<Value>(trimmed) {
        parse_model_chat_provider_output_from_body(&body)
    } else {
        parse_model_chat_provider_output_from_sse_text(trimmed)
    }
}

pub(super) fn parse_model_chat_provider_output_from_body(
    body: &Value,
) -> Result<ModelChatProviderOutput, AppError> {
    let answer = model_chat_answer_from_provider_body(body)
        .ok_or_else(|| AppError::bad_request("LLM 响应为空"))?;
    Ok(ModelChatProviderOutput {
        answer,
        usage: normalize_model_provider_usage(body),
        provider_response_id: model_provider_response_id_from_payload(body),
        provider_response_status: model_provider_response_status_from_payload(body),
        delta_chunks: vec![],
    })
}

fn parse_model_chat_provider_output_from_sse_text(
    body_text: &str,
) -> Result<ModelChatProviderOutput, AppError> {
    let mut builder = ModelChatStreamCompletionBuilder::default();

    for data in model_chat_sse_data_payloads(body_text) {
        if data == "[DONE]" {
            builder.observe_done();
            continue;
        }
        let value = serde_json::from_str::<Value>(&data)
            .map_err(|_| AppError::bad_request("LLM chat SSE 响应不是合法 JSON"))?;
        builder.observe_sse_value(&value);
    }

    builder.finish()
}

pub(super) fn parse_model_chat_compaction_provider_output_from_text(
    body_text: &str,
) -> Result<ModelChatCompactionProviderOutput, AppError> {
    let trimmed = body_text.trim();
    if let Ok(body) = serde_json::from_str::<Value>(trimmed) {
        parse_model_chat_compaction_provider_output_from_body(&body)
    } else {
        parse_model_chat_compaction_provider_output_from_sse_text(trimmed)
    }
}

pub(super) fn parse_model_chat_compaction_provider_output_from_body(
    body: &Value,
) -> Result<ModelChatCompactionProviderOutput, AppError> {
    let output_items = body
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::bad_request("LLM compaction 响应缺少 output"))?;
    let answer = model_chat_compaction_output_from_items(output_items.iter())?;
    Ok(ModelChatCompactionProviderOutput {
        answer,
        usage: normalize_model_provider_usage(body),
        provider_response_id: model_provider_response_id_from_payload(body),
        provider_response_status: model_provider_response_status_from_payload(body),
    })
}

pub(super) fn parse_model_chat_compaction_provider_output_from_sse_text(
    body_text: &str,
) -> Result<ModelChatCompactionProviderOutput, AppError> {
    let mut output_items = Vec::new();
    let mut completed = false;
    let mut usage = ModelTokenUsage::default();
    let mut provider_response_id = None;
    let mut provider_response_status = None;
    for data in model_chat_sse_data_payloads(body_text) {
        if data == "[DONE]" {
            continue;
        }
        let value = serde_json::from_str::<Value>(&data)
            .map_err(|_| AppError::bad_request("LLM compaction SSE 响应不是合法 JSON"))?;
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
        return Err(AppError::bad_request(
            "LLM compaction SSE 响应在 response.completed 前结束",
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

fn model_chat_response_payload_from_sse_value(value: &Value) -> &Value {
    value.get("response").unwrap_or(value)
}

fn model_chat_sse_data_payloads(body_text: &str) -> Vec<String> {
    let normalized = body_text.replace("\r\n", "\n");
    normalized
        .split("\n\n")
        .filter_map(model_chat_sse_record_data_payload)
        .collect()
}

pub(super) fn model_chat_sse_record_data_payload(record: &str) -> Option<String> {
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

fn model_chat_compaction_output_from_items<'a, I>(items: I) -> Result<String, AppError>
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
        return Err(AppError::bad_request(format!(
            "LLM compaction 响应应包含 1 个 compaction 输出，实际 {compaction_count}/{output_item_count}"
        )));
    }

    answer.ok_or_else(|| AppError::bad_request("LLM compaction 响应为空"))
}

fn model_chat_compaction_output_item_text(item: &Value) -> Result<Option<String>, AppError> {
    match item.get("type").and_then(Value::as_str) {
        Some("compaction" | "compaction_summary") => item
            .get("encrypted_content")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|content| !content.is_empty())
            .map(|content| Some(content.to_owned()))
            .ok_or_else(|| AppError::bad_request("LLM compaction 输出缺少 encrypted_content")),
        _ => Ok(None),
    }
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

pub(super) fn model_provider_response_id_from_payloads(
    request: &Value,
    response: &Value,
) -> Option<String> {
    [request, response]
        .into_iter()
        .find_map(model_provider_response_id_from_payload)
}

pub(super) fn model_provider_response_id_from_payload(payload: &Value) -> Option<String> {
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

pub(super) fn normalize_model_provider_response_id(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.contains('/') || value.contains('?') || value.contains('#') {
        return None;
    }
    Some(value.to_owned())
}

fn model_provider_response_status_from_payload(payload: &Value) -> Option<String> {
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
