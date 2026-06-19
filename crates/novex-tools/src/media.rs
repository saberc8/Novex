use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaImageGenerationRequest {
    pub prompt: String,
    pub size: Option<String>,
    pub count: usize,
}

impl MediaImageGenerationRequest {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into().trim().to_owned(),
            size: None,
            count: 1,
        }
    }

    pub fn with_size(mut self, size: impl Into<String>) -> Self {
        let size = size.into().trim().to_owned();
        if !size.is_empty() {
            self.size = Some(size);
        }
        self
    }

    pub fn with_count(mut self, count: usize) -> Self {
        self.count = count.max(1);
        self
    }

    pub fn to_provider_payload(&self) -> Value {
        let mut payload = json!({
            "prompt": self.prompt,
            "n": self.count,
        });
        if let (Some(object), Some(size)) = (payload.as_object_mut(), self.size.as_deref()) {
            object.insert("size".to_owned(), json!(size));
        }
        payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaImageGenerationResult {
    pub asset_url: String,
    pub provider_asset_id: Option<String>,
}

pub fn parse_media_image_generation_response(value: &Value) -> Option<MediaImageGenerationResult> {
    let asset_url = media_image_url(value)?.trim().to_owned();
    if asset_url.is_empty() {
        return None;
    }
    Some(MediaImageGenerationResult {
        asset_url,
        provider_asset_id: media_provider_asset_id(value),
    })
}

fn media_image_url(value: &Value) -> Option<&str> {
    value
        .get("imageUrl")
        .or_else(|| value.get("image_url"))
        .or_else(|| value.get("assetUrl"))
        .or_else(|| value.get("asset_url"))
        .or_else(|| value.get("url"))
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("data")?
                .as_array()?
                .first()?
                .get("url")
                .and_then(Value::as_str)
        })
}

fn media_provider_asset_id(value: &Value) -> Option<String> {
    value
        .get("id")
        .or_else(|| value.get("assetId"))
        .or_else(|| value.get("asset_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
