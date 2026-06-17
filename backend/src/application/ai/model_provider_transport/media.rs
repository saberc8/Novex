use novex_model::ModelMediaImageGenerationResp;
pub(in crate::application::ai) use novex_provider_client::ModelProviderMediaImageRequest;

use super::http::model_provider_client_error_to_app_error;
use crate::shared::error::AppError;

pub(in crate::application::ai) async fn send_model_provider_media_image_request(
    request: ModelProviderMediaImageRequest<'_>,
) -> Result<ModelMediaImageGenerationResp, AppError> {
    novex_provider_client::send_model_provider_media_image_request(request)
        .await
        .map_err(model_provider_client_error_to_app_error)
}
