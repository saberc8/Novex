use novex_model::ModelProviderType;
use serde_json::{json, Value};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
