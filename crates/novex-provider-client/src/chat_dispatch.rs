use std::time::Duration;

use serde_json::Value;

use crate::{
    read_model_provider_response_text, send_model_provider_http_request, ModelProviderClientError,
    ModelProviderHttpRequest,
};

pub struct ModelProviderChatRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub payload: &'a Value,
    pub timeout: Duration,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
