use std::time::Duration;

use novex_model::{ModelEmbeddingVector, ModelRerankScore};
use serde_json::{json, Value};

use super::http::model_provider_http_client;
use crate::shared::error::AppError;

pub(in crate::application::ai) struct ModelProviderEmbeddingRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub model: Option<&'a str>,
    pub texts: &'a [String],
    pub timeout: Duration,
}

pub(in crate::application::ai) struct ModelProviderRerankRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub model: Option<&'a str>,
    pub query: &'a str,
    pub documents: &'a [String],
    pub timeout: Duration,
}

pub(in crate::application::ai) async fn send_model_provider_embedding_request(
    request: ModelProviderEmbeddingRequest<'_>,
) -> Result<Vec<ModelEmbeddingVector>, AppError> {
    let client =
        model_provider_http_client(request.timeout).map_err(|err| AppError::Anyhow(err.into()))?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .json(&json!({
            "model": request.model.unwrap_or_default(),
            "input": request.texts,
        }))
        .send()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(AppError::bad_request(format!(
            "Embedding 模型调用失败: {status}"
        )));
    }
    let vectors = parse_model_provider_embedding_vectors(&body);
    if vectors.is_empty() {
        return Err(AppError::bad_request("Embedding 模型响应为空"));
    }
    Ok(vectors)
}

pub(in crate::application::ai) async fn send_model_provider_rerank_request(
    request: ModelProviderRerankRequest<'_>,
) -> Result<Vec<ModelRerankScore>, AppError> {
    let client =
        model_provider_http_client(request.timeout).map_err(|err| AppError::Anyhow(err.into()))?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .json(&json!({
            "model": request.model.unwrap_or_default(),
            "query": request.query,
            "documents": request.documents,
        }))
        .send()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(AppError::bad_request(format!(
            "Rerank 模型调用失败: {status}"
        )));
    }
    let scores = parse_model_provider_rerank_scores(&body);
    if scores.is_empty() {
        return Err(AppError::bad_request("Rerank 模型响应为空"));
    }
    Ok(scores)
}

pub(in crate::application::ai) fn parse_model_provider_rerank_scores(
    body: &Value,
) -> Vec<ModelRerankScore> {
    body.get("results")
        .or_else(|| body.get("data"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_rerank_score)
        .collect()
}

pub(in crate::application::ai) fn parse_model_provider_embedding_vectors(
    body: &Value,
) -> Vec<ModelEmbeddingVector> {
    body.get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_embedding_vector)
        .collect()
}

fn parse_rerank_score(value: &Value) -> Option<ModelRerankScore> {
    let index = value
        .get("index")
        .and_then(json_usize)
        .or_else(|| value.get("document_index").and_then(json_usize))
        .or_else(|| value.get("documentIndex").and_then(json_usize))?;
    let score = value
        .get("relevance_score")
        .or_else(|| value.get("relevanceScore"))
        .or_else(|| value.get("score"))
        .and_then(json_f32)?;
    if !score.is_finite() {
        return None;
    }
    Some(ModelRerankScore { index, score })
}

fn parse_embedding_vector(value: &Value) -> Option<ModelEmbeddingVector> {
    let index = value.get("index").and_then(json_usize).unwrap_or(0);
    let vector = value
        .get("embedding")?
        .as_array()?
        .iter()
        .filter_map(json_f32)
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if vector.is_empty() {
        return None;
    }
    Some(ModelEmbeddingVector { index, vector })
}

fn json_usize(value: &Value) -> Option<usize> {
    if let Some(value) = value.as_u64() {
        return usize::try_from(value).ok();
    }
    value
        .as_str()
        .and_then(|text| text.trim().parse::<usize>().ok())
}

fn json_f32(value: &Value) -> Option<f32> {
    if let Some(value) = value.as_f64() {
        return Some(value as f32);
    }
    value
        .as_str()
        .and_then(|text| text.trim().parse::<f32>().ok())
}
