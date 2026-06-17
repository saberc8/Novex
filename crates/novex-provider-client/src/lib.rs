use std::{error::Error, fmt, time::Duration};

use novex_model::{
    normalize_model_provider_usage, ModelEmbeddingVector, ModelMediaImageGenerationResp,
    ModelProviderStreamChunk, ModelProviderType, ModelRerankScore, ModelTokenUsage,
};
use novex_tools::{parse_media_image_generation_response, MediaImageGenerationRequest};
use serde_json::{json, Value};

pub const CRATE_ID: &str = "novex-provider-client";

#[derive(Debug)]
pub enum ModelProviderClientError {
    Transport(reqwest::Error),
    HttpStatus {
        failure_message: String,
        status: u16,
    },
    BadResponse(String),
}

impl fmt::Display for ModelProviderClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(err) => write!(f, "{err}"),
            Self::HttpStatus {
                failure_message,
                status,
            } => write!(f, "{failure_message}: HTTP {status}"),
            Self::BadResponse(message) => write!(f, "{message}"),
        }
    }
}

impl Error for ModelProviderClientError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport(err) => Some(err),
            Self::HttpStatus { .. } | Self::BadResponse(_) => None,
        }
    }
}

pub struct ModelProviderHttpRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub payload: &'a Value,
    pub timeout: Duration,
    pub failure_message: &'a str,
}

pub struct ModelProviderChatRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub payload: &'a Value,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelProviderChatTransport {
    ChatCompletions,
    ResponsesCodeAgent,
    ResponsesCompactionV2,
    ResponsesCompactUnary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelProviderChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelProviderChatFileContext {
    pub name: String,
    pub content_type: String,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelProviderChatRequestKind {
    Compaction,
}

impl ModelProviderChatRequestKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compaction => "compaction",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelProviderChatCompactionMetadata {
    pub implementation: String,
    pub trigger: String,
    pub reason: String,
    pub phase: String,
    pub strategy: String,
    pub window_id: u64,
    pub input_history_count: usize,
    pub retained_history_count: usize,
    pub compacted_item_count: usize,
    pub retained_item_count: usize,
    pub tool_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelProviderChatRequestMetadata {
    pub request_kind: ModelProviderChatRequestKind,
    pub compaction: Option<ModelProviderChatCompactionMetadata>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelProviderChatPlanInput {
    pub provider: ModelProviderType,
    pub model: Option<String>,
    pub base_url: String,
    pub endpoint: String,
    pub messages: Vec<ModelProviderChatMessage>,
    pub file_contexts: Vec<ModelProviderChatFileContext>,
    pub temperature: f64,
    pub max_tokens: u32,
    pub response_format: Option<Value>,
    pub request_metadata: Option<ModelProviderChatRequestMetadata>,
    pub should_stream_chat_completion: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelProviderChatPlan {
    pub endpoint: String,
    pub payload: Value,
    pub transport: ModelProviderChatTransport,
}

pub fn model_provider_http_client(timeout: Duration) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder().timeout(timeout).build()
}

pub fn build_model_provider_chat_plan(input: ModelProviderChatPlanInput) -> ModelProviderChatPlan {
    if model_provider_chat_supports_responses_compaction(input.provider)
        && model_provider_chat_is_compaction(&input)
    {
        if matches!(
            model_provider_chat_compaction_implementation(&input),
            Some("responses_compaction_unary")
        ) {
            return ModelProviderChatPlan {
                endpoint: join_model_provider_endpoint(&input.base_url, Some("responses/compact")),
                payload: model_provider_chat_responses_compact_unary_payload(&input),
                transport: ModelProviderChatTransport::ResponsesCompactUnary,
            };
        }
        return ModelProviderChatPlan {
            endpoint: join_model_provider_endpoint(&input.base_url, Some("responses")),
            payload: model_provider_chat_responses_compaction_payload(&input),
            transport: ModelProviderChatTransport::ResponsesCompactionV2,
        };
    }

    if model_provider_chat_uses_responses_code_agent(&input) {
        return ModelProviderChatPlan {
            endpoint: input.endpoint.clone(),
            payload: model_provider_chat_responses_code_agent_payload(&input),
            transport: ModelProviderChatTransport::ResponsesCodeAgent,
        };
    }

    ModelProviderChatPlan {
        endpoint: input.endpoint.clone(),
        payload: model_provider_chat_completion_payload(&input),
        transport: ModelProviderChatTransport::ChatCompletions,
    }
}

pub fn model_provider_chat_plan_streams_chat_completion(plan: &ModelProviderChatPlan) -> bool {
    matches!(
        plan.transport,
        ModelProviderChatTransport::ChatCompletions
            | ModelProviderChatTransport::ResponsesCodeAgent
    ) && plan
        .payload
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn model_provider_chat_is_compaction(input: &ModelProviderChatPlanInput) -> bool {
    matches!(
        input
            .request_metadata
            .as_ref()
            .map(|metadata| metadata.request_kind),
        Some(ModelProviderChatRequestKind::Compaction)
    )
}

fn model_provider_chat_compaction_implementation(
    input: &ModelProviderChatPlanInput,
) -> Option<&str> {
    input
        .request_metadata
        .as_ref()
        .and_then(|metadata| metadata.compaction.as_ref())
        .map(|compaction| compaction.implementation.as_str())
}

fn model_provider_chat_supports_responses_compaction(provider: ModelProviderType) -> bool {
    matches!(
        provider,
        ModelProviderType::OpenAiCompatible | ModelProviderType::LocalRuntime
    )
}

fn model_provider_chat_uses_responses_code_agent(input: &ModelProviderChatPlanInput) -> bool {
    model_provider_chat_supports_responses_compaction(input.provider)
        && input.should_stream_chat_completion
        && model_provider_chat_endpoint_is_responses(&input.endpoint)
}

fn model_provider_chat_endpoint_is_responses(endpoint: &str) -> bool {
    endpoint
        .trim()
        .trim_end_matches('/')
        .ends_with("/responses")
}

fn model_provider_chat_responses_code_agent_payload(input: &ModelProviderChatPlanInput) -> Value {
    let mut payload = json!({
        "model": input.model.as_deref().unwrap_or_default(),
        "input": model_provider_chat_message_input_items(input),
        "temperature": input.temperature,
        "max_output_tokens": input.max_tokens,
        "stream": true,
    });
    if let Some(metadata) = model_provider_chat_metadata(input) {
        payload["metadata"] = metadata;
    }
    payload
}

fn model_provider_chat_responses_compaction_payload(input: &ModelProviderChatPlanInput) -> Value {
    let mut response_input = model_provider_chat_message_input_items(input);
    response_input.push(json!({ "type": "compaction_trigger" }));

    let mut payload = json!({
        "model": input.model.as_deref().unwrap_or_default(),
        "input": response_input,
        "temperature": input.temperature,
        "max_output_tokens": input.max_tokens,
        "background": true,
        "store": true,
        "stream": true,
    });
    if let Some(metadata) = model_provider_chat_metadata(input) {
        payload["metadata"] = metadata;
    }
    payload
}

fn model_provider_chat_responses_compact_unary_payload(
    input: &ModelProviderChatPlanInput,
) -> Value {
    let mut payload = json!({
        "model": input.model.as_deref().unwrap_or_default(),
        "input": model_provider_chat_message_input_items(input),
        "tools": [],
        "parallel_tool_calls": false,
    });
    if let Some(metadata) = model_provider_chat_metadata(input) {
        payload["metadata"] = metadata;
    }
    payload
}

fn model_provider_chat_message_input_items(input: &ModelProviderChatPlanInput) -> Vec<Value> {
    let mut messages = Vec::new();
    if !input.file_contexts.is_empty() {
        messages.push(json!({
            "type": "message",
            "role": "system",
            "content": [{ "type": "input_text", "text": model_provider_chat_file_context_prompt(&input.file_contexts) }],
        }));
    }
    messages.extend(input.messages.iter().map(|message| {
        json!({
            "type": "message",
            "role": message.role,
            "content": [{ "type": "input_text", "text": message.content }],
        })
    }));
    messages
}

fn model_provider_chat_completion_payload(input: &ModelProviderChatPlanInput) -> Value {
    let mut messages = Vec::new();
    if !input.file_contexts.is_empty() {
        messages.push(json!({
            "role": "system",
            "content": model_provider_chat_file_context_prompt(&input.file_contexts),
        }));
    }
    messages.extend(input.messages.iter().map(|message| {
        json!({
            "role": message.role,
            "content": message.content,
        })
    }));

    let mut payload = json!({
        "model": input.model.as_deref().unwrap_or_default(),
        "messages": messages,
        "temperature": input.temperature,
        "max_tokens": input.max_tokens,
        "stream": input.should_stream_chat_completion,
    });
    if let Some(response_format) = &input.response_format {
        payload["response_format"] = response_format.clone();
    }
    if let Some(metadata) = model_provider_chat_metadata(input) {
        payload["metadata"] = metadata;
    }
    payload
}

fn model_provider_chat_metadata(input: &ModelProviderChatPlanInput) -> Option<Value> {
    if !model_provider_chat_supports_metadata(input.provider) {
        return None;
    }

    let metadata = input.request_metadata.as_ref()?;
    let mut payload = serde_json::Map::from_iter([(
        "request_kind".to_owned(),
        json!(metadata.request_kind.as_str()),
    )]);

    if let Some(compaction) = &metadata.compaction {
        payload.insert(
            "compaction_implementation".to_owned(),
            json!(compaction.implementation),
        );
        payload.insert("compaction_trigger".to_owned(), json!(compaction.trigger));
        payload.insert("compaction_reason".to_owned(), json!(compaction.reason));
        payload.insert("compaction_phase".to_owned(), json!(compaction.phase));
        payload.insert("compaction_strategy".to_owned(), json!(compaction.strategy));
        payload.insert(
            "compaction_window_id".to_owned(),
            json!(compaction.window_id.to_string()),
        );
        payload.insert(
            "input_history_count".to_owned(),
            json!(compaction.input_history_count.to_string()),
        );
        payload.insert(
            "retained_history_count".to_owned(),
            json!(compaction.retained_history_count.to_string()),
        );
        payload.insert(
            "compacted_item_count".to_owned(),
            json!(compaction.compacted_item_count.to_string()),
        );
        payload.insert(
            "retained_item_count".to_owned(),
            json!(compaction.retained_item_count.to_string()),
        );
        payload.insert(
            "tool_codes".to_owned(),
            json!(compaction.tool_codes.join(",")),
        );
    }

    Some(Value::Object(payload))
}

fn model_provider_chat_supports_metadata(provider: ModelProviderType) -> bool {
    matches!(
        provider,
        ModelProviderType::OpenAiCompatible
            | ModelProviderType::AzureOpenAi
            | ModelProviderType::LocalRuntime
    )
}

fn model_provider_chat_file_context_prompt(files: &[ModelProviderChatFileContext]) -> String {
    let mut sections = vec![
        "Use the following user-provided file context when it is relevant. If the files do not contain enough evidence, say so.".to_owned(),
    ];
    for file in files {
        sections.push(format!(
            "[File: {} | {}]\n{}",
            file.name, file.content_type, file.content
        ));
    }
    sections.join("\n\n")
}

fn join_model_provider_endpoint(base_url: &str, api_path: Option<&str>) -> String {
    let base_url = base_url.trim().trim_end_matches('/');
    let Some(path) = api_path
        .map(str::trim)
        .map(|path| path.trim_matches('/'))
        .filter(|path| !path.is_empty())
    else {
        return base_url.to_owned();
    };
    format!("{base_url}/{path}")
}

pub async fn send_model_provider_http_request(
    request: ModelProviderHttpRequest<'_>,
) -> Result<reqwest::Response, ModelProviderClientError> {
    let client =
        model_provider_http_client(request.timeout).map_err(ModelProviderClientError::Transport)?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .json(request.payload)
        .send()
        .await
        .map_err(ModelProviderClientError::Transport)?;
    let status = response.status();

    if !status.is_success() {
        return Err(ModelProviderClientError::HttpStatus {
            failure_message: request.failure_message.to_owned(),
            status: status.as_u16(),
        });
    }

    Ok(response)
}

pub async fn send_model_provider_chat_request(
    request: ModelProviderChatRequest<'_>,
) -> Result<reqwest::Response, ModelProviderClientError> {
    send_model_provider_http_request(ModelProviderHttpRequest {
        endpoint: request.endpoint,
        api_key: request.api_key,
        payload: request.payload,
        timeout: request.timeout,
        failure_message: "LLM 模型调用失败",
    })
    .await
}

pub async fn send_model_provider_chat_unary_request(
    request: ModelProviderChatRequest<'_>,
) -> Result<String, ModelProviderClientError> {
    let response = send_model_provider_chat_request(request).await?;
    read_model_provider_response_text(response).await
}

pub async fn read_model_provider_response_text(
    response: reqwest::Response,
) -> Result<String, ModelProviderClientError> {
    response
        .text()
        .await
        .map_err(ModelProviderClientError::Transport)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelChatProviderOutput {
    pub answer: String,
    pub usage: ModelTokenUsage,
    pub provider_response_id: Option<String>,
    pub provider_response_status: Option<String>,
    pub delta_chunks: Vec<ModelProviderStreamChunk>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelChatCompactionProviderOutput {
    pub answer: String,
    pub usage: ModelTokenUsage,
    pub provider_response_id: Option<String>,
    pub provider_response_status: Option<String>,
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

pub struct ModelProviderNativeCancelRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub timeout: Duration,
}

pub async fn send_model_provider_native_cancel_request(
    request: ModelProviderNativeCancelRequest<'_>,
) -> Result<u16, ModelProviderClientError> {
    let client =
        model_provider_http_client(request.timeout).map_err(ModelProviderClientError::Transport)?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .send()
        .await
        .map_err(ModelProviderClientError::Transport)?;
    let status = response.status();

    if !status.is_success() {
        return Err(ModelProviderClientError::HttpStatus {
            failure_message: "Provider native cancel failed".to_owned(),
            status: status.as_u16(),
        });
    }

    Ok(status.as_u16())
}

pub struct ModelProviderMediaImageRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub request: &'a MediaImageGenerationRequest,
    pub timeout: Duration,
}

pub async fn send_model_provider_media_image_request(
    request: ModelProviderMediaImageRequest<'_>,
) -> Result<ModelMediaImageGenerationResp, ModelProviderClientError> {
    let request_payload = request.request.to_provider_payload();
    let client = model_provider_http_client(request.timeout).map_err(|err| {
        ModelProviderClientError::BadResponse(format!("图片生成客户端初始化失败: {err}"))
    })?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .header("x-api-key", request.api_key)
        .json(&request_payload)
        .send()
        .await
        .map_err(|err| ModelProviderClientError::BadResponse(format!("图片生成请求失败: {err}")))?;
    let status = response.status();
    let provider_payload = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !status.is_success() {
        return Err(ModelProviderClientError::BadResponse(format!(
            "图片生成请求失败: HTTP {}",
            status.as_u16()
        )));
    }
    let Some(result) = parse_media_image_generation_response(&provider_payload) else {
        return Err(ModelProviderClientError::BadResponse(
            "图片生成响应缺少资产 URL".to_owned(),
        ));
    };

    Ok(ModelMediaImageGenerationResp {
        provider_payload,
        asset_url: result.asset_url,
        provider_asset_id: result.provider_asset_id,
    })
}

pub struct ModelProviderEmbeddingRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub model: Option<&'a str>,
    pub texts: &'a [String],
    pub timeout: Duration,
}

pub struct ModelProviderRerankRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub model: Option<&'a str>,
    pub query: &'a str,
    pub documents: &'a [String],
    pub timeout: Duration,
}

pub async fn send_model_provider_embedding_request(
    request: ModelProviderEmbeddingRequest<'_>,
) -> Result<Vec<ModelEmbeddingVector>, ModelProviderClientError> {
    let client =
        model_provider_http_client(request.timeout).map_err(ModelProviderClientError::Transport)?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .json(&json!({
            "model": request.model.unwrap_or_default(),
            "input": request.texts,
        }))
        .send()
        .await
        .map_err(ModelProviderClientError::Transport)?;
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(ModelProviderClientError::BadResponse(format!(
            "Embedding 模型调用失败: {status}"
        )));
    }
    let vectors = parse_model_provider_embedding_vectors(&body);
    if vectors.is_empty() {
        return Err(ModelProviderClientError::BadResponse(
            "Embedding 模型响应为空".to_owned(),
        ));
    }
    Ok(vectors)
}

pub async fn send_model_provider_rerank_request(
    request: ModelProviderRerankRequest<'_>,
) -> Result<Vec<ModelRerankScore>, ModelProviderClientError> {
    let client =
        model_provider_http_client(request.timeout).map_err(ModelProviderClientError::Transport)?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .json(&json!({
            "model": request.model.unwrap_or_default(),
            "query": request.query,
            "documents": request.documents,
        }))
        .send()
        .await
        .map_err(ModelProviderClientError::Transport)?;
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(ModelProviderClientError::BadResponse(format!(
            "Rerank 模型调用失败: {status}"
        )));
    }
    let scores = parse_model_provider_rerank_scores(&body);
    if scores.is_empty() {
        return Err(ModelProviderClientError::BadResponse(
            "Rerank 模型响应为空".to_owned(),
        ));
    }
    Ok(scores)
}

pub fn parse_model_provider_rerank_scores(body: &Value) -> Vec<ModelRerankScore> {
    body.get("results")
        .or_else(|| body.get("data"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_rerank_score)
        .collect()
}

pub fn parse_model_provider_embedding_vectors(body: &Value) -> Vec<ModelEmbeddingVector> {
    body.get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_embedding_vector)
        .collect()
}

fn parse_rerank_score(value: &Value) -> Option<ModelRerankScore> {
    let index = value
        .get("index")
        .and_then(json_usize)
        .or_else(|| value.get("document_index").and_then(json_usize))
        .or_else(|| value.get("documentIndex").and_then(json_usize))?;
    let score = value
        .get("relevance_score")
        .or_else(|| value.get("relevanceScore"))
        .or_else(|| value.get("score"))
        .and_then(json_f32)?;
    if !score.is_finite() {
        return None;
    }
    Some(ModelRerankScore { index, score })
}

fn parse_embedding_vector(value: &Value) -> Option<ModelEmbeddingVector> {
    let index = value.get("index").and_then(json_usize).unwrap_or(0);
    let vector = value
        .get("embedding")?
        .as_array()?
        .iter()
        .filter_map(json_f32)
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if vector.is_empty() {
        return None;
    }
    Some(ModelEmbeddingVector { index, vector })
}

fn json_usize(value: &Value) -> Option<usize> {
    if let Some(value) = value.as_u64() {
        return usize::try_from(value).ok();
    }
    value
        .as_str()
        .and_then(|text| text.trim().parse::<usize>().ok())
}

fn json_f32(value: &Value) -> Option<f32> {
    if let Some(value) = value.as_f64() {
        return Some(value as f32);
    }
    value
        .as_str()
        .and_then(|text| text.trim().parse::<f32>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn module_describes_provider_client_boundary() {
        assert_eq!(CRATE_ID, "novex-provider-client");
    }

    #[test]
    fn http_status_error_preserves_backend_message_shape() {
        let error = ModelProviderClientError::HttpStatus {
            failure_message: "LLM 模型调用失败".to_owned(),
            status: 429,
        };

        assert_eq!(error.to_string(), "LLM 模型调用失败: HTTP 429");
    }

    #[test]
    fn http_request_carries_provider_post_inputs() {
        let payload = json!({"model": "demo", "input": "hello"});
        let request = ModelProviderHttpRequest {
            endpoint: "https://provider.example/v1/chat/completions",
            api_key: "secret",
            payload: &payload,
            timeout: Duration::from_secs(15),
            failure_message: "LLM 模型调用失败",
        };

        assert_eq!(
            request.endpoint,
            "https://provider.example/v1/chat/completions"
        );
        assert_eq!(request.api_key, "secret");
        assert_eq!(request.payload["model"], "demo");
        assert_eq!(request.timeout, Duration::from_secs(15));
        assert_eq!(request.failure_message, "LLM 模型调用失败");
    }

    #[test]
    fn chat_request_carries_provider_dispatch_inputs() {
        let payload = json!({"model": "gpt-compatible", "messages": [], "stream": false});
        let request = ModelProviderChatRequest {
            endpoint: "https://provider.example/v1/chat/completions",
            api_key: "secret",
            payload: &payload,
            timeout: Duration::from_secs(120),
        };

        assert_eq!(
            request.endpoint,
            "https://provider.example/v1/chat/completions"
        );
        assert_eq!(request.api_key, "secret");
        assert_eq!(request.payload["model"], "gpt-compatible");
        assert_eq!(request.timeout, Duration::from_secs(120));
    }

    #[test]
    fn chat_http_status_error_preserves_backend_message_shape() {
        let error = ModelProviderClientError::HttpStatus {
            failure_message: "LLM 模型调用失败".to_owned(),
            status: 503,
        };

        assert_eq!(error.to_string(), "LLM 模型调用失败: HTTP 503");
    }

    #[test]
    fn chat_plan_builder_maps_regular_chat_completion_payload() {
        let input = ModelProviderChatPlanInput {
            provider: ModelProviderType::DeepSeek,
            model: Some("deepseek-v4-flash".to_owned()),
            base_url: "https://llm.internal/v1".to_owned(),
            endpoint: "https://llm.internal/v1/chat/completions".to_owned(),
            messages: vec![ModelProviderChatMessage {
                role: "user".to_owned(),
                content: "hello".to_owned(),
            }],
            file_contexts: Vec::new(),
            temperature: 0.2,
            max_tokens: 1024,
            response_format: Some(json!({"type": "json_object"})),
            request_metadata: None,
            should_stream_chat_completion: false,
        };

        let plan = build_model_provider_chat_plan(input);

        assert_eq!(plan.transport, ModelProviderChatTransport::ChatCompletions);
        assert_eq!(plan.endpoint, "https://llm.internal/v1/chat/completions");
        assert_eq!(plan.payload["model"], "deepseek-v4-flash");
        assert_eq!(plan.payload["messages"][0]["role"], "user");
        assert_eq!(plan.payload["messages"][0]["content"], "hello");
        assert_eq!(plan.payload["stream"], false);
        assert_eq!(plan.payload["response_format"]["type"], "json_object");
    }

    #[test]
    fn chat_plan_builder_maps_responses_compaction_and_metadata() {
        let input = ModelProviderChatPlanInput {
            provider: ModelProviderType::OpenAiCompatible,
            model: Some("gpt-compatible".to_owned()),
            base_url: "https://llm.internal/v1/".to_owned(),
            endpoint: "https://llm.internal/v1/chat/completions".to_owned(),
            messages: vec![ModelProviderChatMessage {
                role: "user".to_owned(),
                content: "compact this context".to_owned(),
            }],
            file_contexts: Vec::new(),
            temperature: 0.2,
            max_tokens: 512,
            response_format: None,
            request_metadata: Some(test_provider_compaction_metadata("responses_compaction_v2")),
            should_stream_chat_completion: false,
        };

        let plan = build_model_provider_chat_plan(input);

        assert_eq!(
            plan.transport,
            ModelProviderChatTransport::ResponsesCompactionV2
        );
        assert_eq!(plan.endpoint, "https://llm.internal/v1/responses");
        assert_eq!(plan.payload["stream"], true);
        assert_eq!(plan.payload["background"], true);
        assert_eq!(plan.payload["store"], true);
        assert_eq!(plan.payload["metadata"]["request_kind"], "compaction");
        assert_eq!(plan.payload["metadata"]["tool_codes"], "rag.search");
        assert_eq!(
            plan.payload["input"].as_array().unwrap().last().unwrap()["type"],
            "compaction_trigger"
        );
    }

    #[test]
    fn chat_plan_builder_maps_unary_compaction_endpoint() {
        let input = ModelProviderChatPlanInput {
            provider: ModelProviderType::OpenAiCompatible,
            model: Some("gpt-compatible".to_owned()),
            base_url: "https://llm.internal/v1".to_owned(),
            endpoint: "https://llm.internal/v1/chat/completions".to_owned(),
            messages: vec![ModelProviderChatMessage {
                role: "user".to_owned(),
                content: "compact this context".to_owned(),
            }],
            file_contexts: Vec::new(),
            temperature: 0.2,
            max_tokens: 512,
            response_format: None,
            request_metadata: Some(test_provider_compaction_metadata(
                "responses_compaction_unary",
            )),
            should_stream_chat_completion: false,
        };

        let plan = build_model_provider_chat_plan(input);

        assert_eq!(
            plan.transport,
            ModelProviderChatTransport::ResponsesCompactUnary
        );
        assert_eq!(plan.endpoint, "https://llm.internal/v1/responses/compact");
        assert!(plan.payload.get("stream").is_none());
        assert_eq!(plan.payload["tools"].as_array().unwrap().len(), 0);
        assert_eq!(plan.payload["parallel_tool_calls"], false);
    }

    #[test]
    fn chat_plan_builder_maps_responses_code_agent_streaming() {
        let input = ModelProviderChatPlanInput {
            provider: ModelProviderType::OpenAiCompatible,
            model: Some("qwen-private".to_owned()),
            base_url: "https://llm.internal/v1".to_owned(),
            endpoint: "https://llm.internal/v1/responses".to_owned(),
            messages: vec![ModelProviderChatMessage {
                role: "user".to_owned(),
                content: "use tools if needed".to_owned(),
            }],
            file_contexts: Vec::new(),
            temperature: 0.2,
            max_tokens: 768,
            response_format: None,
            request_metadata: None,
            should_stream_chat_completion: true,
        };

        let plan = build_model_provider_chat_plan(input);

        assert_eq!(
            plan.transport,
            ModelProviderChatTransport::ResponsesCodeAgent
        );
        assert_eq!(plan.endpoint, "https://llm.internal/v1/responses");
        assert_eq!(plan.payload["stream"], true);
        assert!(plan.payload.get("input").is_some());
        assert!(plan.payload.get("messages").is_none());
        assert!(model_provider_chat_plan_streams_chat_completion(&plan));
    }

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

    #[test]
    fn native_cancel_request_carries_provider_dispatch_inputs() {
        let request = ModelProviderNativeCancelRequest {
            endpoint: "https://provider.example/v1/responses/resp_123/cancel",
            api_key: "secret",
            timeout: Duration::from_secs(8),
        };

        assert_eq!(
            request.endpoint,
            "https://provider.example/v1/responses/resp_123/cancel"
        );
        assert_eq!(request.api_key, "secret");
        assert_eq!(request.timeout, Duration::from_secs(8));
    }

    #[test]
    fn native_cancel_http_status_error_preserves_backend_message_shape() {
        let error = ModelProviderClientError::HttpStatus {
            failure_message: "Provider native cancel failed".to_owned(),
            status: 409,
        };

        assert_eq!(error.to_string(), "Provider native cancel failed: HTTP 409");
    }

    #[test]
    fn media_image_request_carries_provider_dispatch_inputs() {
        let media_request = MediaImageGenerationRequest::new("draw a support diagram")
            .with_size("1024x1024")
            .with_count(2);
        let request = ModelProviderMediaImageRequest {
            endpoint: "https://provider.example/v1/images/generations",
            api_key: "secret",
            request: &media_request,
            timeout: Duration::from_secs(45),
        };

        assert_eq!(
            request.endpoint,
            "https://provider.example/v1/images/generations"
        );
        assert_eq!(request.api_key, "secret");
        assert_eq!(request.timeout, Duration::from_secs(45));
        assert_eq!(
            request.request.to_provider_payload()["prompt"],
            "draw a support diagram"
        );
        assert_eq!(request.request.to_provider_payload()["size"], "1024x1024");
        assert_eq!(request.request.to_provider_payload()["n"], 2);
    }

    #[test]
    fn media_image_parser_dependency_maps_provider_asset_payload() {
        let provider_payload = json!({
            "id": "asset_123",
            "data": [{"url": "https://cdn.example/image.png"}]
        });

        let result = parse_media_image_generation_response(&provider_payload)
            .expect("provider payload should expose an image URL");

        assert_eq!(result.asset_url, "https://cdn.example/image.png");
        assert_eq!(result.provider_asset_id.as_deref(), Some("asset_123"));
    }

    #[test]
    fn bad_response_error_preserves_provider_message() {
        let error = ModelProviderClientError::BadResponse("Embedding 模型响应为空".to_owned());

        assert_eq!(error.to_string(), "Embedding 模型响应为空");
    }

    #[test]
    fn rag_requests_carry_provider_dispatch_inputs() {
        let texts = vec!["alpha".to_owned(), "beta".to_owned()];
        let embedding = ModelProviderEmbeddingRequest {
            endpoint: "https://provider.example/v1/embeddings",
            api_key: "secret",
            model: Some("embed-demo"),
            texts: &texts,
            timeout: Duration::from_secs(20),
        };

        assert_eq!(embedding.endpoint, "https://provider.example/v1/embeddings");
        assert_eq!(embedding.api_key, "secret");
        assert_eq!(embedding.model, Some("embed-demo"));
        assert_eq!(embedding.texts, texts.as_slice());
        assert_eq!(embedding.timeout, Duration::from_secs(20));

        let documents = vec!["doc-a".to_owned(), "doc-b".to_owned()];
        let rerank = ModelProviderRerankRequest {
            endpoint: "https://provider.example/v1/rerank",
            api_key: "secret",
            model: Some("rerank-demo"),
            query: "question",
            documents: &documents,
            timeout: Duration::from_secs(30),
        };

        assert_eq!(rerank.endpoint, "https://provider.example/v1/rerank");
        assert_eq!(rerank.api_key, "secret");
        assert_eq!(rerank.model, Some("rerank-demo"));
        assert_eq!(rerank.query, "question");
        assert_eq!(rerank.documents, documents.as_slice());
        assert_eq!(rerank.timeout, Duration::from_secs(30));
    }

    #[test]
    fn rerank_parser_maps_dashscope_result_shapes() {
        let body = json!({
            "results": [
                {"document_index": "2", "relevance_score": "0.91"},
                {"documentIndex": 0, "score": 0.75},
                {"index": 3, "relevanceScore": "nan"},
                {"index": "bad", "score": 0.5}
            ]
        });

        let scores = parse_model_provider_rerank_scores(&body);

        assert_eq!(scores.len(), 2);
        assert_eq!(scores[0].index, 2);
        assert!((scores[0].score - 0.91).abs() < 0.000_001);
        assert_eq!(scores[1].index, 0);
        assert!((scores[1].score - 0.75).abs() < 0.000_001);
    }

    #[test]
    fn embedding_parser_maps_openai_compatible_vectors() {
        let body = json!({
            "data": [
                {"index": 1, "embedding": [0.1, "-0.2", 0.3]},
                {"embedding": ["0.4", "bad", 0.6]},
                {"index": 3, "embedding": ["nan"]},
                {"index": 4, "embedding": []}
            ]
        });

        let vectors = parse_model_provider_embedding_vectors(&body);

        assert_eq!(vectors.len(), 2);
        assert_eq!(vectors[0].index, 1);
        assert_eq!(vectors[0].vector, vec![0.1, -0.2, 0.3]);
        assert_eq!(vectors[1].index, 0);
        assert_eq!(vectors[1].vector, vec![0.4, 0.6]);
    }

    fn test_provider_compaction_metadata(implementation: &str) -> ModelProviderChatRequestMetadata {
        ModelProviderChatRequestMetadata {
            request_kind: ModelProviderChatRequestKind::Compaction,
            compaction: Some(ModelProviderChatCompactionMetadata {
                implementation: implementation.to_owned(),
                trigger: "observation_threshold".to_owned(),
                reason: "observation_threshold".to_owned(),
                phase: "model_loop_follow_up".to_owned(),
                strategy: "model_generated_summary".to_owned(),
                window_id: 1,
                input_history_count: 2,
                retained_history_count: 1,
                compacted_item_count: 1,
                retained_item_count: 1,
                tool_codes: vec!["rag.search".to_owned()],
            }),
        }
    }
}
