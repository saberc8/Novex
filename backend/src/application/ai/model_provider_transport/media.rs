use std::time::Duration;

use novex_model::ModelMediaImageGenerationResp;
use novex_tools::{parse_media_image_generation_response, MediaImageGenerationRequest};
use serde_json::{json, Value};

use super::http::model_provider_http_client;
use crate::shared::error::AppError;

pub(in crate::application::ai) struct ModelProviderMediaImageRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub request: &'a MediaImageGenerationRequest,
    pub timeout: Duration,
}

pub(in crate::application::ai) async fn send_model_provider_media_image_request(
    request: ModelProviderMediaImageRequest<'_>,
) -> Result<ModelMediaImageGenerationResp, AppError> {
    let request_payload = request.request.to_provider_payload();
    let client = model_provider_http_client(request.timeout)
        .map_err(|err| AppError::bad_request(format!("图片生成客户端初始化失败: {err}")))?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .header("x-api-key", request.api_key)
        .json(&request_payload)
        .send()
        .await
        .map_err(|err| AppError::bad_request(format!("图片生成请求失败: {err}")))?;
    let status = response.status();
    let provider_payload = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !status.is_success() {
        return Err(AppError::bad_request(format!(
            "图片生成请求失败: HTTP {}",
            status.as_u16()
        )));
    }
    let Some(result) = parse_media_image_generation_response(&provider_payload) else {
        return Err(AppError::bad_request("图片生成响应缺少资产 URL"));
    };

    Ok(ModelMediaImageGenerationResp {
        provider_payload,
        asset_url: result.asset_url,
        provider_asset_id: result.provider_asset_id,
    })
}
