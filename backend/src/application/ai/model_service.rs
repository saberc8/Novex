use std::time::{Duration, Instant};

use novex_model::{
    mask_api_key, ModelRuntimeConfig, ModelRuntimeRoute, ModelRuntimeSummary, ModelRuntimeTarget,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgPool};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRegistrySummary {
    pub provider_count: usize,
    pub deployment_count: usize,
    pub profile_count: usize,
    pub route_count: usize,
    pub providers: Vec<ModelProviderRegistryResp>,
    pub deployments: Vec<ModelDeploymentRegistryResp>,
    pub profiles: Vec<ModelProfileRegistryResp>,
    pub routes: Vec<ModelRouteRegistryResp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProviderRegistryResp {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub provider_type: String,
    pub status: i16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDeploymentRegistryResp {
    pub id: i64,
    pub provider_id: i64,
    pub code: String,
    pub name: String,
    pub endpoint: String,
    pub network_zone: String,
    pub status: i16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfileRegistryResp {
    pub id: i64,
    pub deployment_id: i64,
    pub code: String,
    pub name: String,
    pub model_name: String,
    pub model_kind: String,
    pub status: i16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRouteRegistryResp {
    pub id: i64,
    pub code: String,
    pub route_purpose: String,
    pub model_profile_id: i64,
    pub priority: i32,
    pub status: i16,
    pub masked_credential: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct ModelProviderRegistryRow {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub provider_type: String,
    pub status: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct ModelDeploymentRegistryRow {
    pub id: i64,
    pub provider_id: i64,
    pub code: String,
    pub name: String,
    pub endpoint: String,
    pub network_zone: String,
    pub status: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct ModelProfileRegistryRow {
    pub id: i64,
    pub deployment_id: i64,
    pub code: String,
    pub name: String,
    pub model_name: String,
    pub model_kind: String,
    pub status: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct ModelRouteRegistryRow {
    pub id: i64,
    pub code: String,
    pub route_purpose: String,
    pub model_profile_id: i64,
    pub priority: i32,
    pub status: i16,
    pub credential_ref: Option<String>,
    pub masked_value: Option<String>,
}

impl ModelRuntimeService {
    pub fn runtime_config_summary(config: ModelRuntimeConfig) -> ModelRuntimeSummary {
        config.summary()
    }

    pub fn runtime_config() -> ModelRuntimeSummary {
        Self::runtime_config_summary(ModelRuntimeConfig::from_env())
    }

    pub async fn registry_summary(db: &PgPool) -> Result<ModelRegistrySummary, AppError> {
        let providers = sqlx::query_as::<_, ModelProviderRegistryRow>(
            r#"
SELECT id, code, name, provider_type, status
FROM ai_model_provider
WHERE tenant_id = 1
ORDER BY id
"#,
        )
        .fetch_all(db)
        .await?;
        let deployments = sqlx::query_as::<_, ModelDeploymentRegistryRow>(
            r#"
SELECT id, provider_id, code, name, endpoint, network_zone, status
FROM ai_model_deployment
WHERE tenant_id = 1
ORDER BY id
"#,
        )
        .fetch_all(db)
        .await?;
        let profiles = sqlx::query_as::<_, ModelProfileRegistryRow>(
            r#"
SELECT id, deployment_id, code, name, model_name, model_kind, status
FROM ai_model_profile
WHERE tenant_id = 1
ORDER BY id
"#,
        )
        .fetch_all(db)
        .await?;
        let routes = sqlx::query_as::<_, ModelRouteRegistryRow>(
            r#"
SELECT
    r.id,
    r.code,
    r.route_purpose,
    r.model_profile_id,
    r.priority,
    r.status,
    c.credential_ref,
    c.masked_value
FROM ai_model_route r
LEFT JOIN ai_model_credential c ON c.id = r.credential_id
WHERE r.tenant_id = 1
ORDER BY r.priority, r.id
"#,
        )
        .fetch_all(db)
        .await?;

        Ok(Self::registry_summary_from_rows(
            providers,
            deployments,
            profiles,
            routes,
        ))
    }

    pub fn registry_summary_from_rows(
        providers: Vec<ModelProviderRegistryRow>,
        deployments: Vec<ModelDeploymentRegistryRow>,
        profiles: Vec<ModelProfileRegistryRow>,
        routes: Vec<ModelRouteRegistryRow>,
    ) -> ModelRegistrySummary {
        ModelRegistrySummary {
            provider_count: providers.len(),
            deployment_count: deployments.len(),
            profile_count: profiles.len(),
            route_count: routes.len(),
            providers: providers
                .into_iter()
                .map(|row| ModelProviderRegistryResp {
                    id: row.id,
                    code: row.code,
                    name: row.name,
                    provider_type: row.provider_type,
                    status: row.status,
                })
                .collect(),
            deployments: deployments
                .into_iter()
                .map(|row| ModelDeploymentRegistryResp {
                    id: row.id,
                    provider_id: row.provider_id,
                    code: row.code,
                    name: row.name,
                    endpoint: row.endpoint,
                    network_zone: row.network_zone,
                    status: row.status,
                })
                .collect(),
            profiles: profiles
                .into_iter()
                .map(|row| ModelProfileRegistryResp {
                    id: row.id,
                    deployment_id: row.deployment_id,
                    code: row.code,
                    name: row.name,
                    model_name: row.model_name,
                    model_kind: row.model_kind,
                    status: row.status,
                })
                .collect(),
            routes: routes
                .into_iter()
                .map(|row| ModelRouteRegistryResp {
                    id: row.id,
                    code: row.code,
                    route_purpose: row.route_purpose,
                    model_profile_id: row.model_profile_id,
                    priority: row.priority,
                    status: row.status,
                    masked_credential: row.masked_value,
                })
                .collect(),
        }
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

    #[test]
    fn model_registry_summary_does_not_expose_raw_secret_references() {
        let summary = ModelRuntimeService::registry_summary_from_rows(
            vec![ModelProviderRegistryRow {
                id: 1,
                code: "deepseek".to_owned(),
                name: "DeepSeek".to_owned(),
                provider_type: "deep-seek".to_owned(),
                status: 1,
            }],
            vec![ModelDeploymentRegistryRow {
                id: 10,
                provider_id: 1,
                code: "deepseek-public".to_owned(),
                name: "DeepSeek Public API".to_owned(),
                endpoint: "https://api.deepseek.com".to_owned(),
                network_zone: "public".to_owned(),
                status: 1,
            }],
            vec![ModelProfileRegistryRow {
                id: 20,
                deployment_id: 10,
                code: "deepseek-v4-flash".to_owned(),
                name: "DeepSeek V4 Flash".to_owned(),
                model_name: "deepseek-v4-flash".to_owned(),
                model_kind: "llm".to_owned(),
                status: 1,
            }],
            vec![ModelRouteRegistryRow {
                id: 30,
                code: "runtime.llm.chat".to_owned(),
                route_purpose: "chat".to_owned(),
                model_profile_id: 20,
                priority: 100,
                status: 1,
                credential_ref: Some("env:LLM_API_KEY".to_owned()),
                masked_value: Some("sk-****508d".to_owned()),
            }],
        );

        assert_eq!(summary.provider_count, 1);
        assert_eq!(summary.route_count, 1);
        assert_eq!(
            summary.routes[0].masked_credential.as_deref(),
            Some("sk-****508d")
        );
        let debug = format!("{summary:?}");
        assert!(!debug.contains("LLM_API_KEY"));
        assert!(!debug.contains("env:"));
    }
}
