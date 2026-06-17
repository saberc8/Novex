use std::time::Duration;

use novex_model::ModelMediaImageGenerationResp;
use novex_tools::{parse_media_image_generation_response, MediaImageGenerationRequest};
use serde_json::{json, Value};

use crate::{model_provider_http_client, ModelProviderClientError};

pub struct ModelProviderMediaImageRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub request: &'a MediaImageGenerationRequest,
    pub timeout: Duration,
}

pub async fn send_model_provider_media_image_request(
    request: ModelProviderMediaImageRequest<'_>,
) -> Result<ModelMediaImageGenerationResp, ModelProviderClientError> {
    let request_payload = request.request.to_provider_payload();
    let client = model_provider_http_client(request.timeout).map_err(|err| {
        ModelProviderClientError::BadResponse(format!("图片生成客户端初始化失败: {err}"))
    })?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .header("x-api-key", request.api_key)
        .json(&request_payload)
        .send()
        .await
        .map_err(|err| ModelProviderClientError::BadResponse(format!("图片生成请求失败: {err}")))?;
    let status = response.status();
    let provider_payload = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !status.is_success() {
        return Err(ModelProviderClientError::BadResponse(format!(
            "图片生成请求失败: HTTP {}",
            status.as_u16()
        )));
    }
    let Some(result) = parse_media_image_generation_response(&provider_payload) else {
        return Err(ModelProviderClientError::BadResponse(
            "图片生成响应缺少资产 URL".to_owned(),
        ));
    };

    Ok(ModelMediaImageGenerationResp {
        provider_payload,
        asset_url: result.asset_url,
        provider_asset_id: result.provider_asset_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn media_image_request_carries_provider_dispatch_inputs() {
        let media_request = MediaImageGenerationRequest::new("draw a support diagram")
            .with_size("1024x1024")
            .with_count(2);
        let request = ModelProviderMediaImageRequest {
            endpoint: "https://provider.example/v1/images/generations",
            api_key: "secret",
            request: &media_request,
            timeout: Duration::from_secs(45),
        };

        assert_eq!(
            request.endpoint,
            "https://provider.example/v1/images/generations"
        );
        assert_eq!(request.api_key, "secret");
        assert_eq!(request.timeout, Duration::from_secs(45));
        assert_eq!(
            request.request.to_provider_payload()["prompt"],
            "draw a support diagram"
        );
        assert_eq!(request.request.to_provider_payload()["size"], "1024x1024");
        assert_eq!(request.request.to_provider_payload()["n"], 2);
    }

    #[test]
    fn media_image_parser_dependency_maps_provider_asset_payload() {
        let provider_payload = json!({
            "id": "asset_123",
            "data": [{"url": "https://cdn.example/image.png"}]
        });

        let result = parse_media_image_generation_response(&provider_payload)
            .expect("provider payload should expose an image URL");

        assert_eq!(result.asset_url, "https://cdn.example/image.png");
        assert_eq!(result.provider_asset_id.as_deref(), Some("asset_123"));
    }
}
