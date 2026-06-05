use std::time::{Duration, Instant};

use novex_model::{
    mask_api_key, ModelRuntimeConfig, ModelRuntimeRoute, ModelRuntimeSummary, ModelRuntimeTarget,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::shared::error::AppError;

const MODEL_HEALTH_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Default)]
pub struct ModelRuntimeService;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelHealthCheckCommand {
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelHealthCheckResp {
    pub results: Vec<ModelHealthCheckResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelHealthCheckResult {
    pub target: ModelRuntimeTarget,
    pub configured: bool,
    pub ok: bool,
    pub endpoint: Option<String>,
    pub masked_api_key: Option<String>,
    pub http_status: Option<u16>,
    pub latency_ms: u128,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<Value>,
}

impl ModelRuntimeService {
    pub fn runtime_config_summary(config: ModelRuntimeConfig) -> ModelRuntimeSummary {
        config.summary()
    }

    pub fn runtime_config() -> ModelRuntimeSummary {
        Self::runtime_config_summary(ModelRuntimeConfig::from_env())
    }

    pub async fn health_check(
        command: ModelHealthCheckCommand,
    ) -> Result<ModelHealthCheckResp, AppError> {
        let targets = health_check_targets(command.target.as_deref())?;
        let config = ModelRuntimeConfig::from_env();
        let client = reqwest::Client::builder()
            .timeout(MODEL_HEALTH_TIMEOUT)
            .build()
            .map_err(|err| AppError::Anyhow(err.into()))?;

        let mut results = Vec::with_capacity(targets.len());
        for target in targets {
            results.push(check_target(&client, &config, target).await);
        }

        Ok(ModelHealthCheckResp { results })
    }
}

fn health_check_targets(target: Option<&str>) -> Result<Vec<ModelRuntimeTarget>, AppError> {
    let target = target.unwrap_or("all").trim();
    if target.is_empty() || target.eq_ignore_ascii_case("all") {
        return Ok(ModelRuntimeTarget::all().to_vec());
    }

    ModelRuntimeTarget::parse(target)
        .map(|target| vec![target])
        .ok_or_else(|| AppError::bad_request("未知模型健康检查目标"))
}

async fn check_target(
    client: &reqwest::Client,
    config: &ModelRuntimeConfig,
    target: ModelRuntimeTarget,
) -> ModelHealthCheckResult {
    let Some(route) = config.route(target) else {
        return ModelHealthCheckResult {
            target,
            configured: false,
            ok: false,
            endpoint: None,
            masked_api_key: None,
            http_status: None,
            latency_ms: 0,
            message: "未配置完整环境变量".to_owned(),
            detail: Some(json!({ "missingEnv": config.missing_env() })),
        };
    };

    let started = Instant::now();
    let checked = match target {
        ModelRuntimeTarget::Llm => check_llm(client, route).await,
        ModelRuntimeTarget::Embedding => check_embedding(client, route).await,
        ModelRuntimeTarget::Reranker => check_reranker(client, route).await,
        ModelRuntimeTarget::Draw => check_draw(client, route).await,
    };
    let latency_ms = started.elapsed().as_millis();

    match checked {
        Ok((status, ok, message, detail)) => ModelHealthCheckResult {
            target,
            configured: true,
            ok,
            endpoint: Some(route.endpoint().to_owned()),
            masked_api_key: Some(mask_api_key(route.api_key())),
            http_status: Some(status.as_u16()),
            latency_ms,
            message,
            detail,
        },
        Err(err) => ModelHealthCheckResult {
            target,
            configured: true,
            ok: false,
            endpoint: Some(route.endpoint().to_owned()),
            masked_api_key: Some(mask_api_key(route.api_key())),
            http_status: None,
            latency_ms,
            message: sanitize_error_message(&err.to_string(), route),
            detail: None,
        },
    }
}

async fn check_llm(
    client: &reqwest::Client,
    route: &ModelRuntimeRoute,
) -> Result<(reqwest::StatusCode, bool, String, Option<Value>), reqwest::Error> {
    let response = client
        .post(route.endpoint())
        .bearer_auth(route.api_key())
        .json(&json!({
            "model": route.model().unwrap_or_default(),
            "messages": [
                { "role": "user", "content": "Reply with OK." }
            ],
            "max_tokens": 128,
            "temperature": 0
        }))
        .send()
        .await?;
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    let choice_count = body
        .get("choices")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let ok = status.is_success() && choice_count > 0;

    Ok((
        status,
        ok,
        health_message(status, ok),
        Some(json!({ "choiceCount": choice_count })),
    ))
}

async fn check_embedding(
    client: &reqwest::Client,
    route: &ModelRuntimeRoute,
) -> Result<(reqwest::StatusCode, bool, String, Option<Value>), reqwest::Error> {
    let response = client
        .post(route.endpoint())
        .bearer_auth(route.api_key())
        .json(&json!({
            "model": route.model().unwrap_or_default(),
            "input": ["hello"]
        }))
        .send()
        .await?;
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    let dimensions = body
        .pointer("/data/0/embedding")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let ok = status.is_success() && dimensions > 0;

    Ok((
        status,
        ok,
        health_message(status, ok),
        Some(json!({ "dimensions": dimensions })),
    ))
}

async fn check_reranker(
    client: &reqwest::Client,
    route: &ModelRuntimeRoute,
) -> Result<(reqwest::StatusCode, bool, String, Option<Value>), reqwest::Error> {
    let response = client
        .post(route.endpoint())
        .bearer_auth(route.api_key())
        .json(&json!({
            "model": route.model().unwrap_or_default(),
            "query": "hello",
            "documents": ["hello world", "goodbye"]
        }))
        .send()
        .await?;
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    let result_count = body
        .get("results")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let ok = status.is_success() && result_count > 0;

    Ok((
        status,
        ok,
        health_message(status, ok),
        Some(json!({ "resultCount": result_count })),
    ))
}

async fn check_draw(
    client: &reqwest::Client,
    route: &ModelRuntimeRoute,
) -> Result<(reqwest::StatusCode, bool, String, Option<Value>), reqwest::Error> {
    let response = client
        .get(route.endpoint())
        .bearer_auth(route.api_key())
        .header("x-api-key", route.api_key())
        .send()
        .await?;
    let status = response.status();
    let ok = status.is_success() || status.is_redirection();

    Ok((
        status,
        ok,
        health_message(status, ok),
        Some(json!({ "authenticatedReachability": ok })),
    ))
}

fn health_message(status: reqwest::StatusCode, ok: bool) -> String {
    if ok {
        "ok".to_owned()
    } else {
        format!("provider returned HTTP {}", status.as_u16())
    }
}

fn sanitize_error_message(message: &str, route: &ModelRuntimeRoute) -> String {
    message.replace(route.api_key(), &mask_api_key(route.api_key()))
}

#[cfg(test)]
mod tests {
    use novex_model::{ModelRuntimeConfig, ModelRuntimeTarget};

    use super::*;

    #[test]
    fn runtime_config_summary_masks_keys_and_reports_routes() {
        let config = ModelRuntimeConfig::from_env_map(|key| match key {
            "LLM_API_KEY" => Some("sk-fake-llm-secret-508d".to_owned()),
            "LLM_BASE_URL" => Some("https://api.deepseek.com".to_owned()),
            "LLM_MODEL" => Some("deepseek-v4-flash".to_owned()),
            "EMBEDDING_API_KEY" => Some("sk-fake-embedding-secret-ffff".to_owned()),
            "EMBEDDING_BASE_URL" => {
                Some("https://dashscope.aliyuncs.com/compatible-mode/v1".to_owned())
            }
            "EMBEDDING_MODEL" => Some("text-embedding-v4".to_owned()),
            "RERANKER_API_KEY" => Some("sk-fake-reranker-secret-ffff".to_owned()),
            "RERANKER_BASE_URL" => {
                Some("https://dashscope.aliyuncs.com/compatible-api/v1".to_owned())
            }
            "RERANKER_MODEL" => Some("qwen3-rerank".to_owned()),
            "RIGHT_CODE_DRAW_API_KEY" => Some("sk-fake-draw-secret-2064".to_owned()),
            "RIGHT_CODE_DRAW_BASE_URL" => Some("https://www.right.codes/draw".to_owned()),
            _ => None,
        });

        let summary = ModelRuntimeService::runtime_config_summary(config);

        assert_eq!(summary.routes.len(), 4);
        assert_eq!(
            summary
                .routes
                .iter()
                .find(|route| route.target == ModelRuntimeTarget::Llm)
                .unwrap()
                .masked_api_key,
            "sk-****508d"
        );
        assert!(!format!("{summary:?}").contains("sk-fake-llm-secret-508d"));
    }
}
