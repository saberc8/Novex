use std::time::Duration;

use super::http::model_provider_http_client;
use crate::shared::error::AppError;

pub(in crate::application::ai) struct ModelProviderNativeCancelRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub timeout: Duration,
}

pub(in crate::application::ai) async fn send_model_provider_native_cancel_request(
    request: ModelProviderNativeCancelRequest<'_>,
) -> Result<u16, AppError> {
    let client =
        model_provider_http_client(request.timeout).map_err(|err| AppError::Anyhow(err.into()))?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .send()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let status = response.status();

    if !status.is_success() {
        return Err(AppError::bad_request(format!(
            "Provider native cancel failed: HTTP {}",
            status.as_u16()
        )));
    }

    Ok(status.as_u16())
}
