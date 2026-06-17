use std::time::Duration;

use serde_json::Value;

use crate::ModelProviderClientError;

pub struct ModelProviderHttpRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub payload: &'a Value,
    pub timeout: Duration,
    pub failure_message: &'a str,
}

pub fn model_provider_http_client(timeout: Duration) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder().timeout(timeout).build()
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

pub async fn read_model_provider_response_text(
    response: reqwest::Response,
) -> Result<String, ModelProviderClientError> {
    response
        .text()
        .await
        .map_err(ModelProviderClientError::Transport)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
