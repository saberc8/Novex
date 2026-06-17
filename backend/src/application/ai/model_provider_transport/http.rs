pub(super) use novex_provider_client::model_provider_http_client;
pub(in crate::application::ai) use novex_provider_client::ModelProviderHttpRequest;
use novex_provider_client::{self, ModelProviderClientError};

use crate::shared::error::AppError;

pub(in crate::application::ai) async fn send_model_provider_http_request(
    request: ModelProviderHttpRequest<'_>,
) -> Result<reqwest::Response, AppError> {
    novex_provider_client::send_model_provider_http_request(request)
        .await
        .map_err(model_provider_client_error_to_app_error)
}

fn model_provider_client_error_to_app_error(error: ModelProviderClientError) -> AppError {
    match error {
        ModelProviderClientError::Transport(err) => AppError::Anyhow(err.into()),
        ModelProviderClientError::HttpStatus {
            failure_message,
            status,
        } => AppError::bad_request(format!("{failure_message}: HTTP {status}")),
    }
}
