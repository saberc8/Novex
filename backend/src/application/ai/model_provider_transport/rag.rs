use novex_model::{ModelEmbeddingVector, ModelRerankScore};
pub(in crate::application::ai) use novex_provider_client::{
    ModelProviderEmbeddingRequest, ModelProviderRerankRequest,
};

use super::http::model_provider_client_error_to_app_error;
use crate::shared::error::AppError;

pub(in crate::application::ai) async fn send_model_provider_embedding_request(
    request: ModelProviderEmbeddingRequest<'_>,
) -> Result<Vec<ModelEmbeddingVector>, AppError> {
    novex_provider_client::send_model_provider_embedding_request(request)
        .await
        .map_err(model_provider_client_error_to_app_error)
}

pub(in crate::application::ai) async fn send_model_provider_rerank_request(
    request: ModelProviderRerankRequest<'_>,
) -> Result<Vec<ModelRerankScore>, AppError> {
    novex_provider_client::send_model_provider_rerank_request(request)
        .await
        .map_err(model_provider_client_error_to_app_error)
}
