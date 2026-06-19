use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProviderStreamChunk {
    pub index: usize,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_event: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMediaImageGenerationResp {
    pub provider_payload: Value,
    pub asset_url: String,
    pub provider_asset_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelRerankScore {
    pub index: usize,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelEmbeddingVector {
    pub index: usize,
    pub vector: Vec<f32>,
}
