use std::time::{Duration, Instant};

use chrono::{NaiveDateTime, Utc};
use novex_model::{
    mask_api_key, ModelRuntimeConfig, ModelRuntimeRoute, ModelRuntimeSummary, ModelRuntimeTarget,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgPool};

use crate::{application::system::ensure_max_chars, shared::error::AppError, shared::id::next_id};

const MODEL_HEALTH_TIMEOUT: Duration = Duration::from_secs(20);
const MODEL_CHAT_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_MODEL_CHAT_TEMPERATURE: f64 = 0.2;
const MAX_MODEL_CHAT_TEMPERATURE: f64 = 1.0;
const DEFAULT_MODEL_CHAT_MAX_TOKENS: u32 = 1024;
const MAX_MODEL_CHAT_MAX_TOKENS: u32 = 2048;
const MAX_MODEL_CHAT_MESSAGES: usize = 30;
const MAX_MODEL_CHAT_CONTENT_CHARS: usize = 12_000;

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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelChatCommand {
    #[serde(default)]
    pub messages: Vec<ModelChatMessage>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default, rename = "maxTokens")]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelChatMessage {
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelChatUsage {
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelChatResp {
    pub answer: String,
    pub route_id: String,
    pub model: Option<String>,
    pub latency_ms: u128,
    pub usage: ModelChatUsage,
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

#[derive(Debug, Clone)]
struct ModelUsageSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub usage_kind: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub latency_ms: Option<i64>,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
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
                    masked_credential: public_masked_credential(row.masked_value),
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

    pub async fn chat_completion(command: ModelChatCommand) -> Result<ModelChatResp, AppError> {
        execute_chat_completion(command).await
    }

    pub async fn chat_completion_with_usage(
        db: &PgPool,
        user_id: i64,
        command: ModelChatCommand,
    ) -> Result<ModelChatResp, AppError> {
        let response = execute_chat_completion(command).await?;
        let record = model_chat_usage_record(user_id, &response, Utc::now().naive_utc());
        record_model_chat_usage(db, &record).await?;
        Ok(response)
    }
}

async fn execute_chat_completion(command: ModelChatCommand) -> Result<ModelChatResp, AppError> {
    let command = normalize_model_chat_command(command)?;
    let config = ModelRuntimeConfig::from_env();
    let route = config
        .route(ModelRuntimeTarget::Llm)
        .ok_or_else(|| AppError::bad_request("LLM 模型环境变量未配置完整"))?;
    let client = reqwest::Client::builder()
        .timeout(MODEL_CHAT_TIMEOUT)
        .build()
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let payload = model_chat_request_payload(route, &command);
    let started = Instant::now();
    let response = client
        .post(route.endpoint())
        .bearer_auth(route.api_key())
        .json(&payload)
        .send()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);

    if !status.is_success() {
        return Err(AppError::bad_request(format!(
            "LLM 模型调用失败: HTTP {}",
            status.as_u16()
        )));
    }

    model_chat_response_from_provider(route, body, started.elapsed().as_millis())
}

fn normalize_model_chat_command(
    mut command: ModelChatCommand,
) -> Result<ModelChatCommand, AppError> {
    if command.messages.is_empty() {
        return Err(AppError::bad_request("至少需要一条消息"));
    }
    if command.messages.len() > MAX_MODEL_CHAT_MESSAGES {
        return Err(AppError::bad_request(format!(
            "消息数量不能超过 {MAX_MODEL_CHAT_MESSAGES}"
        )));
    }

    for message in &mut command.messages {
        message.role = message.role.trim().to_ascii_lowercase();
        message.content = message.content.trim().to_owned();
        if !matches!(message.role.as_str(), "system" | "user" | "assistant") {
            return Err(AppError::bad_request("消息角色不支持"));
        }
        if message.content.is_empty() {
            return Err(AppError::bad_request("消息内容不能为空"));
        }
        ensure_max_chars("消息内容", &message.content, MAX_MODEL_CHAT_CONTENT_CHARS)?;
    }

    command.temperature = Some(
        command
            .temperature
            .unwrap_or(DEFAULT_MODEL_CHAT_TEMPERATURE)
            .clamp(0.0, MAX_MODEL_CHAT_TEMPERATURE),
    );
    command.max_tokens = Some(
        command
            .max_tokens
            .unwrap_or(DEFAULT_MODEL_CHAT_MAX_TOKENS)
            .clamp(1, MAX_MODEL_CHAT_MAX_TOKENS),
    );

    Ok(command)
}

fn model_chat_request_payload(route: &ModelRuntimeRoute, command: &ModelChatCommand) -> Value {
    let messages = command
        .messages
        .iter()
        .map(|message| {
            json!({
                "role": message.role,
                "content": message.content,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "model": route.model().unwrap_or_default(),
        "messages": messages,
        "temperature": command.temperature.unwrap_or(DEFAULT_MODEL_CHAT_TEMPERATURE),
        "max_tokens": command.max_tokens.unwrap_or(DEFAULT_MODEL_CHAT_MAX_TOKENS),
        "stream": false,
    })
}

fn model_chat_response_from_provider(
    route: &ModelRuntimeRoute,
    body: Value,
    latency_ms: u128,
) -> Result<ModelChatResp, AppError> {
    let answer = body
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| AppError::bad_request("LLM 响应为空"))?;
    let usage = body.get("usage").unwrap_or(&Value::Null);

    Ok(ModelChatResp {
        answer,
        route_id: route.summary().route_id,
        model: route.model().map(str::to_owned),
        latency_ms,
        usage: ModelChatUsage {
            prompt_tokens: usage.get("prompt_tokens").and_then(Value::as_i64),
            completion_tokens: usage.get("completion_tokens").and_then(Value::as_i64),
            total_tokens: usage.get("total_tokens").and_then(Value::as_i64),
        },
    })
}

fn model_chat_usage_record(
    user_id: i64,
    response: &ModelChatResp,
    now: NaiveDateTime,
) -> ModelUsageSaveRecord {
    ModelUsageSaveRecord {
        id: next_id(),
        tenant_id: 1,
        usage_kind: "chat".to_owned(),
        prompt_tokens: response.usage.prompt_tokens.unwrap_or(0).max(0),
        completion_tokens: response.usage.completion_tokens.unwrap_or(0).max(0),
        total_tokens: response.usage.total_tokens.unwrap_or(0).max(0),
        latency_ms: Some(u128_to_i64(response.latency_ms)),
        metadata: json!({
            "routeId": response.route_id,
            "model": response.model,
            "target": "llm",
            "source": "ai.models.chat"
        }),
        user_id,
        now,
    }
}

async fn record_model_chat_usage(
    db: &PgPool,
    record: &ModelUsageSaveRecord,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
WITH selected_route AS (
    SELECT id, model_profile_id
    FROM ai_model_route
    WHERE tenant_id = $2
      AND route_purpose = 'chat'
      AND status = 1
    ORDER BY priority ASC, id ASC
    LIMIT 1
)
INSERT INTO ai_model_usage (
    id, tenant_id, route_id, model_profile_id, run_id, usage_kind,
    prompt_tokens, completion_tokens, total_tokens, request_count, vector_count,
    cost_cents, latency_ms, metadata, create_user, create_time
)
VALUES (
    $1, $2,
    (SELECT id FROM selected_route),
    (SELECT model_profile_id FROM selected_route),
    NULL, $3, $4, $5, $6, 1, 0, 0, $7, $8, $9, $10
);
"#,
    )
    .bind(record.id)
    .bind(record.tenant_id)
    .bind(&record.usage_kind)
    .bind(record.prompt_tokens)
    .bind(record.completion_tokens)
    .bind(record.total_tokens)
    .bind(record.latency_ms)
    .bind(&record.metadata)
    .bind(record.user_id)
    .bind(record.now)
    .execute(db)
    .await?;
    Ok(())
}

fn u128_to_i64(value: u128) -> i64 {
    value.min(i64::MAX as u128) as i64
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

fn public_masked_credential(masked_value: Option<String>) -> Option<String> {
    let value = masked_value?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let upper = value.to_ascii_uppercase();
    if value.starts_with("env:") || upper.contains("_API_KEY") || upper.contains("SECRET") {
        return Some("configured".to_owned());
    }

    if value.starts_with("sk-") && !value.contains("****") {
        return Some(mask_api_key(value));
    }

    Some(value.to_owned())
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

    #[test]
    fn model_registry_summary_sanitizes_env_mask_placeholders() {
        let summary = ModelRuntimeService::registry_summary_from_rows(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![ModelRouteRegistryRow {
                id: 30,
                code: "runtime.llm.chat".to_owned(),
                route_purpose: "chat".to_owned(),
                model_profile_id: 20,
                priority: 100,
                status: 1,
                credential_ref: Some("env:LLM_API_KEY".to_owned()),
                masked_value: Some("env:LLM_API_KEY".to_owned()),
            }],
        );

        assert_eq!(
            summary.routes[0].masked_credential.as_deref(),
            Some("configured")
        );
        let debug = format!("{summary:?}");
        assert!(!debug.contains("LLM_API_KEY"));
        assert!(!debug.contains("env:"));
    }

    #[test]
    fn model_chat_command_keeps_supported_roles_and_trims_content() {
        let command = normalize_model_chat_command(ModelChatCommand {
            messages: vec![
                ModelChatMessage {
                    role: " system ".to_owned(),
                    content: "  You are Novex.  ".to_owned(),
                },
                ModelChatMessage {
                    role: "user".to_owned(),
                    content: "  介绍一下 RAG 入库链路  ".to_owned(),
                },
            ],
            temperature: Some(1.5),
            max_tokens: Some(4096),
        })
        .unwrap();

        assert_eq!(command.messages[0].role, "system");
        assert_eq!(command.messages[0].content, "You are Novex.");
        assert_eq!(command.messages[1].role, "user");
        assert_eq!(command.temperature, Some(1.0));
        assert_eq!(command.max_tokens, Some(2048));
    }

    #[test]
    fn model_chat_payload_uses_llm_route_model_and_messages() {
        let route = llm_test_config()
            .route(ModelRuntimeTarget::Llm)
            .unwrap()
            .clone();
        let command = normalize_model_chat_command(ModelChatCommand {
            messages: vec![ModelChatMessage {
                role: "user".to_owned(),
                content: "hello".to_owned(),
            }],
            temperature: None,
            max_tokens: None,
        })
        .unwrap();

        let payload = model_chat_request_payload(&route, &command);

        assert_eq!(payload["model"], "deepseek-v4-flash");
        assert_eq!(payload["temperature"], 0.2);
        assert_eq!(payload["max_tokens"], 1024);
        assert_eq!(payload["messages"][0]["role"], "user");
        assert_eq!(payload["messages"][0]["content"], "hello");
        let debug = format!("{payload:?}");
        assert!(!debug.contains("sk-fake-llm-secret-508d"));
    }

    #[test]
    fn model_chat_response_extracts_answer_usage_and_route_summary() {
        let route = llm_test_config()
            .route(ModelRuntimeTarget::Llm)
            .unwrap()
            .clone();
        let body = json!({
            "choices": [
                { "message": { "content": "Novex can run pure model chat." } }
            ],
            "usage": {
                "prompt_tokens": 11,
                "completion_tokens": 7,
                "total_tokens": 18
            }
        });

        let response = model_chat_response_from_provider(&route, body, 42).unwrap();

        assert_eq!(response.answer, "Novex can run pure model chat.");
        assert_eq!(response.route_id, "runtime.llm");
        assert_eq!(response.model.as_deref(), Some("deepseek-v4-flash"));
        assert_eq!(response.latency_ms, 42);
        assert_eq!(response.usage.prompt_tokens, Some(11));
        assert_eq!(response.usage.completion_tokens, Some(7));
        assert_eq!(response.usage.total_tokens, Some(18));
        assert!(!format!("{response:?}").contains("sk-fake-llm-secret-508d"));
    }

    #[test]
    fn model_chat_usage_record_maps_tokens_latency_and_route_without_content() {
        let now = chrono::NaiveDateTime::parse_from_str("2026-06-05 10:00:00", "%Y-%m-%d %H:%M:%S")
            .unwrap();
        let response = ModelChatResp {
            answer: "Do not persist this answer".to_owned(),
            route_id: "runtime.llm".to_owned(),
            model: Some("deepseek-v4-flash".to_owned()),
            latency_ms: 42,
            usage: ModelChatUsage {
                prompt_tokens: Some(11),
                completion_tokens: Some(7),
                total_tokens: Some(18),
            },
        };

        let record = model_chat_usage_record(99, &response, now);

        assert_eq!(record.tenant_id, 1);
        assert_eq!(record.user_id, 99);
        assert_eq!(record.usage_kind, "chat");
        assert_eq!(record.prompt_tokens, 11);
        assert_eq!(record.completion_tokens, 7);
        assert_eq!(record.total_tokens, 18);
        assert_eq!(record.latency_ms, Some(42));
        assert_eq!(record.metadata["routeId"], "runtime.llm");
        assert_eq!(record.metadata["model"], "deepseek-v4-flash");
        assert!(!record
            .metadata
            .to_string()
            .contains("Do not persist this answer"));
    }

    #[test]
    fn model_chat_rejects_empty_or_unsupported_messages() {
        let err = normalize_model_chat_command(ModelChatCommand::default()).unwrap_err();
        assert!(err.to_string().contains("至少需要一条消息"));

        let err = normalize_model_chat_command(ModelChatCommand {
            messages: vec![ModelChatMessage {
                role: "tool".to_owned(),
                content: "hello".to_owned(),
            }],
            ..ModelChatCommand::default()
        })
        .unwrap_err();
        assert!(err.to_string().contains("消息角色不支持"));
    }

    fn llm_test_config() -> ModelRuntimeConfig {
        ModelRuntimeConfig::from_env_map(|key| match key {
            "LLM_API_KEY" => Some("sk-fake-llm-secret-508d".to_owned()),
            "LLM_BASE_URL" => Some("https://api.deepseek.com".to_owned()),
            "LLM_MODEL" => Some("deepseek-v4-flash".to_owned()),
            _ => None,
        })
    }
}
