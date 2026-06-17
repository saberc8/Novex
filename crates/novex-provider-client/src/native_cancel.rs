use std::time::Duration;

use crate::{model_provider_http_client, ModelProviderClientError};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
