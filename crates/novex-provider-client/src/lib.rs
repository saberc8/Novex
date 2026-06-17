use std::{error::Error, fmt, time::Duration};

use novex_model::{ModelEmbeddingVector, ModelRerankScore};
use serde_json::Value;

pub const CRATE_ID: &str = "novex-provider-client";

#[derive(Debug)]
pub enum ModelProviderClientError {
    Transport(reqwest::Error),
    HttpStatus {
        failure_message: String,
        status: u16,
    },
}

impl fmt::Display for ModelProviderClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(err) => write!(f, "{err}"),
            Self::HttpStatus {
                failure_message,
                status,
            } => write!(f, "{failure_message}: HTTP {status}"),
        }
    }
}

impl Error for ModelProviderClientError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport(err) => Some(err),
            Self::HttpStatus { .. } => None,
        }
    }
}

pub struct ModelProviderHttpRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub payload: &'a Value,
    pub timeout: Duration,
    pub failure_message: &'a str,
}

pub fn model_provider_http_client(timeout: Duration) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder().timeout(timeout).build()
}

pub async fn send_model_provider_http_request(
    request: ModelProviderHttpRequest<'_>,
) -> Result<reqwest::Response, ModelProviderClientError> {
    let client =
        model_provider_http_client(request.timeout).map_err(ModelProviderClientError::Transport)?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .json(request.payload)
        .send()
        .await
        .map_err(ModelProviderClientError::Transport)?;
    let status = response.status();

    if !status.is_success() {
        return Err(ModelProviderClientError::HttpStatus {
            failure_message: request.failure_message.to_owned(),
            status: status.as_u16(),
        });
    }

    Ok(response)
}

pub fn parse_model_provider_rerank_scores(body: &Value) -> Vec<ModelRerankScore> {
    body.get("results")
        .or_else(|| body.get("data"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_rerank_score)
        .collect()
}

pub fn parse_model_provider_embedding_vectors(body: &Value) -> Vec<ModelEmbeddingVector> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn module_describes_provider_client_boundary() {
        assert_eq!(CRATE_ID, "novex-provider-client");
    }

    #[test]
    fn http_status_error_preserves_backend_message_shape() {
        let error = ModelProviderClientError::HttpStatus {
            failure_message: "LLM 模型调用失败".to_owned(),
            status: 429,
        };

        assert_eq!(error.to_string(), "LLM 模型调用失败: HTTP 429");
    }

    #[test]
    fn http_request_carries_provider_post_inputs() {
        let payload = json!({"model": "demo", "input": "hello"});
        let request = ModelProviderHttpRequest {
            endpoint: "https://provider.example/v1/chat/completions",
            api_key: "secret",
            payload: &payload,
            timeout: Duration::from_secs(15),
            failure_message: "LLM 模型调用失败",
        };

        assert_eq!(
            request.endpoint,
            "https://provider.example/v1/chat/completions"
        );
        assert_eq!(request.api_key, "secret");
        assert_eq!(request.payload["model"], "demo");
        assert_eq!(request.timeout, Duration::from_secs(15));
        assert_eq!(request.failure_message, "LLM 模型调用失败");
    }

    #[test]
    fn rerank_parser_maps_dashscope_result_shapes() {
        let body = json!({
            "results": [
                {"document_index": "2", "relevance_score": "0.91"},
                {"documentIndex": 0, "score": 0.75},
                {"index": 3, "relevanceScore": "nan"},
                {"index": "bad", "score": 0.5}
            ]
        });

        let scores = parse_model_provider_rerank_scores(&body);

        assert_eq!(scores.len(), 2);
        assert_eq!(scores[0].index, 2);
        assert!((scores[0].score - 0.91).abs() < 0.000_001);
        assert_eq!(scores[1].index, 0);
        assert!((scores[1].score - 0.75).abs() < 0.000_001);
    }

    #[test]
    fn embedding_parser_maps_openai_compatible_vectors() {
        let body = json!({
            "data": [
                {"index": 1, "embedding": [0.1, "-0.2", 0.3]},
                {"embedding": ["0.4", "bad", 0.6]},
                {"index": 3, "embedding": ["nan"]},
                {"index": 4, "embedding": []}
            ]
        });

        let vectors = parse_model_provider_embedding_vectors(&body);

        assert_eq!(vectors.len(), 2);
        assert_eq!(vectors[0].index, 1);
        assert_eq!(vectors[0].vector, vec![0.1, -0.2, 0.3]);
        assert_eq!(vectors[1].index, 0);
        assert_eq!(vectors[1].vector, vec![0.4, 0.6]);
    }
}
