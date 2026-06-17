use std::time::Duration;

use serde_json::Value;

use crate::shared::error::AppError;

pub(super) struct ModelProviderHttpRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub payload: &'a Value,
    pub timeout: Duration,
    pub failure_message: &'a str,
}

pub(super) async fn send_model_provider_http_request(
    request: ModelProviderHttpRequest<'_>,
) -> Result<reqwest::Response, AppError> {
    let client = reqwest::Client::builder()
        .timeout(request.timeout)
        .build()
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .json(request.payload)
        .send()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let status = response.status();

    if !status.is_success() {
        return Err(AppError::bad_request(format!(
            "{}: HTTP {}",
            request.failure_message,
            status.as_u16()
        )));
    }

    Ok(response)
}
