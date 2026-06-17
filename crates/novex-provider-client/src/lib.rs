use std::{error::Error, fmt, time::Duration};

use novex_model::{ModelEmbeddingVector, ModelRerankScore};
use serde_json::{json, Value};

pub const CRATE_ID: &str = "novex-provider-client";

#[derive(Debug)]
pub enum ModelProviderClientError {
    Transport(reqwest::Error),
    HttpStatus {
        failure_message: String,
        status: u16,
    },
    BadResponse(String),
}

impl fmt::Display for ModelProviderClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(err) => write!(f, "{err}"),
            Self::HttpStatus {
                failure_message,
                status,
            } => write!(f, "{failure_message}: HTTP {status}"),
            Self::BadResponse(message) => write!(f, "{message}"),
        }
    }
}

impl Error for ModelProviderClientError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport(err) => Some(err),
            Self::HttpStatus { .. } | Self::BadResponse(_) => None,
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

pub struct ModelProviderNativeCancelRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub timeout: Duration,
}

pub async fn send_model_provider_native_cancel_request(
    request: ModelProviderNativeCancelRequest<'_>,
) -> Result<u16, ModelProviderClientError> {
    let client =
        model_provider_http_client(request.timeout).map_err(ModelProviderClientError::Transport)?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .send()
        .await
        .map_err(ModelProviderClientError::Transport)?;
    let status = response.status();

    if !status.is_success() {
        return Err(ModelProviderClientError::HttpStatus {
            failure_message: "Provider native cancel failed".to_owned(),
            status: status.as_u16(),
        });
    }

    Ok(status.as_u16())
}

pub struct ModelProviderEmbeddingRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub model: Option<&'a str>,
    pub texts: &'a [String],
    pub timeout: Duration,
}

pub struct ModelProviderRerankRequest<'a> {
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub model: Option<&'a str>,
    pub query: &'a str,
    pub documents: &'a [String],
    pub timeout: Duration,
}

pub async fn send_model_provider_embedding_request(
    request: ModelProviderEmbeddingRequest<'_>,
) -> Result<Vec<ModelEmbeddingVector>, ModelProviderClientError> {
    let client =
        model_provider_http_client(request.timeout).map_err(ModelProviderClientError::Transport)?;
    let response = client
        .post(request.endpoint)
        .bearer_auth(request.api_key)
        .json(&json!({
            "model": request.model.unwrap_or_default(),
            "input": request.texts,
        }))
        .send()
        .await
        .map_err(ModelProviderClientError::Transport)?;
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(ModelProviderClientError::BadResponse(format!(
            "Embedding 模型调用失败: {status}"
        )));
    }
    let vectors = parse_model_provider_embedding_vectors(&body);
    if vectors.is_empty() {
        return Err(ModelProviderClientError::BadResponse(
            "Embedding 模型响应为空".to_owned(),
        ));
    }
    Ok(vectors)
}

pub async fn send_model_provider_rerank_request(
    request: ModelProviderRerankRequest<'_>,
) -> Result<Vec<ModelRerankScore>, ModelProviderClientError> {
    let client =
        model_provider_http_client(request.timeout).map_err(ModelProviderClientError::Transport)?;
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
        .map_err(ModelProviderClientError::Transport)?;
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(ModelProviderClientError::BadResponse(format!(
            "Rerank 模型调用失败: {status}"
        )));
    }
    let scores = parse_model_provider_rerank_scores(&body);
    if scores.is_empty() {
        return Err(ModelProviderClientError::BadResponse(
            "Rerank 模型响应为空".to_owned(),
        ));
    }
    Ok(scores)
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
    fn native_cancel_request_carries_provider_dispatch_inputs() {
        let request = ModelProviderNativeCancelRequest {
            endpoint: "https://provider.example/v1/responses/resp_123/cancel",
            api_key: "secret",
            timeout: Duration::from_secs(8),
        };

        assert_eq!(
            request.endpoint,
            "https://provider.example/v1/responses/resp_123/cancel"
        );
        assert_eq!(request.api_key, "secret");
        assert_eq!(request.timeout, Duration::from_secs(8));
    }

    #[test]
    fn native_cancel_http_status_error_preserves_backend_message_shape() {
        let error = ModelProviderClientError::HttpStatus {
            failure_message: "Provider native cancel failed".to_owned(),
            status: 409,
        };

        assert_eq!(error.to_string(), "Provider native cancel failed: HTTP 409");
    }

    #[test]
    fn bad_response_error_preserves_provider_message() {
        let error = ModelProviderClientError::BadResponse("Embedding 模型响应为空".to_owned());

        assert_eq!(error.to_string(), "Embedding 模型响应为空");
    }

    #[test]
    fn rag_requests_carry_provider_dispatch_inputs() {
        let texts = vec!["alpha".to_owned(), "beta".to_owned()];
        let embedding = ModelProviderEmbeddingRequest {
            endpoint: "https://provider.example/v1/embeddings",
            api_key: "secret",
            model: Some("embed-demo"),
            texts: &texts,
            timeout: Duration::from_secs(20),
        };

        assert_eq!(embedding.endpoint, "https://provider.example/v1/embeddings");
        assert_eq!(embedding.api_key, "secret");
        assert_eq!(embedding.model, Some("embed-demo"));
        assert_eq!(embedding.texts, texts.as_slice());
        assert_eq!(embedding.timeout, Duration::from_secs(20));

        let documents = vec!["doc-a".to_owned(), "doc-b".to_owned()];
        let rerank = ModelProviderRerankRequest {
            endpoint: "https://provider.example/v1/rerank",
            api_key: "secret",
            model: Some("rerank-demo"),
            query: "question",
            documents: &documents,
            timeout: Duration::from_secs(30),
        };

        assert_eq!(rerank.endpoint, "https://provider.example/v1/rerank");
        assert_eq!(rerank.api_key, "secret");
        assert_eq!(rerank.model, Some("rerank-demo"));
        assert_eq!(rerank.query, "question");
        assert_eq!(rerank.documents, documents.as_slice());
        assert_eq!(rerank.timeout, Duration::from_secs(30));
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
