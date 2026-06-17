use std::time::Duration;

use novex_model::{ModelEmbeddingVector, ModelRerankScore};
use novex_provider_client::{
    parse_model_provider_embedding_vectors, parse_model_provider_rerank_scores,
};
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
