use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use chrono::{NaiveDateTime, Utc};
use novex_connectors::FeishuTextMessage;
use novex_model::{
    estimate_model_cost_cents, estimate_model_text_tokens, evaluate_model_route_policy,
    mask_api_key, normalize_model_provider_usage, ModelKind, ModelProviderType,
    ModelRoutePolicyInput, ModelRoutePolicyStatus, ModelRoutePurpose, ModelRuntimeConfig,
    ModelRuntimeRoute, ModelRuntimeRouteSummary, ModelRuntimeSummary, ModelRuntimeTarget,
    ModelTokenUsage, ModelUsageCostInput,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgPool};

use crate::{
    application::system::{ensure_max_chars, format_datetime},
    infrastructure::persistence::ai_capability_repository::{
        AiCapabilityRepository, ToolAuditSaveRecord,
    },
    shared::error::AppError,
    shared::id::next_id,
};

const MODEL_HEALTH_TIMEOUT: Duration = Duration::from_secs(20);
const MODEL_ALERT_DELIVERY_TOOL_CODE: &str = "feishu.message.send";
const MODEL_ALERT_DELIVERY_CHANNEL_FEISHU: &str = "feishu";
const MODEL_ALERT_DELIVERY_TIMEOUT: Duration = Duration::from_secs(10);
const MODEL_ALERT_DELIVERY_BATCH_LIMIT: i64 = 100;
const MODEL_CHAT_TIMEOUT: Duration = Duration::from_secs(120);
const MODEL_RERANK_TIMEOUT: Duration = Duration::from_secs(30);
const MODEL_EMBEDDING_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_MODEL_CHAT_TEMPERATURE: f64 = 0.2;
const MAX_MODEL_CHAT_TEMPERATURE: f64 = 1.0;
const DEFAULT_MODEL_CHAT_MAX_TOKENS: u32 = 1024;
const MAX_MODEL_CHAT_MAX_TOKENS: u32 = 4096;
const MAX_MODEL_CHAT_MESSAGES: usize = 30;
const MAX_MODEL_CHAT_CONTENT_CHARS: usize = 12_000;
const MAX_MODEL_CHAT_FILE_CONTEXTS: usize = 3;
const MAX_MODEL_CHAT_FILE_CONTEXT_CHARS: usize = 20_000;
const MAX_MODEL_RUNTIME_RETRIES: usize = 3;
const DEFAULT_TENANT_ID: i64 = 1;
const MODEL_CHAT_HISTORY_LIMIT: i64 = 30;
const MODEL_CHAT_TITLE_CHARS: usize = 60;
const MODEL_CHAT_PREVIEW_CHARS: usize = 160;
const MAX_MODEL_FALLBACK_HOPS: usize = 3;

static MODEL_ROUTE_CIRCUIT_BREAKERS: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct ModelRuntimeService {
    db: PgPool,
    tenant_id: i64,
}

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
    pub conversation_id: Option<i64>,
    #[serde(default)]
    pub route_id: Option<String>,
    #[serde(default)]
    pub messages: Vec<ModelChatMessage>,
    #[serde(default)]
    pub file_contexts: Vec<ModelChatFileContext>,
    #[serde(default)]
    pub response_format: Option<Value>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default, rename = "maxTokens")]
    pub max_tokens: Option<u32>,
    #[serde(default, rename = "requestMetadata")]
    pub request_metadata: Option<ModelChatRequestMetadata>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelChatRequestKind {
    Compaction,
}

impl ModelChatRequestKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compaction => "compaction",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelChatCompactionMetadata {
    pub implementation: String,
    pub trigger: String,
    pub reason: String,
    pub phase: String,
    pub strategy: String,
    pub window_id: u64,
    pub input_history_count: usize,
    pub retained_history_count: usize,
    pub compacted_item_count: usize,
    pub retained_item_count: usize,
    pub tool_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelChatRequestMetadata {
    pub request_kind: ModelChatRequestKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compaction: Option<ModelChatCompactionMetadata>,
}

impl ModelChatRequestMetadata {
    pub fn remote_compaction(compaction: ModelChatCompactionMetadata) -> Self {
        Self {
            request_kind: ModelChatRequestKind::Compaction,
            compaction: Some(compaction),
        }
    }
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
pub struct ModelChatFileContext {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub content: String,
}

pub type ModelChatUsage = ModelTokenUsage;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelChatResp {
    pub conversation_id: Option<i64>,
    pub answer: String,
    pub route_id: String,
    pub provider: String,
    pub model: Option<String>,
    pub latency_ms: u128,
    pub usage: ModelChatUsage,
    pub cost_cents: Option<f64>,
    #[serde(default)]
    pub provider_attempts: Vec<ModelProviderAttempt>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProviderAttempt {
    pub attempt_kind: String,
    pub route_id: String,
    pub provider: String,
    pub model: Option<String>,
    pub status: String,
    pub latency_ms: i64,
    pub error_kind: Option<String>,
    pub http_status: Option<u16>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRetryPolicy {
    pub max_retries: usize,
}

impl ModelRetryPolicy {
    pub const fn disabled() -> Self {
        Self { max_retries: 0 }
    }

    pub const fn max_attempts(&self) -> usize {
        self.max_retries + 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFallbackPolicyDecision {
    pub enabled: bool,
    pub fallback_route_id: Option<String>,
    pub block_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRouteFallbackPlan {
    pub primary_route_id: String,
    pub decision: ModelFallbackPolicyDecision,
    pub policy_status: ModelRoutePolicyStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelChatConversationResp {
    pub id: i64,
    pub title: String,
    pub route_id: String,
    pub model: Option<String>,
    pub message_count: i32,
    pub last_message_preview: String,
    pub create_time: String,
    pub update_time: String,
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
    pub fallback_policy: Value,
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
    pub fallback_route_id: Option<i64>,
    pub status: i16,
    pub policy_status: ModelRoutePolicyStatus,
    pub masked_credential: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRouteCircuitBreakerResp {
    pub route_id: String,
    pub opened_until: String,
    pub open_reason: String,
    pub last_error_kind: Option<String>,
    pub last_http_status: Option<i32>,
    pub is_open: bool,
    pub remaining_ms: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelOpsUsageSummaryResp {
    pub request_count: i64,
    pub total_tokens: i64,
    pub cost_cents: f64,
    pub avg_latency_ms: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRouteOpsSummaryResp {
    pub route_id: String,
    pub route_purpose: String,
    pub provider: String,
    pub provider_type: String,
    pub model: String,
    pub network_zone: String,
    pub status: i16,
    pub breaker_open: bool,
    pub breaker_remaining_ms: i64,
    pub breaker_opened_until: Option<String>,
    pub last_health_status: Option<String>,
    pub last_health_checked_at: Option<String>,
    pub last_health_latency_ms: Option<i64>,
    pub active_alert_count: usize,
    pub degraded: bool,
    pub usage_24h: ModelOpsUsageSummaryResp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelOpsAlertResp {
    pub alert_key: String,
    pub alert_kind: String,
    pub severity: String,
    pub status: String,
    pub route_id: Option<String>,
    pub route_purpose: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub source_ref: String,
    pub message: String,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub event_payload: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelOpsAlertDeliverySummary {
    pub attempted_count: usize,
    pub sent_count: usize,
    pub dry_run_count: usize,
    pub failed_count: usize,
}

impl ModelOpsAlertDeliverySummary {
    fn record(&mut self, result: &ModelOpsAlertDeliveryResult) {
        self.attempted_count += 1;
        match result.status.as_str() {
            "sent" => self.sent_count += 1,
            "dry_run" => self.dry_run_count += 1,
            _ => self.failed_count += 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelOpsSummaryResp {
    pub route_count: usize,
    pub active_route_count: usize,
    pub open_breaker_count: usize,
    pub degraded_route_count: usize,
    pub active_alert_count: usize,
    pub usage_24h: ModelOpsUsageSummaryResp,
    pub alerts: Vec<ModelOpsAlertResp>,
    pub routes: Vec<ModelRouteOpsSummaryResp>,
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

#[derive(Debug, Clone, PartialEq, FromRow)]
pub struct ModelProfileRegistryRow {
    pub id: i64,
    pub deployment_id: i64,
    pub code: String,
    pub name: String,
    pub model_name: String,
    pub model_kind: String,
    pub fallback_policy: Value,
    pub status: i16,
}

#[derive(Debug, Clone, PartialEq, FromRow)]
pub struct ModelRouteRegistryRow {
    pub id: i64,
    pub code: String,
    pub route_purpose: String,
    pub model_profile_id: i64,
    pub priority: i32,
    pub fallback_route_id: Option<i64>,
    pub status: i16,
    pub policy: Value,
    pub credential_ref: Option<String>,
    pub masked_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, FromRow)]
struct ModelRuntimeRouteRow {
    pub route_id: i64,
    pub route_code: String,
    pub route_purpose: String,
    pub provider_type: String,
    pub model_profile_id: i64,
    pub model_name: String,
    pub model_kind: String,
    pub deployment_endpoint: String,
    pub api_path: Option<String>,
    pub credential_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, FromRow)]
struct ModelRouteRetryPolicyRow {
    pub route_policy: Value,
    pub fallback_policy: Value,
    pub network_zone: String,
    pub fallback_network_zone: Option<String>,
}

#[derive(Debug, Clone, PartialEq, FromRow)]
struct ModelRouteFallbackPolicyRow {
    pub route_code: String,
    pub route_policy: Value,
    pub fallback_policy: Value,
    pub network_zone: String,
    pub fallback_route_code: Option<String>,
    pub fallback_network_zone: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
struct ModelRouteCircuitBreakerRow {
    pub opened_until: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
struct ModelRouteCircuitBreakerControlRow {
    pub route_id: String,
    pub opened_until: NaiveDateTime,
    pub open_reason: String,
    pub last_error_kind: Option<String>,
    pub last_http_status: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, FromRow)]
struct ModelRouteOpsSummaryRow {
    pub route_code: String,
    pub route_purpose: String,
    pub provider_code: String,
    pub provider_type: String,
    pub model_name: String,
    pub network_zone: String,
    pub status: i16,
    pub breaker_opened_until: Option<NaiveDateTime>,
    pub last_health_status: Option<String>,
    pub last_health_checked_at: Option<NaiveDateTime>,
    pub last_health_latency_ms: Option<i64>,
    pub request_count_24h: i64,
    pub total_tokens_24h: i64,
    pub cost_cents_24h: f64,
    pub avg_latency_ms_24h: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, FromRow)]
struct ModelOpsAlertRow {
    pub alert_key: String,
    pub alert_kind: String,
    pub severity: String,
    pub status: String,
    pub route_code: Option<String>,
    pub route_purpose: Option<String>,
    pub provider_code: Option<String>,
    pub model_name: Option<String>,
    pub source_ref: String,
    pub event_payload: Value,
    pub first_seen_at: NaiveDateTime,
    pub last_seen_at: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
struct ModelHealthCheckRouteIdsRow {
    pub route_id: i64,
    pub provider_id: i64,
    pub model_profile_id: i64,
}

#[derive(Debug, Clone, PartialEq)]
struct ModelHealthCheckSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub route_id: Option<i64>,
    pub provider_id: Option<i64>,
    pub model_profile_id: Option<i64>,
    pub status: String,
    pub http_status: Option<i32>,
    pub latency_ms: Option<i64>,
    pub checked_at: NaiveDateTime,
    pub error_message: Option<String>,
    pub detail: Value,
    pub user_id: i64,
}

#[derive(Debug, Clone, PartialEq)]
struct ModelOpsAlertSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub alert_key: String,
    pub alert_kind: String,
    pub severity: String,
    pub status: String,
    pub route_id: Option<i64>,
    pub provider_id: Option<i64>,
    pub model_profile_id: Option<i64>,
    pub source_ref: String,
    pub event_payload: Value,
    pub first_seen_at: NaiveDateTime,
    pub last_seen_at: NaiveDateTime,
    pub user_id: i64,
}

#[derive(Debug, Clone, PartialEq, FromRow)]
struct ModelOpsAlertDeliveryCandidateRow {
    pub alert_id: i64,
    pub tenant_id: i64,
    pub alert_key: String,
    pub alert_kind: String,
    pub severity: String,
    pub route_code: Option<String>,
    pub route_purpose: Option<String>,
    pub provider_code: Option<String>,
    pub model_name: Option<String>,
    pub source_ref: String,
    pub event_payload: Value,
    pub first_seen_at: NaiveDateTime,
    pub last_seen_at: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq)]
struct ModelOpsAlertDeliveryResult {
    pub status: String,
    pub dry_run: bool,
    pub request_payload: Value,
    pub response_payload: Value,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct ModelOpsAlertDeliverySaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub alert_id: i64,
    pub alert_key: String,
    pub channel: String,
    pub status: String,
    pub dry_run: bool,
    pub tool_call_audit_id: Option<i64>,
    pub request_payload: Value,
    pub response_payload: Value,
    pub error_message: Option<String>,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelOpsAlertFeishuConfig {
    webhook_url: String,
}

impl ModelOpsAlertFeishuConfig {
    fn from_env() -> Option<Self> {
        Self::from_env_map(|key| env::var(key).ok())
    }

    fn from_env_map<F>(mut env_get: F) -> Option<Self>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let webhook_url = env_get("FEISHU_WEBHOOK_URL")
            .or_else(|| env_get("NOVEX_FEISHU_WEBHOOK_URL"))
            .map(|value| value.trim().trim_end_matches('/').to_owned())
            .filter(|value| !value.is_empty())?;

        Some(Self { webhook_url })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct ModelChatConversationRow {
    pub id: i64,
    pub title: String,
    pub route_id: String,
    pub model: Option<String>,
    pub message_count: i32,
    pub last_message_preview: String,
    pub create_time: NaiveDateTime,
    pub update_time: NaiveDateTime,
}

#[derive(Debug, Clone)]
struct ModelUsageSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub route_id: String,
    pub usage_kind: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub latency_ms: Option<i64>,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq, FromRow)]
struct ModelUsageRouteAccountingRow {
    pub route_id: i64,
    pub model_profile_id: i64,
    pub cost_spec: Value,
}

#[derive(Debug, Clone)]
struct ModelChatConversationSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub title: String,
    pub route_id: String,
    pub model: Option<String>,
    pub message_count_increment: i32,
    pub last_message_preview: String,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
struct ModelChatMessageSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub conversation_id: i64,
    pub role: String,
    pub content: String,
    pub route_id: Option<String>,
    pub model: Option<String>,
    pub token_count: i32,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
struct ModelChatHistorySaveRecords {
    pub conversation: ModelChatConversationSaveRecord,
    pub messages: Vec<ModelChatMessageSaveRecord>,
}

impl ModelRuntimeService {
    pub fn new(db: PgPool) -> Self {
        Self::for_tenant(db, DEFAULT_TENANT_ID)
    }

    pub fn for_tenant(db: PgPool, tenant_id: i64) -> Self {
        Self { db, tenant_id }
    }

    pub fn runtime_config_summary(config: ModelRuntimeConfig) -> ModelRuntimeSummary {
        config.summary()
    }

    pub fn runtime_config() -> ModelRuntimeSummary {
        Self::runtime_config_summary(ModelRuntimeConfig::from_env())
    }

    pub async fn effective_runtime_summary(&self) -> Result<ModelRuntimeSummary, AppError> {
        let mut routes = Vec::new();
        for purpose in [
            ModelRoutePurpose::Chat,
            ModelRoutePurpose::RagAnswer,
            ModelRoutePurpose::Embedding,
            ModelRoutePurpose::Rerank,
            ModelRoutePurpose::EvalJudge,
            ModelRoutePurpose::CodeAgent,
            ModelRoutePurpose::GuardianReview,
            ModelRoutePurpose::MediaGeneration,
        ] {
            if let Some(route) = self.resolve_route_for_purpose(purpose).await? {
                routes.push(route);
            }
        }

        Ok(effective_runtime_summary_from_routes(
            routes,
            ModelRuntimeConfig::from_env().missing_env().to_vec(),
        ))
    }

    pub async fn resolve_route_for_purpose(
        &self,
        purpose: ModelRoutePurpose,
    ) -> Result<Option<ModelRuntimeRoute>, AppError> {
        self.resolve_route_for_purpose_with_route_id(purpose, None)
            .await
    }

    pub async fn retry_policy_for_purpose(
        &self,
        purpose: ModelRoutePurpose,
    ) -> Result<ModelRetryPolicy, AppError> {
        self.retry_policy_for_purpose_with_route_id(purpose, None)
            .await
    }

    pub async fn retry_policy_for_purpose_with_route_id(
        &self,
        purpose: ModelRoutePurpose,
        route_id: Option<&str>,
    ) -> Result<ModelRetryPolicy, AppError> {
        let row = sqlx::query_as::<_, ModelRouteRetryPolicyRow>(
            r#"
SELECT
    r.policy AS route_policy,
    profile.fallback_policy AS fallback_policy,
    deployment.network_zone AS network_zone,
    fallback_deployment.network_zone AS fallback_network_zone
FROM ai_model_route r
JOIN ai_model_profile profile
  ON profile.tenant_id = r.tenant_id
 AND profile.id = r.model_profile_id
 AND profile.status = 1
JOIN ai_model_deployment deployment
  ON deployment.tenant_id = profile.tenant_id
 AND deployment.id = profile.deployment_id
 AND deployment.status = 1
LEFT JOIN ai_model_route fallback_route
  ON fallback_route.tenant_id = r.tenant_id
 AND fallback_route.id = r.fallback_route_id
 AND fallback_route.status = 1
LEFT JOIN ai_model_profile fallback_profile
  ON fallback_profile.tenant_id = fallback_route.tenant_id
 AND fallback_profile.id = fallback_route.model_profile_id
 AND fallback_profile.status = 1
LEFT JOIN ai_model_deployment fallback_deployment
  ON fallback_deployment.tenant_id = fallback_profile.tenant_id
 AND fallback_deployment.id = fallback_profile.deployment_id
 AND fallback_deployment.status = 1
WHERE r.tenant_id = $1
  AND r.route_purpose = $2
  AND ($3::text IS NULL OR r.code = $3)
  AND r.status = 1
ORDER BY r.priority ASC, r.id ASC
LIMIT 1;
"#,
        )
        .bind(self.tenant_id)
        .bind(purpose.as_str())
        .bind(route_id)
        .fetch_optional(&self.db)
        .await?;

        let Some(row) = row else {
            return Ok(ModelRetryPolicy::disabled());
        };
        let status = evaluate_model_route_policy(ModelRoutePolicyInput {
            network_zone: &row.network_zone,
            fallback_network_zone: row.fallback_network_zone.as_deref(),
            fallback_policy: &row.fallback_policy,
            route_policy: &row.route_policy,
        });

        Ok(model_retry_policy_from_route_policy_status(&status))
    }

    pub async fn fallback_plan_for_purpose(
        &self,
        purpose: ModelRoutePurpose,
    ) -> Result<Option<ModelRouteFallbackPlan>, AppError> {
        self.fallback_plan_for_purpose_with_route_id(purpose, None)
            .await
    }

    pub async fn list_route_circuit_breakers(
        &self,
    ) -> Result<Vec<ModelRouteCircuitBreakerResp>, AppError> {
        let rows = sqlx::query_as::<_, ModelRouteCircuitBreakerControlRow>(
            r#"
SELECT route_id, opened_until, open_reason, last_error_kind, last_http_status
FROM ai_model_route_circuit_breaker
WHERE tenant_id = $1
ORDER BY opened_until DESC, route_id ASC;
"#,
        )
        .bind(self.tenant_id)
        .fetch_all(&self.db)
        .await?;
        let now = Utc::now().naive_utc();

        Ok(rows
            .into_iter()
            .map(|row| {
                let remaining_ms = (row.opened_until - now).num_milliseconds().max(0);

                ModelRouteCircuitBreakerResp {
                    route_id: row.route_id,
                    opened_until: format_datetime(row.opened_until),
                    open_reason: row.open_reason,
                    last_error_kind: row.last_error_kind,
                    last_http_status: row.last_http_status,
                    is_open: remaining_ms > 0,
                    remaining_ms,
                }
            })
            .collect())
    }

    pub async fn clear_route_circuit_breaker(&self, route_id: &str) -> Result<(), AppError> {
        let route_id = route_id.trim();
        if route_id.is_empty() {
            return Err(AppError::bad_request("模型路由不能为空"));
        }
        ensure_max_chars("模型路由", route_id, 128)?;

        sqlx::query(
            r#"
DELETE FROM ai_model_route_circuit_breaker
WHERE tenant_id = $1
  AND route_id = $2;
"#,
        )
        .bind(self.tenant_id)
        .bind(route_id)
        .execute(&self.db)
        .await?;
        model_circuit_breaker_clear(route_id);

        Ok(())
    }

    pub async fn model_ops_summary(&self) -> Result<ModelOpsSummaryResp, AppError> {
        let rows = sqlx::query_as::<_, ModelRouteOpsSummaryRow>(
            r#"
SELECT
    r.code AS route_code,
    r.route_purpose,
    provider.code AS provider_code,
    provider.provider_type,
    profile.model_name,
    deployment.network_zone,
    r.status,
    breaker.opened_until AS breaker_opened_until,
    health.status AS last_health_status,
    health.checked_at AS last_health_checked_at,
    health.latency_ms AS last_health_latency_ms,
    COALESCE(usage.request_count_24h, 0)::bigint AS request_count_24h,
    COALESCE(usage.total_tokens_24h, 0)::bigint AS total_tokens_24h,
    COALESCE(usage.cost_cents_24h, 0)::float8 AS cost_cents_24h,
    usage.avg_latency_ms_24h AS avg_latency_ms_24h
FROM ai_model_route r
JOIN ai_model_profile profile
  ON profile.tenant_id = r.tenant_id
 AND profile.id = r.model_profile_id
JOIN ai_model_deployment deployment
  ON deployment.tenant_id = profile.tenant_id
 AND deployment.id = profile.deployment_id
JOIN ai_model_provider provider
  ON provider.tenant_id = deployment.tenant_id
 AND provider.id = deployment.provider_id
LEFT JOIN ai_model_route_circuit_breaker breaker
  ON breaker.tenant_id = r.tenant_id
 AND breaker.route_id = r.code
 AND breaker.opened_until > NOW()::timestamp
LEFT JOIN LATERAL (
    SELECT status, checked_at, latency_ms
    FROM ai_model_health_check health
    WHERE health.tenant_id = r.tenant_id
      AND health.route_id = r.id
    ORDER BY health.checked_at DESC, health.id DESC
    LIMIT 1
) health ON TRUE
LEFT JOIN (
    SELECT
        route_id,
        SUM(request_count)::bigint AS request_count_24h,
        SUM(total_tokens)::bigint AS total_tokens_24h,
        SUM(cost_cents)::float8 AS cost_cents_24h,
        AVG(latency_ms)::float8 AS avg_latency_ms_24h
    FROM ai_model_usage
    WHERE tenant_id = $1
      AND create_time >= NOW()::timestamp - INTERVAL '24 hours'
      AND route_id IS NOT NULL
    GROUP BY route_id
) usage ON usage.route_id = r.id
WHERE r.tenant_id = $1
ORDER BY r.priority ASC, r.id ASC;
"#,
        )
        .bind(self.tenant_id)
        .fetch_all(&self.db)
        .await?;
        let alert_rows = sqlx::query_as::<_, ModelOpsAlertRow>(
            r#"
SELECT
    alert.alert_key,
    alert.alert_kind,
    alert.severity,
    alert.status,
    route.code AS route_code,
    route.route_purpose,
    provider.code AS provider_code,
    profile.model_name,
    COALESCE(alert.source_ref, '') AS source_ref,
    alert.event_payload,
    alert.first_seen_at,
    alert.last_seen_at
FROM ai_model_ops_alert alert
LEFT JOIN ai_model_route route
  ON route.tenant_id = alert.tenant_id
 AND route.id = alert.route_id
LEFT JOIN ai_model_profile profile
  ON profile.tenant_id = alert.tenant_id
 AND profile.id = alert.model_profile_id
LEFT JOIN ai_model_deployment deployment
  ON deployment.tenant_id = alert.tenant_id
 AND deployment.id = profile.deployment_id
LEFT JOIN ai_model_provider provider
  ON provider.tenant_id = alert.tenant_id
 AND provider.id = alert.provider_id
WHERE alert.tenant_id = $1
  AND alert.resolved_at IS NULL
ORDER BY alert.last_seen_at DESC, alert.id DESC;
"#,
        )
        .bind(self.tenant_id)
        .fetch_all(&self.db)
        .await?;

        Ok(model_ops_summary_from_rows(
            rows,
            alert_rows,
            Utc::now().naive_utc(),
        ))
    }

    async fn fallback_plan_for_purpose_with_route_id(
        &self,
        purpose: ModelRoutePurpose,
        route_id: Option<&str>,
    ) -> Result<Option<ModelRouteFallbackPlan>, AppError> {
        let row = sqlx::query_as::<_, ModelRouteFallbackPolicyRow>(
            r#"
SELECT
    r.code AS route_code,
    r.policy AS route_policy,
    profile.fallback_policy AS fallback_policy,
    deployment.network_zone AS network_zone,
    fallback_route.code AS fallback_route_code,
    fallback_deployment.network_zone AS fallback_network_zone
FROM ai_model_route r
JOIN ai_model_profile profile
  ON profile.tenant_id = r.tenant_id
 AND profile.id = r.model_profile_id
 AND profile.status = 1
JOIN ai_model_deployment deployment
  ON deployment.tenant_id = profile.tenant_id
 AND deployment.id = profile.deployment_id
 AND deployment.status = 1
LEFT JOIN ai_model_route fallback_route
  ON fallback_route.tenant_id = r.tenant_id
 AND fallback_route.id = r.fallback_route_id
 AND fallback_route.status = 1
LEFT JOIN ai_model_profile fallback_profile
  ON fallback_profile.tenant_id = fallback_route.tenant_id
 AND fallback_profile.id = fallback_route.model_profile_id
 AND fallback_profile.status = 1
LEFT JOIN ai_model_deployment fallback_deployment
  ON fallback_deployment.tenant_id = fallback_profile.tenant_id
 AND fallback_deployment.id = fallback_profile.deployment_id
 AND fallback_deployment.status = 1
WHERE r.tenant_id = $1
  AND r.route_purpose = $2
  AND ($3::text IS NULL OR r.code = $3)
  AND r.status = 1
ORDER BY r.priority ASC, r.id ASC
LIMIT 1;
"#,
        )
        .bind(self.tenant_id)
        .bind(purpose.as_str())
        .bind(route_id)
        .fetch_optional(&self.db)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        let policy_status = evaluate_model_route_policy(ModelRoutePolicyInput {
            network_zone: &row.network_zone,
            fallback_network_zone: row.fallback_network_zone.as_deref(),
            fallback_policy: &row.fallback_policy,
            route_policy: &row.route_policy,
        });
        let decision = model_fallback_policy_decision_from_status(
            &policy_status,
            row.fallback_route_code.as_deref(),
        );

        Ok(Some(ModelRouteFallbackPlan {
            primary_route_id: row.route_code,
            decision,
            policy_status,
        }))
    }

    async fn persistent_model_circuit_breaker_open(
        &self,
        route_id: &str,
        cooldown_seconds: u32,
        attempt: &ModelProviderAttempt,
    ) -> Result<(), AppError> {
        if cooldown_seconds == 0 || route_id.trim().is_empty() {
            return Ok(());
        }
        let now = Utc::now().naive_utc();
        let opened_until = now + chrono::Duration::seconds(cooldown_seconds as i64);
        let http_status = attempt.http_status.map(i32::from);
        sqlx::query(
            r#"
INSERT INTO ai_model_route_circuit_breaker
    (id, tenant_id, route_id, opened_until, open_reason, last_error_kind, last_http_status, create_user, create_time, update_user, update_time)
VALUES
    ($1, $2, $3, $4, 'provider_failure', $5, $6, $7, $8, $7, $8)
ON CONFLICT (tenant_id, route_id) DO UPDATE
SET opened_until = EXCLUDED.opened_until,
    open_reason = EXCLUDED.open_reason,
    last_error_kind = EXCLUDED.last_error_kind,
    last_http_status = EXCLUDED.last_http_status,
    update_user = EXCLUDED.update_user,
    update_time = EXCLUDED.update_time;
"#,
        )
        .bind(next_id())
        .bind(self.tenant_id)
        .bind(route_id.trim())
        .bind(opened_until)
        .bind(attempt.error_kind.as_deref())
        .bind(http_status)
        .bind(DEFAULT_TENANT_ID)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn persistent_model_circuit_breaker_open_attempt(
        &self,
        route: &ModelRuntimeRoute,
    ) -> Result<Option<ModelProviderAttempt>, AppError> {
        let row = sqlx::query_as::<_, ModelRouteCircuitBreakerRow>(
            r#"
SELECT opened_until
FROM ai_model_route_circuit_breaker
WHERE tenant_id = $1
  AND route_id = $2
  AND opened_until > NOW()::timestamp
ORDER BY opened_until DESC
LIMIT 1;
"#,
        )
        .bind(self.tenant_id)
        .bind(route.route_id())
        .fetch_optional(&self.db)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        let now = Utc::now().naive_utc();
        let Ok(remaining) = (row.opened_until - now).to_std() else {
            return Ok(None);
        };
        if remaining.is_zero() {
            return Ok(None);
        }

        Ok(Some(model_provider_attempt_circuit_open(route, remaining)))
    }

    pub async fn resolve_route_for_purpose_with_route_id(
        &self,
        purpose: ModelRoutePurpose,
        route_id: Option<&str>,
    ) -> Result<Option<ModelRuntimeRoute>, AppError> {
        let route_id = route_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let rows = sqlx::query_as::<_, ModelRuntimeRouteRow>(
            r#"
SELECT
    r.id AS route_id,
    r.code AS route_code,
    r.route_purpose,
    provider.provider_type,
    profile.id AS model_profile_id,
    profile.model_name,
    profile.model_kind,
    deployment.endpoint AS deployment_endpoint,
    deployment.api_path,
    credential.credential_ref
FROM ai_model_route r
JOIN ai_model_profile profile
  ON profile.tenant_id = r.tenant_id
 AND profile.id = r.model_profile_id
 AND profile.status = 1
JOIN ai_model_deployment deployment
  ON deployment.tenant_id = profile.tenant_id
 AND deployment.id = profile.deployment_id
 AND deployment.status = 1
JOIN ai_model_provider provider
  ON provider.tenant_id = deployment.tenant_id
 AND provider.id = deployment.provider_id
 AND provider.status = 1
LEFT JOIN ai_model_credential credential
  ON credential.tenant_id = r.tenant_id
 AND credential.id = r.credential_id
 AND credential.status = 1
WHERE r.tenant_id = $1
  AND r.route_purpose = $2
  AND ($3::text IS NULL OR r.code = $3)
  AND r.status = 1
ORDER BY r.priority ASC, r.id ASC;
"#,
        )
        .bind(self.tenant_id)
        .bind(purpose.as_str())
        .bind(route_id.as_deref())
        .fetch_all(&self.db)
        .await?;

        for row in rows {
            if let Some(route) = runtime_route_from_registry_row(&row, |key| env::var(key).ok()) {
                return Ok(Some(route));
            }
        }

        let fallback = env_fallback_route_for_purpose(purpose, &ModelRuntimeConfig::from_env());
        match (route_id.as_deref(), fallback) {
            (None, fallback) => Ok(fallback),
            (Some(selected), Some(route)) if route.route_id() == selected => Ok(Some(route)),
            (Some(_), _) => Err(AppError::bad_request("选择的模型路由不可用")),
        }
    }

    pub fn parse_rerank_scores(body: &Value) -> Vec<ModelRerankScore> {
        body.get("results")
            .or_else(|| body.get("data"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(parse_rerank_score)
            .collect()
    }

    pub fn parse_embedding_vectors(body: &Value) -> Vec<ModelEmbeddingVector> {
        body.get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(parse_embedding_vector)
            .collect()
    }

    pub async fn embed_texts(
        route: &ModelRuntimeRoute,
        texts: &[String],
    ) -> Result<Vec<ModelEmbeddingVector>, AppError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let client = reqwest::Client::builder()
            .timeout(MODEL_EMBEDDING_TIMEOUT)
            .build()
            .map_err(|err| AppError::Anyhow(err.into()))?;
        let response = client
            .post(route.endpoint())
            .bearer_auth(route.api_key())
            .json(&json!({
                "model": route.model().unwrap_or_default(),
                "input": texts,
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
        let vectors = Self::parse_embedding_vectors(&body);
        if vectors.is_empty() {
            return Err(AppError::bad_request("Embedding 模型响应为空"));
        }
        Ok(vectors)
    }

    pub async fn rerank_documents(
        route: &ModelRuntimeRoute,
        query: &str,
        documents: &[String],
    ) -> Result<Vec<ModelRerankScore>, AppError> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }
        let client = reqwest::Client::builder()
            .timeout(MODEL_RERANK_TIMEOUT)
            .build()
            .map_err(|err| AppError::Anyhow(err.into()))?;
        let response = client
            .post(route.endpoint())
            .bearer_auth(route.api_key())
            .json(&json!({
                "model": route.model().unwrap_or_default(),
                "query": query,
                "documents": documents,
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
        let scores = Self::parse_rerank_scores(&body);
        if scores.is_empty() {
            return Err(AppError::bad_request("Rerank 模型响应为空"));
        }
        Ok(scores)
    }

    pub async fn registry_summary(db: &PgPool) -> Result<ModelRegistrySummary, AppError> {
        Self::registry_summary_for_tenant(db, DEFAULT_TENANT_ID).await
    }

    pub async fn refresh_active_tenant_model_health(db: &PgPool) -> Result<usize, AppError> {
        let tenant_ids = sqlx::query_scalar::<_, i64>(
            r#"
SELECT DISTINCT tenant_id
FROM ai_model_route
WHERE status = 1
ORDER BY tenant_id;
"#,
        )
        .fetch_all(db)
        .await?;
        let mut count = 0usize;
        for tenant_id in tenant_ids {
            let service = Self::for_tenant(db.clone(), tenant_id);
            let response = service
                .health_check_for_tenant(ModelHealthCheckCommand {
                    target: Some("all".to_owned()),
                })
                .await?;
            count += response.results.len();
        }

        Ok(count)
    }

    pub async fn deliver_active_model_ops_alerts(
        db: &PgPool,
    ) -> Result<ModelOpsAlertDeliverySummary, AppError> {
        let candidates =
            model_ops_alert_delivery_candidates(db, MODEL_ALERT_DELIVERY_BATCH_LIMIT).await?;
        let capability_repo = AiCapabilityRepository::new(db.clone());
        let mut summary = ModelOpsAlertDeliverySummary::default();
        for candidate in candidates {
            let result =
                deliver_model_ops_alert_candidate(db, &capability_repo, &candidate).await?;
            summary.record(&result);
        }

        Ok(summary)
    }

    pub async fn registry_summary_for_tenant(
        db: &PgPool,
        tenant_id: i64,
    ) -> Result<ModelRegistrySummary, AppError> {
        let providers = sqlx::query_as::<_, ModelProviderRegistryRow>(
            r#"
SELECT id, code, name, provider_type, status
FROM ai_model_provider
WHERE tenant_id = $1
ORDER BY id
"#,
        )
        .bind(tenant_id)
        .fetch_all(db)
        .await?;
        let deployments = sqlx::query_as::<_, ModelDeploymentRegistryRow>(
            r#"
SELECT id, provider_id, code, name, endpoint, network_zone, status
FROM ai_model_deployment
WHERE tenant_id = $1
ORDER BY id
"#,
        )
        .bind(tenant_id)
        .fetch_all(db)
        .await?;
        let profiles = sqlx::query_as::<_, ModelProfileRegistryRow>(
            r#"
SELECT id, deployment_id, code, name, model_name, model_kind, fallback_policy, status
FROM ai_model_profile
WHERE tenant_id = $1
ORDER BY id
"#,
        )
        .bind(tenant_id)
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
    r.fallback_route_id,
    r.status,
    r.policy,
    c.credential_ref,
    c.masked_value
FROM ai_model_route r
LEFT JOIN ai_model_credential c ON c.id = r.credential_id
WHERE r.tenant_id = $1
ORDER BY r.priority, r.id
"#,
        )
        .bind(tenant_id)
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
        let deployment_zones = deployments
            .iter()
            .map(|row| (row.id, row.network_zone.clone()))
            .collect::<HashMap<_, _>>();
        let profile_policy_contexts = profiles
            .iter()
            .map(|row| {
                let network_zone = deployment_zones
                    .get(&row.deployment_id)
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_owned());
                (row.id, (network_zone, row.fallback_policy.clone()))
            })
            .collect::<HashMap<_, _>>();
        let route_network_zones = routes
            .iter()
            .map(|row| {
                let network_zone = profile_policy_contexts
                    .get(&row.model_profile_id)
                    .map(|(network_zone, _)| network_zone.clone())
                    .unwrap_or_else(|| "unknown".to_owned());
                (row.id, network_zone)
            })
            .collect::<HashMap<_, _>>();

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
                    fallback_policy: row.fallback_policy,
                    status: row.status,
                })
                .collect(),
            routes: routes
                .into_iter()
                .map(|row| {
                    let (network_zone, fallback_policy) = profile_policy_contexts
                        .get(&row.model_profile_id)
                        .cloned()
                        .unwrap_or_else(|| ("unknown".to_owned(), Value::Null));
                    let fallback_network_zone = row.fallback_route_id.and_then(|route_id| {
                        route_network_zones.get(&route_id).map(String::as_str)
                    });
                    let policy_status = evaluate_model_route_policy(ModelRoutePolicyInput {
                        network_zone: &network_zone,
                        fallback_network_zone,
                        fallback_policy: &fallback_policy,
                        route_policy: &row.policy,
                    });

                    ModelRouteRegistryResp {
                        id: row.id,
                        code: row.code,
                        route_purpose: row.route_purpose,
                        model_profile_id: row.model_profile_id,
                        priority: row.priority,
                        fallback_route_id: row.fallback_route_id,
                        status: row.status,
                        policy_status,
                        masked_credential: public_masked_credential(row.masked_value),
                    }
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

    pub async fn health_check_for_tenant(
        &self,
        command: ModelHealthCheckCommand,
    ) -> Result<ModelHealthCheckResp, AppError> {
        let targets = health_check_targets(command.target.as_deref())?;
        let client = reqwest::Client::builder()
            .timeout(MODEL_HEALTH_TIMEOUT)
            .build()
            .map_err(|err| AppError::Anyhow(err.into()))?;

        let mut results = Vec::with_capacity(targets.len());
        for target in targets {
            let purpose = default_purpose_for_target(target);
            match self.resolve_route_for_purpose(purpose).await? {
                Some(route) => results.push(check_target_with_route(&client, &route, target).await),
                None => results.push(ModelHealthCheckResult {
                    target,
                    configured: false,
                    ok: false,
                    endpoint: None,
                    masked_api_key: None,
                    http_status: None,
                    latency_ms: 0,
                    message: "未配置完整模型路由".to_owned(),
                    detail: None,
                }),
            }
        }
        self.persist_model_health_check_results(&results, DEFAULT_TENANT_ID)
            .await?;

        Ok(ModelHealthCheckResp { results })
    }

    async fn persist_model_health_check_results(
        &self,
        results: &[ModelHealthCheckResult],
        user_id: i64,
    ) -> Result<usize, AppError> {
        let mut count = 0usize;
        for result in results {
            let route_ids = self
                .model_health_check_route_ids(default_purpose_for_target(result.target))
                .await?
                .map(|row| (row.route_id, row.provider_id, row.model_profile_id));
            let record = model_health_check_record_from_result(
                self.tenant_id,
                user_id,
                route_ids,
                result,
                Utc::now().naive_utc(),
            );
            persist_model_health_check_record(&self.db, &record).await?;
            record_model_ops_alert_for_health_check(
                &self.db,
                self.tenant_id,
                user_id,
                route_ids,
                result,
                &record,
            )
            .await?;
            count += 1;
        }
        Ok(count)
    }

    async fn model_health_check_route_ids(
        &self,
        purpose: ModelRoutePurpose,
    ) -> Result<Option<ModelHealthCheckRouteIdsRow>, AppError> {
        sqlx::query_as::<_, ModelHealthCheckRouteIdsRow>(
            r#"
SELECT
    r.id AS route_id,
    provider.id AS provider_id,
    profile.id AS model_profile_id
FROM ai_model_route r
JOIN ai_model_profile profile
  ON profile.tenant_id = r.tenant_id
 AND profile.id = r.model_profile_id
JOIN ai_model_deployment deployment
  ON deployment.tenant_id = profile.tenant_id
 AND deployment.id = profile.deployment_id
JOIN ai_model_provider provider
  ON provider.tenant_id = deployment.tenant_id
 AND provider.id = deployment.provider_id
WHERE r.tenant_id = $1
  AND r.route_purpose = $2
  AND r.status = 1
ORDER BY r.priority ASC, r.id ASC
LIMIT 1;
"#,
        )
        .bind(self.tenant_id)
        .bind(purpose.as_str())
        .fetch_optional(&self.db)
        .await
        .map_err(AppError::from)
    }

    pub async fn chat_completion(command: ModelChatCommand) -> Result<ModelChatResp, AppError> {
        execute_chat_completion(command).await
    }

    pub async fn chat_completion_with_usage(
        &self,
        user_id: i64,
        command: ModelChatCommand,
    ) -> Result<ModelChatResp, AppError> {
        let command = normalize_model_chat_command(command)?;
        if let Some(conversation_id) = command.conversation_id {
            ensure_model_chat_conversation_owner(
                &self.db,
                self.tenant_id,
                user_id,
                conversation_id,
            )
            .await?;
        }
        let conversation_id = command.conversation_id.unwrap_or_else(next_id);
        let route = self
            .resolve_route_for_purpose_with_route_id(
                ModelRoutePurpose::Chat,
                command.route_id.as_deref(),
            )
            .await?
            .ok_or_else(|| AppError::bad_request("LLM 模型环境变量未配置完整"))?;
        let mut response =
            execute_normalized_chat_completion_with_route(&route, &command, Some(conversation_id))
                .await?;
        response.cost_cents =
            estimate_model_chat_response_cost_cents(&self.db, self.tenant_id, &response).await?;
        let now = Utc::now().naive_utc();
        let history =
            model_chat_history_records(self.tenant_id, user_id, &command, &response, now)?;
        persist_model_chat_history(&self.db, &history).await?;
        let record = model_chat_usage_record(
            self.tenant_id,
            user_id,
            &response,
            Utc::now().naive_utc(),
            "ai.models.chat",
        );
        record_model_chat_usage(&self.db, &record).await?;
        Ok(response)
    }

    pub async fn chat_completion_for_chat_flow(
        &self,
        user_id: i64,
        command: ModelChatCommand,
    ) -> Result<ModelChatResp, AppError> {
        self.chat_completion_for_source(user_id, command, "ai.chatFlow.model")
            .await
    }

    pub async fn chat_completion_for_source(
        &self,
        user_id: i64,
        command: ModelChatCommand,
        source: &str,
    ) -> Result<ModelChatResp, AppError> {
        let command = normalize_model_chat_command(command)?;
        let route = self
            .resolve_route_for_purpose_with_route_id(
                ModelRoutePurpose::Chat,
                command.route_id.as_deref(),
            )
            .await?
            .ok_or_else(|| AppError::bad_request("LLM 模型环境变量未配置完整"))?;
        let mut response = execute_normalized_chat_completion_with_route(
            &route,
            &command,
            command.conversation_id,
        )
        .await?;
        response.cost_cents =
            estimate_model_chat_response_cost_cents(&self.db, self.tenant_id, &response).await?;
        let record = model_chat_usage_record(
            self.tenant_id,
            user_id,
            &response,
            Utc::now().naive_utc(),
            source,
        );
        record_model_chat_usage(&self.db, &record).await?;
        Ok(response)
    }

    pub async fn chat_completion_for_purpose(
        &self,
        purpose: ModelRoutePurpose,
        command: ModelChatCommand,
    ) -> Result<ModelChatResp, AppError> {
        let command = normalize_model_chat_command(command)?;
        let route = self
            .resolve_route_for_purpose_with_route_id(purpose, command.route_id.as_deref())
            .await?
            .ok_or_else(|| AppError::bad_request("LLM 模型环境变量未配置完整"))?;
        let mut response = self
            .execute_normalized_chat_completion_with_fallback(
                purpose,
                &route,
                &command,
                command.conversation_id,
            )
            .await?;
        response.cost_cents =
            estimate_model_chat_response_cost_cents(&self.db, self.tenant_id, &response).await?;
        Ok(response)
    }

    async fn execute_normalized_chat_completion_with_fallback(
        &self,
        purpose: ModelRoutePurpose,
        primary_route: &ModelRuntimeRoute,
        command: &ModelChatCommand,
        conversation_id: Option<i64>,
    ) -> Result<ModelChatResp, AppError> {
        let mut current_route = primary_route.clone();
        let mut visited_route_ids = HashSet::from([primary_route.route_id().to_owned()]);
        let mut fallback_hops = 0usize;
        let mut attempts = Vec::new();

        while fallback_hops <= MAX_MODEL_FALLBACK_HOPS {
            let fallback_plan = self
                .fallback_plan_for_purpose_with_route_id(purpose, Some(current_route.route_id()))
                .await?;
            let attempt_kind = if fallback_hops == 0 {
                "primary"
            } else {
                "fallback"
            };

            if fallback_plan
                .as_ref()
                .is_some_and(|plan| plan.decision.enabled)
            {
                let open_attempt = match self
                    .persistent_model_circuit_breaker_open_attempt(&current_route)
                    .await?
                {
                    Some(attempt) => Some(attempt),
                    None => model_circuit_breaker_open_attempt(&current_route),
                };
                if let Some(mut skipped_attempt) = open_attempt {
                    skipped_attempt.attempt_kind = attempt_kind.to_owned();
                    attempts.push(skipped_attempt);
                    let Some(next_route_id) =
                        model_enabled_fallback_route_id(fallback_plan.as_ref()).map(str::to_owned)
                    else {
                        return Err(AppError::bad_request("模型 fallback 路由不可用"));
                    };
                    if !model_fallback_chain_can_visit(
                        &visited_route_ids,
                        &next_route_id,
                        fallback_hops,
                    ) {
                        return Err(AppError::bad_request("模型 fallback 路由不可用"));
                    }
                    let Some(next_route) = self
                        .resolve_route_for_purpose_with_route_id(purpose, Some(&next_route_id))
                        .await?
                    else {
                        return Err(AppError::bad_request("模型 fallback 路由不可用"));
                    };
                    visited_route_ids.insert(next_route.route_id().to_owned());
                    current_route = next_route;
                    fallback_hops += 1;
                    continue;
                }
            }

            let attempt_started = Instant::now();
            let result = execute_normalized_chat_completion_with_route(
                &current_route,
                command,
                conversation_id,
            )
            .await;
            let provider_error = match result {
                Ok(mut response) => {
                    for attempt in &mut response.provider_attempts {
                        attempt.attempt_kind = attempt_kind.to_owned();
                    }
                    response.provider_attempts.splice(0..0, attempts);
                    return Ok(response);
                }
                Err(err) if model_provider_error_is_fallback_candidate(&err) => err,
                Err(err) => return Err(err),
            };

            let failed_attempt = model_provider_attempt_failed(
                attempt_kind,
                &current_route,
                &provider_error,
                attempt_started.elapsed().as_millis(),
            );
            if let Some(cooldown_seconds) =
                model_circuit_breaker_cooldown_seconds(fallback_plan.as_ref())
            {
                model_circuit_breaker_open(current_route.route_id(), cooldown_seconds);
                self.persistent_model_circuit_breaker_open(
                    current_route.route_id(),
                    cooldown_seconds,
                    &failed_attempt,
                )
                .await?;
            }
            attempts.push(failed_attempt);

            let Some(next_route_id) =
                model_enabled_fallback_route_id(fallback_plan.as_ref()).map(str::to_owned)
            else {
                return Err(provider_error);
            };
            if !model_fallback_chain_can_visit(&visited_route_ids, &next_route_id, fallback_hops) {
                return Err(provider_error);
            }
            let Some(next_route) = self
                .resolve_route_for_purpose_with_route_id(purpose, Some(&next_route_id))
                .await?
            else {
                return Err(AppError::bad_request("模型 fallback 路由不可用"));
            };
            visited_route_ids.insert(next_route.route_id().to_owned());
            current_route = next_route;
            fallback_hops += 1;
        }

        Err(AppError::bad_request("模型 fallback 链超过最大跳数"))
    }

    pub async fn list_chat_conversations(
        &self,
        user_id: i64,
    ) -> Result<Vec<ModelChatConversationResp>, AppError> {
        let rows = sqlx::query_as::<_, ModelChatConversationRow>(
            r#"
SELECT
    id,
    title,
    route_id,
    model,
    message_count,
    last_message_preview,
    create_time,
    COALESCE(update_time, create_time) AS update_time
FROM ai_model_chat_conversation
WHERE tenant_id = $1
  AND create_user = $2
ORDER BY COALESCE(update_time, create_time) DESC, id DESC
LIMIT $3;
"#,
        )
        .bind(self.tenant_id)
        .bind(user_id)
        .bind(MODEL_CHAT_HISTORY_LIMIT)
        .fetch_all(&self.db)
        .await?;

        Ok(rows
            .into_iter()
            .map(ModelChatConversationResp::from)
            .collect())
    }
}

#[derive(Debug)]
struct RuntimeRouteSummaryAccumulator {
    summary: ModelRuntimeRouteSummary,
    source_route_ids: Vec<String>,
    purpose_route_ids: BTreeMap<String, String>,
}

fn effective_runtime_summary_from_routes(
    routes: Vec<ModelRuntimeRoute>,
    missing_env: Vec<String>,
) -> ModelRuntimeSummary {
    let mut groups: Vec<(String, RuntimeRouteSummaryAccumulator)> = Vec::new();

    for route in routes {
        let group_key = runtime_route_physical_key(&route);
        let source_route_id = route.route_id().to_owned();
        let route_summary = route.summary();
        if let Some((_, accumulator)) = groups
            .iter_mut()
            .find(|(existing_key, _)| existing_key == &group_key)
        {
            accumulator.source_route_ids.push(source_route_id.clone());
            merge_unique_model_purposes(&mut accumulator.summary.purposes, &route_summary.purposes);
            merge_unique_strings(&mut accumulator.summary.env_keys, &route_summary.env_keys);
            for purpose in &route_summary.purposes {
                accumulator
                    .purpose_route_ids
                    .entry(purpose.as_str().to_owned())
                    .or_insert_with(|| source_route_id.clone());
            }
            accumulator.summary.route_id =
                merged_runtime_route_id(&accumulator.source_route_ids, accumulator.summary.target);
            accumulator.summary.purposes =
                sort_runtime_purposes(accumulator.summary.purposes.clone());
            accumulator.summary.purpose_route_ids = accumulator.purpose_route_ids.clone();
            continue;
        }

        let mut purpose_route_ids = BTreeMap::new();
        for purpose in &route_summary.purposes {
            purpose_route_ids.insert(purpose.as_str().to_owned(), source_route_id.clone());
        }
        let mut summary = route_summary;
        summary.purposes = sort_runtime_purposes(summary.purposes);
        summary.purpose_route_ids = purpose_route_ids.clone();
        groups.push((
            group_key,
            RuntimeRouteSummaryAccumulator {
                summary,
                source_route_ids: vec![source_route_id],
                purpose_route_ids,
            },
        ));
    }

    ModelRuntimeSummary {
        routes: groups
            .into_iter()
            .map(|(_, accumulator)| accumulator.summary)
            .collect(),
        missing_env,
    }
}

fn runtime_route_physical_key(route: &ModelRuntimeRoute) -> String {
    [
        route.target().as_str(),
        route.kind().as_str(),
        route.provider().as_str(),
        route.model().unwrap_or_default(),
        route.base_url(),
        route.endpoint(),
        route.api_key(),
    ]
    .join("\u{1f}")
}

fn merge_unique_model_purposes(
    current: &mut Vec<ModelRoutePurpose>,
    incoming: &[ModelRoutePurpose],
) {
    for purpose in incoming {
        if !current.contains(purpose) {
            current.push(*purpose);
        }
    }
}

fn merge_unique_strings(current: &mut Vec<String>, incoming: &[String]) {
    for value in incoming {
        if !current.contains(value) {
            current.push(value.clone());
        }
    }
}

fn merged_runtime_route_id(route_ids: &[String], target: ModelRuntimeTarget) -> String {
    if route_ids.len() <= 1 {
        return route_ids
            .first()
            .cloned()
            .unwrap_or_else(|| format!("runtime.{}", target.as_str()));
    }
    let Some(first) = route_ids.first() else {
        return format!("runtime.{}", target.as_str());
    };
    let Some(prefix) = first.rsplit_once('.').map(|(prefix, _)| prefix) else {
        return first.clone();
    };
    if route_ids
        .iter()
        .all(|route_id| route_id.starts_with(prefix) && route_id[prefix.len()..].starts_with('.'))
    {
        prefix.to_owned()
    } else {
        first.clone()
    }
}

fn sort_runtime_purposes(mut purposes: Vec<ModelRoutePurpose>) -> Vec<ModelRoutePurpose> {
    purposes.sort_by_key(|purpose| runtime_purpose_order(*purpose));
    purposes.dedup();
    purposes
}

fn runtime_purpose_order(purpose: ModelRoutePurpose) -> usize {
    match purpose {
        ModelRoutePurpose::Chat => 0,
        ModelRoutePurpose::RagAnswer => 1,
        ModelRoutePurpose::QueryRewrite => 2,
        ModelRoutePurpose::Embedding => 3,
        ModelRoutePurpose::Rerank => 4,
        ModelRoutePurpose::EvalJudge => 5,
        ModelRoutePurpose::CodeAgent => 6,
        ModelRoutePurpose::GuardianReview => 7,
        ModelRoutePurpose::MediaGeneration => 8,
    }
}

async fn execute_chat_completion(command: ModelChatCommand) -> Result<ModelChatResp, AppError> {
    let command = normalize_model_chat_command(command)?;
    execute_normalized_chat_completion(&command, command.conversation_id).await
}

async fn execute_normalized_chat_completion(
    command: &ModelChatCommand,
    conversation_id: Option<i64>,
) -> Result<ModelChatResp, AppError> {
    let config = ModelRuntimeConfig::from_env();
    let route = config
        .route(ModelRuntimeTarget::Llm)
        .filter(|route| {
            command
                .route_id
                .as_deref()
                .map_or(true, |route_id| route.route_id() == route_id)
        })
        .ok_or_else(|| AppError::bad_request("LLM 模型环境变量未配置完整"))?;
    execute_normalized_chat_completion_with_route(route, command, conversation_id).await
}

async fn execute_normalized_chat_completion_with_route(
    route: &ModelRuntimeRoute,
    command: &ModelChatCommand,
    conversation_id: Option<i64>,
) -> Result<ModelChatResp, AppError> {
    let client = reqwest::Client::builder()
        .timeout(MODEL_CHAT_TIMEOUT)
        .build()
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let payload = model_chat_request_payload(route, command);
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

    model_chat_response_from_provider(route, body, started.elapsed().as_millis(), conversation_id)
}

fn normalize_model_chat_command(
    mut command: ModelChatCommand,
) -> Result<ModelChatCommand, AppError> {
    if matches!(command.conversation_id, Some(value) if value <= 0) {
        return Err(AppError::bad_request("会话 ID 不合法"));
    }
    command.route_id = normalize_optional_runtime_route_id(command.route_id)?;
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
    if command.file_contexts.len() > MAX_MODEL_CHAT_FILE_CONTEXTS {
        return Err(AppError::bad_request(format!(
            "文件上下文不能超过 {MAX_MODEL_CHAT_FILE_CONTEXTS} 个"
        )));
    }
    for file in &mut command.file_contexts {
        file.name = file.name.trim().to_owned();
        file.content_type = file.content_type.trim().to_owned();
        file.content = file.content.trim().to_owned();
        if file.name.is_empty() {
            return Err(AppError::bad_request("文件名称不能为空"));
        }
        if file.content_type.is_empty() {
            file.content_type = "text/plain".to_owned();
        }
        if file.content.is_empty() {
            return Err(AppError::bad_request("文件内容不能为空"));
        }
        ensure_max_chars("文件名称", &file.name, 255)?;
        ensure_max_chars("文件内容类型", &file.content_type, 128)?;
        ensure_max_chars("文件内容", &file.content, MAX_MODEL_CHAT_FILE_CONTEXT_CHARS)?;
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

fn normalize_optional_runtime_route_id(
    route_id: Option<String>,
) -> Result<Option<String>, AppError> {
    let route_id = route_id
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(route_id) = &route_id {
        ensure_max_chars("模型路由", route_id, 128)?;
    }
    Ok(route_id)
}

fn model_chat_request_payload(route: &ModelRuntimeRoute, command: &ModelChatCommand) -> Value {
    let mut messages = Vec::new();
    if !command.file_contexts.is_empty() {
        messages.push(json!({
            "role": "system",
            "content": model_chat_file_context_prompt(&command.file_contexts),
        }));
    }
    messages.extend(
        command
            .messages
            .iter()
            .map(|message| {
                json!({
                    "role": message.role,
                    "content": message.content,
                })
            })
            .collect::<Vec<_>>(),
    );

    let mut payload = json!({
        "model": route.model().unwrap_or_default(),
        "messages": messages,
        "temperature": command.temperature.unwrap_or(DEFAULT_MODEL_CHAT_TEMPERATURE),
        "max_tokens": command.max_tokens.unwrap_or(DEFAULT_MODEL_CHAT_MAX_TOKENS),
        "stream": false,
    });
    if let Some(response_format) = &command.response_format {
        payload["response_format"] = response_format.clone();
    }
    if let Some(metadata) = model_chat_provider_metadata(route, command) {
        payload["metadata"] = metadata;
    }
    payload
}

fn model_chat_provider_metadata(
    route: &ModelRuntimeRoute,
    command: &ModelChatCommand,
) -> Option<Value> {
    if !model_chat_route_supports_provider_metadata(route) {
        return None;
    }

    let metadata = command.request_metadata.as_ref()?;
    let mut payload = serde_json::Map::from_iter([(
        "request_kind".to_owned(),
        json!(metadata.request_kind.as_str()),
    )]);

    if let Some(compaction) = &metadata.compaction {
        payload.insert(
            "compaction_implementation".to_owned(),
            json!(compaction.implementation),
        );
        payload.insert("compaction_trigger".to_owned(), json!(compaction.trigger));
        payload.insert("compaction_reason".to_owned(), json!(compaction.reason));
        payload.insert("compaction_phase".to_owned(), json!(compaction.phase));
        payload.insert("compaction_strategy".to_owned(), json!(compaction.strategy));
        payload.insert(
            "compaction_window_id".to_owned(),
            json!(compaction.window_id.to_string()),
        );
        payload.insert(
            "input_history_count".to_owned(),
            json!(compaction.input_history_count.to_string()),
        );
        payload.insert(
            "retained_history_count".to_owned(),
            json!(compaction.retained_history_count.to_string()),
        );
        payload.insert(
            "compacted_item_count".to_owned(),
            json!(compaction.compacted_item_count.to_string()),
        );
        payload.insert(
            "retained_item_count".to_owned(),
            json!(compaction.retained_item_count.to_string()),
        );
        payload.insert(
            "tool_codes".to_owned(),
            json!(compaction.tool_codes.join(",")),
        );
    }

    Some(Value::Object(payload))
}

fn model_chat_route_supports_provider_metadata(route: &ModelRuntimeRoute) -> bool {
    matches!(
        route.provider(),
        ModelProviderType::OpenAiCompatible
            | ModelProviderType::AzureOpenAi
            | ModelProviderType::LocalRuntime
    )
}

fn model_chat_file_context_prompt(files: &[ModelChatFileContext]) -> String {
    let mut sections = vec![
        "Use the following user-provided file context when it is relevant. If the files do not contain enough evidence, say so.".to_owned(),
    ];
    for file in files {
        sections.push(format!(
            "[File: {} | {}]\n{}",
            file.name, file.content_type, file.content
        ));
    }
    sections.join("\n\n")
}

fn model_chat_response_from_provider(
    route: &ModelRuntimeRoute,
    body: Value,
    latency_ms: u128,
    conversation_id: Option<i64>,
) -> Result<ModelChatResp, AppError> {
    let answer = model_chat_answer_from_provider_body(&body)
        .ok_or_else(|| AppError::bad_request("LLM 响应为空"))?;
    Ok(ModelChatResp {
        conversation_id,
        answer,
        route_id: route.summary().route_id,
        provider: route.provider().as_str().to_owned(),
        model: route.model().map(str::to_owned),
        latency_ms,
        usage: normalize_model_provider_usage(&body),
        cost_cents: None,
        provider_attempts: vec![model_provider_attempt_succeeded(
            "primary", route, latency_ms,
        )],
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelProviderErrorClass {
    kind: &'static str,
    http_status: Option<u16>,
    fallback_candidate: bool,
}

fn model_provider_attempt_succeeded(
    attempt_kind: &str,
    route: &ModelRuntimeRoute,
    latency_ms: u128,
) -> ModelProviderAttempt {
    ModelProviderAttempt {
        attempt_kind: attempt_kind.to_owned(),
        route_id: route.route_id().to_owned(),
        provider: route.provider().as_str().to_owned(),
        model: route.model().map(str::to_owned),
        status: "succeeded".to_owned(),
        latency_ms: u128_to_i64(latency_ms),
        error_kind: None,
        http_status: None,
        message: None,
    }
}

fn model_provider_attempt_failed(
    attempt_kind: &str,
    route: &ModelRuntimeRoute,
    error: &AppError,
    latency_ms: u128,
) -> ModelProviderAttempt {
    let class = model_provider_error_class(error);
    ModelProviderAttempt {
        attempt_kind: attempt_kind.to_owned(),
        route_id: route.route_id().to_owned(),
        provider: route.provider().as_str().to_owned(),
        model: route.model().map(str::to_owned),
        status: "failed".to_owned(),
        latency_ms: u128_to_i64(latency_ms),
        error_kind: Some(class.kind.to_owned()),
        http_status: class.http_status,
        message: Some(model_provider_error_message(error)),
    }
}

fn model_provider_attempt_circuit_open(
    route: &ModelRuntimeRoute,
    remaining: Duration,
) -> ModelProviderAttempt {
    ModelProviderAttempt {
        attempt_kind: "primary".to_owned(),
        route_id: route.route_id().to_owned(),
        provider: route.provider().as_str().to_owned(),
        model: route.model().map(str::to_owned),
        status: "skipped".to_owned(),
        latency_ms: 0,
        error_kind: Some("circuit_open".to_owned()),
        http_status: None,
        message: Some(format!(
            "model route circuit breaker open for {}ms",
            remaining.as_millis().min(i64::MAX as u128)
        )),
    }
}

fn model_provider_error_is_fallback_candidate(error: &AppError) -> bool {
    model_provider_error_class(error).fallback_candidate
}

fn model_provider_error_class(error: &AppError) -> ModelProviderErrorClass {
    let message = model_provider_error_message(error);
    let http_status = model_provider_error_http_status(&message);
    let kind = match error {
        AppError::BadRequest(_) if http_status.is_some() => "provider_http",
        AppError::BadRequest(_) if model_provider_error_is_timeout(&message) => "provider_timeout",
        AppError::BadRequest(_) => "invalid_model_request",
        AppError::Unauthorized => "unauthorized",
        AppError::Forbidden => "forbidden",
        AppError::NotFound => "not_found",
        AppError::Conflict(_) => "conflict",
        AppError::Sqlx(_) | AppError::Io(_) | AppError::Anyhow(_) => "provider_transport",
    };
    let fallback_candidate = match kind {
        "provider_http" => http_status.is_some_and(|status| status == 429 || status >= 500),
        "provider_timeout" | "provider_transport" => true,
        _ => false,
    };

    ModelProviderErrorClass {
        kind,
        http_status,
        fallback_candidate,
    }
}

fn model_provider_error_message(error: &AppError) -> String {
    match error {
        AppError::BadRequest(message) | AppError::Conflict(message) => message.clone(),
        AppError::Unauthorized => "unauthorized model request".to_owned(),
        AppError::Forbidden => "forbidden model request".to_owned(),
        AppError::NotFound => "model route not found".to_owned(),
        AppError::Sqlx(_) | AppError::Io(_) | AppError::Anyhow(_) => {
            "model provider transport error".to_owned()
        }
    }
}

fn model_provider_error_http_status(message: &str) -> Option<u16> {
    let marker_index = message.find("HTTP")?;
    let digits = message[marker_index + "HTTP".len()..]
        .trim_start()
        .chars()
        .take_while(|char| char.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u16>().ok()
}

fn model_provider_error_is_timeout(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("timeout") || message.contains("timed out") || message.contains("超时")
}

fn model_chat_answer_from_provider_body(body: &Value) -> Option<String> {
    for pointer in [
        "/choices/0/message/content",
        "/choices/0/text",
        "/output_text",
    ] {
        if let Some(value) = body.pointer(pointer) {
            if let Some(answer) = model_chat_text_from_value(value) {
                return Some(answer);
            }
        }
    }
    None
}

fn model_chat_text_from_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => non_empty_model_chat_text(text),
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|part| !part.is_empty())
                })
                .collect::<Vec<_>>()
                .join("\n");
            non_empty_model_chat_text(&text)
        }
        _ => None,
    }
}

fn non_empty_model_chat_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

impl From<ModelChatConversationRow> for ModelChatConversationResp {
    fn from(row: ModelChatConversationRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            route_id: row.route_id,
            model: row.model,
            message_count: row.message_count,
            last_message_preview: row.last_message_preview,
            create_time: format_datetime(row.create_time),
            update_time: format_datetime(row.update_time),
        }
    }
}

fn model_ops_summary_from_rows(
    rows: Vec<ModelRouteOpsSummaryRow>,
    alert_rows: Vec<ModelOpsAlertRow>,
    now: NaiveDateTime,
) -> ModelOpsSummaryResp {
    let route_count = rows.len();
    let active_route_count = rows.iter().filter(|row| row.status == 1).count();
    let mut open_breaker_count = 0usize;
    let mut degraded_route_count = 0usize;
    let mut usage_24h = ModelOpsUsageSummaryResp::default();
    let mut weighted_latency_sum = 0.0f64;
    let mut weighted_latency_count = 0i64;
    let mut routes = Vec::with_capacity(rows.len());
    let mut route_alert_counts = HashMap::<String, usize>::new();
    for alert in &alert_rows {
        if let Some(route_code) = &alert.route_code {
            *route_alert_counts.entry(route_code.clone()).or_default() += 1;
        }
    }

    for row in rows {
        let breaker_remaining_ms = row
            .breaker_opened_until
            .map(|opened_until| (opened_until - now).num_milliseconds().max(0))
            .unwrap_or(0);
        let breaker_open = breaker_remaining_ms > 0;
        let active_alert_count = route_alert_counts
            .get(&row.route_code)
            .copied()
            .unwrap_or_default();
        let health_degraded = row
            .last_health_status
            .as_deref()
            .is_some_and(|status| status != "ok");
        let degraded = breaker_open || health_degraded || active_alert_count > 0;
        if breaker_open {
            open_breaker_count += 1;
        }
        if degraded {
            degraded_route_count += 1;
        }

        usage_24h.request_count += row.request_count_24h;
        usage_24h.total_tokens += row.total_tokens_24h;
        usage_24h.cost_cents += row.cost_cents_24h;
        if let Some(avg_latency_ms) = row.avg_latency_ms_24h {
            if row.request_count_24h > 0 {
                weighted_latency_sum += avg_latency_ms * row.request_count_24h as f64;
                weighted_latency_count += row.request_count_24h;
            }
        }

        let usage = ModelOpsUsageSummaryResp {
            request_count: row.request_count_24h,
            total_tokens: row.total_tokens_24h,
            cost_cents: row.cost_cents_24h,
            avg_latency_ms: row.avg_latency_ms_24h,
        };
        routes.push(ModelRouteOpsSummaryResp {
            route_id: row.route_code,
            route_purpose: row.route_purpose,
            provider: row.provider_code,
            provider_type: row.provider_type,
            model: row.model_name,
            network_zone: row.network_zone,
            status: row.status,
            breaker_open,
            breaker_remaining_ms,
            breaker_opened_until: row.breaker_opened_until.map(format_datetime),
            last_health_status: row.last_health_status,
            last_health_checked_at: row.last_health_checked_at.map(format_datetime),
            last_health_latency_ms: row.last_health_latency_ms,
            active_alert_count,
            degraded,
            usage_24h: usage,
        });
    }

    if weighted_latency_count > 0 {
        usage_24h.avg_latency_ms = Some(weighted_latency_sum / weighted_latency_count as f64);
    }

    ModelOpsSummaryResp {
        route_count,
        active_route_count,
        open_breaker_count,
        degraded_route_count,
        active_alert_count: alert_rows.len(),
        usage_24h,
        alerts: alert_rows
            .into_iter()
            .map(ModelOpsAlertResp::from)
            .collect(),
        routes,
    }
}

impl From<ModelOpsAlertRow> for ModelOpsAlertResp {
    fn from(row: ModelOpsAlertRow) -> Self {
        let message = row
            .event_payload
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();

        Self {
            alert_key: row.alert_key,
            alert_kind: row.alert_kind,
            severity: row.severity,
            status: row.status,
            route_id: row.route_code,
            route_purpose: row.route_purpose,
            provider: row.provider_code,
            model: row.model_name,
            source_ref: row.source_ref,
            message,
            first_seen_at: format_datetime(row.first_seen_at),
            last_seen_at: format_datetime(row.last_seen_at),
            event_payload: row.event_payload,
        }
    }
}

fn model_health_check_record_from_result(
    tenant_id: i64,
    user_id: i64,
    route_ids: Option<(i64, i64, i64)>,
    result: &ModelHealthCheckResult,
    now: NaiveDateTime,
) -> ModelHealthCheckSaveRecord {
    let (route_id, provider_id, model_profile_id) = route_ids
        .map(|(route_id, provider_id, model_profile_id)| {
            (Some(route_id), Some(provider_id), Some(model_profile_id))
        })
        .unwrap_or((None, None, None));
    let error_message = (!result.ok).then(|| result.message.clone());
    let detail = json!({
        "target": result.target.as_str(),
        "configured": result.configured,
        "endpoint": result.endpoint,
        "maskedApiKey": result.masked_api_key,
        "message": result.message,
        "detail": result.detail,
    });

    ModelHealthCheckSaveRecord {
        id: next_id(),
        tenant_id,
        route_id,
        provider_id,
        model_profile_id,
        status: if result.ok {
            "ok".to_owned()
        } else {
            preview_chars(&result.message, 32)
        },
        http_status: result.http_status.map(i32::from),
        latency_ms: Some(u128_to_i64(result.latency_ms)),
        checked_at: now,
        error_message,
        detail,
        user_id,
    }
}

fn model_ops_alert_key_from_health_check(
    tenant_id: i64,
    route_ids: Option<(i64, i64, i64)>,
    result: &ModelHealthCheckResult,
) -> String {
    match route_ids {
        Some((route_id, _, _)) => {
            format!("model_health:{}:route:{route_id}", result.target.as_str())
        }
        None => format!("model_health:{}:tenant:{tenant_id}", result.target.as_str()),
    }
}

fn model_ops_alert_record_from_health_check(
    tenant_id: i64,
    user_id: i64,
    route_ids: Option<(i64, i64, i64)>,
    result: &ModelHealthCheckResult,
    health_check_id: i64,
    now: NaiveDateTime,
) -> ModelOpsAlertSaveRecord {
    let (route_id, provider_id, model_profile_id) = route_ids
        .map(|(route_id, provider_id, model_profile_id)| {
            (Some(route_id), Some(provider_id), Some(model_profile_id))
        })
        .unwrap_or((None, None, None));
    let source_ref = format!("health_check:{health_check_id}");
    let event_payload = json!({
        "healthCheckId": health_check_id,
        "target": result.target.as_str(),
        "configured": result.configured,
        "routeId": route_id,
        "providerId": provider_id,
        "modelProfileId": model_profile_id,
        "endpoint": result.endpoint,
        "maskedApiKey": result.masked_api_key,
        "httpStatus": result.http_status,
        "latencyMs": result.latency_ms,
        "message": result.message,
        "detail": result.detail,
    });

    ModelOpsAlertSaveRecord {
        id: next_id(),
        tenant_id,
        alert_key: model_ops_alert_key_from_health_check(tenant_id, route_ids, result),
        alert_kind: "model_health".to_owned(),
        severity: "critical".to_owned(),
        status: "active".to_owned(),
        route_id,
        provider_id,
        model_profile_id,
        source_ref,
        event_payload,
        first_seen_at: now,
        last_seen_at: now,
        user_id,
    }
}

fn model_ops_alert_delivery_message(alert: &ModelOpsAlertDeliveryCandidateRow) -> String {
    let message = alert
        .event_payload
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("model ops alert");
    let route = alert.route_code.as_deref().unwrap_or("-");
    let purpose = alert.route_purpose.as_deref().unwrap_or("-");
    let provider = alert.provider_code.as_deref().unwrap_or("-");
    let model = alert.model_name.as_deref().unwrap_or("-");

    format!(
        "Novex Model Alert\nseverity: {}\nkind: {}\nalertKey: {}\nroute: {}\npurpose: {}\nprovider: {}\nmodel: {}\nsource: {}\nmessage: {}\nfirstSeenAt: {}\nlastSeenAt: {}",
        alert.severity,
        alert.alert_kind,
        alert.alert_key,
        route,
        purpose,
        provider,
        model,
        alert.source_ref,
        message,
        format_datetime(alert.first_seen_at),
        format_datetime(alert.last_seen_at),
    )
}

fn model_ops_alert_delivery_request_payload(alert: &ModelOpsAlertDeliveryCandidateRow) -> Value {
    let webhook_payload =
        FeishuTextMessage::new(model_ops_alert_delivery_message(alert)).to_webhook_payload();

    json!({
        "toolCode": MODEL_ALERT_DELIVERY_TOOL_CODE,
        "channel": MODEL_ALERT_DELIVERY_CHANNEL_FEISHU,
        "alertId": alert.alert_id,
        "alertKey": alert.alert_key,
        "webhookPayload": webhook_payload,
    })
}

fn model_ops_alert_delivery_dry_run_result(
    alert: &ModelOpsAlertDeliveryCandidateRow,
) -> ModelOpsAlertDeliveryResult {
    ModelOpsAlertDeliveryResult {
        status: "dry_run".to_owned(),
        dry_run: true,
        request_payload: model_ops_alert_delivery_request_payload(alert),
        response_payload: json!({
            "status": "dry_run",
            "channel": MODEL_ALERT_DELIVERY_CHANNEL_FEISHU,
            "message": "FEISHU_WEBHOOK_URL is not configured",
        }),
        error_message: None,
    }
}

async fn model_ops_alert_delivery_candidates(
    db: &PgPool,
    limit: i64,
) -> Result<Vec<ModelOpsAlertDeliveryCandidateRow>, AppError> {
    Ok(sqlx::query_as::<_, ModelOpsAlertDeliveryCandidateRow>(
        r#"
SELECT
    alert.id AS alert_id,
    alert.tenant_id,
    alert.alert_key,
    alert.alert_kind,
    alert.severity,
    route.code AS route_code,
    route.route_purpose,
    provider.code AS provider_code,
    profile.model_name,
    COALESCE(alert.source_ref, '') AS source_ref,
    alert.event_payload,
    alert.first_seen_at,
    alert.last_seen_at
FROM ai_model_ops_alert alert
LEFT JOIN ai_model_route route
    ON route.id = alert.route_id
   AND route.tenant_id = alert.tenant_id
LEFT JOIN ai_model_profile profile
    ON profile.id = alert.model_profile_id
   AND profile.tenant_id = alert.tenant_id
LEFT JOIN ai_model_deployment deployment
    ON deployment.id = profile.deployment_id
   AND deployment.tenant_id = alert.tenant_id
LEFT JOIN ai_model_provider provider
    ON provider.id = COALESCE(alert.provider_id, deployment.provider_id)
   AND provider.tenant_id = alert.tenant_id
WHERE alert.resolved_at IS NULL
  AND NOT EXISTS (
      SELECT 1
      FROM ai_model_ops_alert_delivery delivery
      WHERE delivery.tenant_id = alert.tenant_id
        AND delivery.alert_id = alert.id
        AND delivery.channel = $1
        AND delivery.status IN ('sent', 'dry_run')
  )
ORDER BY alert.last_seen_at DESC, alert.id DESC
LIMIT $2;
"#,
    )
    .bind(MODEL_ALERT_DELIVERY_CHANNEL_FEISHU)
    .bind(limit)
    .fetch_all(db)
    .await?)
}

async fn deliver_model_ops_alert_candidate(
    db: &PgPool,
    capability_repo: &AiCapabilityRepository,
    candidate: &ModelOpsAlertDeliveryCandidateRow,
) -> Result<ModelOpsAlertDeliveryResult, AppError> {
    let result = execute_model_ops_alert_feishu_delivery(candidate).await;
    let now = Utc::now().naive_utc();
    let audit_id =
        record_model_ops_alert_delivery_audit(capability_repo, candidate, &result, now).await?;
    let record = ModelOpsAlertDeliverySaveRecord {
        id: next_id(),
        tenant_id: candidate.tenant_id,
        alert_id: candidate.alert_id,
        alert_key: candidate.alert_key.clone(),
        channel: MODEL_ALERT_DELIVERY_CHANNEL_FEISHU.to_owned(),
        status: result.status.clone(),
        dry_run: result.dry_run,
        tool_call_audit_id: Some(audit_id),
        request_payload: result.request_payload.clone(),
        response_payload: result.response_payload.clone(),
        error_message: result.error_message.clone(),
        user_id: 1,
        now,
    };
    persist_model_ops_alert_delivery(db, &record).await?;

    Ok(result)
}

async fn execute_model_ops_alert_feishu_delivery(
    alert: &ModelOpsAlertDeliveryCandidateRow,
) -> ModelOpsAlertDeliveryResult {
    let request_payload = model_ops_alert_delivery_request_payload(alert);
    let Some(config) = ModelOpsAlertFeishuConfig::from_env() else {
        return model_ops_alert_delivery_dry_run_result(alert);
    };
    let webhook_payload = request_payload
        .get("webhookPayload")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let client = match reqwest::Client::builder()
        .timeout(MODEL_ALERT_DELIVERY_TIMEOUT)
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            let error = format!("Feishu 客户端初始化失败: {error}");
            return model_ops_alert_delivery_failed_result(request_payload, error);
        }
    };
    let response = match client
        .post(&config.webhook_url)
        .json(&webhook_payload)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let error = format!("Feishu 告警发送失败: {error}");
            return model_ops_alert_delivery_failed_result(request_payload, error);
        }
    };
    let status = response.status();
    let response_payload = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !status.is_success()
        || model_ops_alert_feishu_response_code(&response_payload).is_some_and(|code| code != 0)
    {
        let error = format!(
            "Feishu 告警发送失败: HTTP {status}, code {:?}",
            model_ops_alert_feishu_response_code(&response_payload)
        );
        return ModelOpsAlertDeliveryResult {
            status: "failed".to_owned(),
            dry_run: false,
            request_payload,
            response_payload,
            error_message: Some(error),
        };
    }

    ModelOpsAlertDeliveryResult {
        status: "sent".to_owned(),
        dry_run: false,
        request_payload,
        response_payload: json!({
            "status": "sent",
            "channel": MODEL_ALERT_DELIVERY_CHANNEL_FEISHU,
            "providerResponse": response_payload,
        }),
        error_message: None,
    }
}

fn model_ops_alert_delivery_failed_result(
    request_payload: Value,
    error: String,
) -> ModelOpsAlertDeliveryResult {
    ModelOpsAlertDeliveryResult {
        status: "failed".to_owned(),
        dry_run: false,
        request_payload,
        response_payload: json!({
            "status": "failed",
            "channel": MODEL_ALERT_DELIVERY_CHANNEL_FEISHU,
            "error": error,
        }),
        error_message: Some(error),
    }
}

fn model_ops_alert_delivery_audit_status(result: &ModelOpsAlertDeliveryResult) -> String {
    if result.status == "failed" {
        "failed".to_owned()
    } else {
        "succeeded".to_owned()
    }
}

async fn record_model_ops_alert_delivery_audit(
    capability_repo: &AiCapabilityRepository,
    candidate: &ModelOpsAlertDeliveryCandidateRow,
    result: &ModelOpsAlertDeliveryResult,
    now: NaiveDateTime,
) -> Result<i64, AppError> {
    let tool = capability_repo
        .find_tool_by_code(candidate.tenant_id, MODEL_ALERT_DELIVERY_TOOL_CODE)
        .await?
        .ok_or(AppError::NotFound)?;
    let audit_id = next_id();
    capability_repo
        .create_tool_call_audit(&ToolAuditSaveRecord {
            id: audit_id,
            tenant_id: candidate.tenant_id,
            tool_id: tool.id,
            tool_code: tool.code,
            caller_kind: "model_ops_alert_delivery".to_owned(),
            caller_id: Some(candidate.alert_id),
            request_payload: result.request_payload.clone(),
            response_payload: result.response_payload.clone(),
            status: model_ops_alert_delivery_audit_status(result),
            dry_run: result.dry_run,
            risk_level: tool.risk_level,
            permission_code: tool.permission_code,
            error_message: result.error_message.clone(),
            user_id: 1,
            now,
        })
        .await?;

    Ok(audit_id)
}

async fn persist_model_ops_alert_delivery(
    db: &PgPool,
    record: &ModelOpsAlertDeliverySaveRecord,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
INSERT INTO ai_model_ops_alert_delivery (
    id, tenant_id, alert_id, alert_key, channel, status, dry_run,
    tool_call_audit_id, request_payload, response_payload, error_message,
    create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6, $7,
    $8, $9, $10, $11,
    $12, $13
);
"#,
    )
    .bind(record.id)
    .bind(record.tenant_id)
    .bind(record.alert_id)
    .bind(&record.alert_key)
    .bind(&record.channel)
    .bind(&record.status)
    .bind(record.dry_run)
    .bind(record.tool_call_audit_id)
    .bind(&record.request_payload)
    .bind(&record.response_payload)
    .bind(&record.error_message)
    .bind(record.user_id)
    .bind(record.now)
    .execute(db)
    .await?;

    Ok(())
}

fn model_ops_alert_feishu_response_code(value: &Value) -> Option<i64> {
    value
        .get("code")
        .or_else(|| value.get("StatusCode"))
        .and_then(Value::as_i64)
}

async fn persist_model_health_check_record(
    db: &PgPool,
    record: &ModelHealthCheckSaveRecord,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
INSERT INTO ai_model_health_check (
    id, tenant_id, route_id, provider_id, model_profile_id, status,
    http_status, latency_ms, checked_at, error_message, detail, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6,
    $7, $8, $9, $10, $11, $12, $9
);
"#,
    )
    .bind(record.id)
    .bind(record.tenant_id)
    .bind(record.route_id)
    .bind(record.provider_id)
    .bind(record.model_profile_id)
    .bind(&record.status)
    .bind(record.http_status)
    .bind(record.latency_ms)
    .bind(record.checked_at)
    .bind(record.error_message.as_deref())
    .bind(&record.detail)
    .bind(record.user_id)
    .execute(db)
    .await?;

    Ok(())
}

async fn record_model_ops_alert_for_health_check(
    db: &PgPool,
    tenant_id: i64,
    user_id: i64,
    route_ids: Option<(i64, i64, i64)>,
    result: &ModelHealthCheckResult,
    health_record: &ModelHealthCheckSaveRecord,
) -> Result<(), AppError> {
    let alert_key = model_ops_alert_key_from_health_check(tenant_id, route_ids, result);
    if result.ok {
        return resolve_model_ops_alert(
            db,
            tenant_id,
            &alert_key,
            user_id,
            health_record.checked_at,
            "model health check recovered",
        )
        .await;
    }

    let alert = model_ops_alert_record_from_health_check(
        tenant_id,
        user_id,
        route_ids,
        result,
        health_record.id,
        health_record.checked_at,
    );
    upsert_model_ops_alert(db, &alert).await
}

async fn upsert_model_ops_alert(
    db: &PgPool,
    record: &ModelOpsAlertSaveRecord,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
INSERT INTO ai_model_ops_alert (
    id, tenant_id, alert_key, alert_kind, severity, status, route_id,
    provider_id, model_profile_id, source_ref, event_payload, first_seen_at,
    last_seen_at, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6, $7,
    $8, $9, $10, $11, $12,
    $13, $14, $12
)
ON CONFLICT (tenant_id, alert_key) WHERE resolved_at IS NULL
DO UPDATE SET
    alert_kind = EXCLUDED.alert_kind,
    severity = EXCLUDED.severity,
    status = EXCLUDED.status,
    route_id = EXCLUDED.route_id,
    provider_id = EXCLUDED.provider_id,
    model_profile_id = EXCLUDED.model_profile_id,
    source_ref = EXCLUDED.source_ref,
    event_payload = EXCLUDED.event_payload,
    last_seen_at = EXCLUDED.last_seen_at,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.last_seen_at;
"#,
    )
    .bind(record.id)
    .bind(record.tenant_id)
    .bind(&record.alert_key)
    .bind(&record.alert_kind)
    .bind(&record.severity)
    .bind(&record.status)
    .bind(record.route_id)
    .bind(record.provider_id)
    .bind(record.model_profile_id)
    .bind(&record.source_ref)
    .bind(&record.event_payload)
    .bind(record.first_seen_at)
    .bind(record.last_seen_at)
    .bind(record.user_id)
    .execute(db)
    .await?;

    Ok(())
}

async fn resolve_model_ops_alert(
    db: &PgPool,
    tenant_id: i64,
    alert_key: &str,
    user_id: i64,
    resolved_at: NaiveDateTime,
    resolve_message: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
UPDATE ai_model_ops_alert
SET status = 'resolved',
    resolved_at = $4,
    resolve_message = $5,
    update_user = $3,
    update_time = $4
WHERE tenant_id = $1
  AND alert_key = $2
  AND resolved_at IS NULL;
"#,
    )
    .bind(tenant_id)
    .bind(alert_key)
    .bind(user_id)
    .bind(resolved_at)
    .bind(resolve_message)
    .execute(db)
    .await?;

    Ok(())
}

fn model_chat_history_records(
    tenant_id: i64,
    user_id: i64,
    command: &ModelChatCommand,
    response: &ModelChatResp,
    now: NaiveDateTime,
) -> Result<ModelChatHistorySaveRecords, AppError> {
    let conversation_id = response
        .conversation_id
        .ok_or_else(|| AppError::bad_request("会话 ID 不合法"))?;
    let user_message = latest_user_message(command)
        .ok_or_else(|| AppError::bad_request("至少需要一条用户消息"))?;
    let assistant_token_count = response
        .usage
        .completion_tokens
        .unwrap_or_else(|| estimate_model_text_tokens(&response.answer) as i64)
        .max(0)
        .min(i32::MAX as i64) as i32;

    Ok(ModelChatHistorySaveRecords {
        conversation: ModelChatConversationSaveRecord {
            id: conversation_id,
            tenant_id,
            title: preview_chars(&user_message.content, MODEL_CHAT_TITLE_CHARS),
            route_id: response.route_id.clone(),
            model: response.model.clone(),
            message_count_increment: 2,
            last_message_preview: preview_chars(&response.answer, MODEL_CHAT_PREVIEW_CHARS),
            user_id,
            now,
        },
        messages: vec![
            ModelChatMessageSaveRecord {
                id: next_id(),
                tenant_id,
                conversation_id,
                role: "user".to_owned(),
                content: user_message.content.clone(),
                route_id: None,
                model: None,
                token_count: estimate_model_text_tokens(&user_message.content),
                metadata: json!({
                    "source": "ai.models.chat",
                    "messageCount": command.messages.len(),
                    "fileContexts": model_chat_file_context_metadata(&command.file_contexts),
                }),
                user_id,
                now,
            },
            ModelChatMessageSaveRecord {
                id: next_id(),
                tenant_id,
                conversation_id,
                role: "assistant".to_owned(),
                content: response.answer.clone(),
                route_id: Some(response.route_id.clone()),
                model: response.model.clone(),
                token_count: assistant_token_count,
                metadata: json!({
                    "source": "ai.models.chat",
                    "latencyMs": u128_to_i64(response.latency_ms),
                    "usage": response.usage,
                }),
                user_id,
                now,
            },
        ],
    })
}

fn model_chat_file_context_metadata(files: &[ModelChatFileContext]) -> Vec<Value> {
    files
        .iter()
        .map(|file| {
            json!({
                "name": file.name,
                "contentType": file.content_type,
                "charCount": file.content.chars().count(),
            })
        })
        .collect()
}

fn latest_user_message(command: &ModelChatCommand) -> Option<&ModelChatMessage> {
    command
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
}

async fn ensure_model_chat_conversation_owner(
    db: &PgPool,
    tenant_id: i64,
    user_id: i64,
    conversation_id: i64,
) -> Result<(), AppError> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"
SELECT EXISTS (
    SELECT 1
    FROM ai_model_chat_conversation
    WHERE tenant_id = $1
      AND id = $2
      AND create_user = $3
);
"#,
    )
    .bind(tenant_id)
    .bind(conversation_id)
    .bind(user_id)
    .fetch_one(db)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}

async fn persist_model_chat_history(
    db: &PgPool,
    records: &ModelChatHistorySaveRecords,
) -> Result<(), AppError> {
    let mut tx = db.begin().await?;
    let conversation = &records.conversation;
    sqlx::query(
        r#"
INSERT INTO ai_model_chat_conversation (
    id, tenant_id, title, route_id, model, message_count, last_message_preview,
    create_user, create_time, update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $8, $9)
ON CONFLICT (id) DO UPDATE
SET route_id = EXCLUDED.route_id,
    model = EXCLUDED.model,
    message_count = ai_model_chat_conversation.message_count + EXCLUDED.message_count,
    last_message_preview = EXCLUDED.last_message_preview,
    update_user = EXCLUDED.update_user,
    update_time = EXCLUDED.update_time;
"#,
    )
    .bind(conversation.id)
    .bind(conversation.tenant_id)
    .bind(&conversation.title)
    .bind(&conversation.route_id)
    .bind(&conversation.model)
    .bind(conversation.message_count_increment)
    .bind(&conversation.last_message_preview)
    .bind(conversation.user_id)
    .bind(conversation.now)
    .execute(&mut *tx)
    .await?;

    for message in &records.messages {
        sqlx::query(
            r#"
INSERT INTO ai_model_chat_message (
    id, tenant_id, conversation_id, role, content, route_id, model,
    token_count, metadata, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11);
"#,
        )
        .bind(message.id)
        .bind(message.tenant_id)
        .bind(message.conversation_id)
        .bind(&message.role)
        .bind(&message.content)
        .bind(&message.route_id)
        .bind(&message.model)
        .bind(message.token_count)
        .bind(&message.metadata)
        .bind(message.user_id)
        .bind(message.now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

fn model_chat_usage_record(
    tenant_id: i64,
    user_id: i64,
    response: &ModelChatResp,
    now: NaiveDateTime,
    source: &str,
) -> ModelUsageSaveRecord {
    let counts = response.usage.accounting_counts();

    ModelUsageSaveRecord {
        id: next_id(),
        tenant_id,
        route_id: response.route_id.clone(),
        usage_kind: "chat".to_owned(),
        prompt_tokens: counts.prompt_tokens,
        completion_tokens: counts.completion_tokens,
        total_tokens: counts.total_tokens,
        latency_ms: Some(u128_to_i64(response.latency_ms)),
        metadata: json!({
            "routeId": response.route_id,
            "model": response.model,
            "conversationId": response.conversation_id,
            "target": "llm",
            "source": source
        }),
        user_id,
        now,
    }
}

fn model_chat_cost_cents_from_spec(cost_spec: &Value, response: &ModelChatResp) -> Option<f64> {
    if cost_spec.is_null() || cost_spec.as_object().is_some_and(serde_json::Map::is_empty) {
        return None;
    }
    let counts = response.usage.accounting_counts();
    Some(estimate_model_cost_cents(
        cost_spec,
        &ModelUsageCostInput {
            prompt_tokens: counts.prompt_tokens,
            completion_tokens: counts.completion_tokens,
            total_tokens: counts.total_tokens,
            request_count: 1,
            vector_count: 0,
        },
    ))
}

async fn estimate_model_chat_response_cost_cents(
    db: &PgPool,
    tenant_id: i64,
    response: &ModelChatResp,
) -> Result<Option<f64>, AppError> {
    let cost_spec = sqlx::query_scalar::<_, Value>(
        r#"
SELECT p.cost_spec
FROM ai_model_route r
JOIN ai_model_profile p
  ON p.tenant_id = r.tenant_id
 AND p.id = r.model_profile_id
WHERE r.tenant_id = $1
  AND r.code = $2
  AND r.status = 1
ORDER BY r.priority ASC, r.id ASC
LIMIT 1;
"#,
    )
    .bind(tenant_id)
    .bind(&response.route_id)
    .fetch_optional(db)
    .await?;

    Ok(cost_spec.and_then(|cost_spec| model_chat_cost_cents_from_spec(&cost_spec, response)))
}

async fn record_model_chat_usage(
    db: &PgPool,
    record: &ModelUsageSaveRecord,
) -> Result<(), AppError> {
    let route = sqlx::query_as::<_, ModelUsageRouteAccountingRow>(
        r#"
SELECT
    r.id AS route_id,
    r.model_profile_id,
    p.cost_spec
FROM ai_model_route r
JOIN ai_model_profile p
  ON p.tenant_id = r.tenant_id
 AND p.id = r.model_profile_id
WHERE r.tenant_id = $1
  AND r.code = $2
  AND r.status = 1
ORDER BY r.priority ASC, r.id ASC
LIMIT 1;
"#,
    )
    .bind(record.tenant_id)
    .bind(&record.route_id)
    .fetch_optional(db)
    .await?;
    let (route_id, model_profile_id, cost_spec) = route
        .map(|route| {
            (
                Some(route.route_id),
                Some(route.model_profile_id),
                route.cost_spec,
            )
        })
        .unwrap_or((None, None, Value::Null));
    let cost_cents = estimate_model_cost_cents(
        &cost_spec,
        &ModelUsageCostInput {
            prompt_tokens: record.prompt_tokens,
            completion_tokens: record.completion_tokens,
            total_tokens: record.total_tokens,
            request_count: 1,
            vector_count: 0,
        },
    );

    sqlx::query(
        r#"
INSERT INTO ai_model_usage (
    id, tenant_id, route_id, model_profile_id, run_id, usage_kind,
    prompt_tokens, completion_tokens, total_tokens, request_count, vector_count,
    cost_cents, latency_ms, metadata, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, NULL, $5, $6, $7, $8, 1, 0, $9::numeric, $10, $11, $12, $13
);
"#,
    )
    .bind(record.id)
    .bind(record.tenant_id)
    .bind(route_id)
    .bind(model_profile_id)
    .bind(&record.usage_kind)
    .bind(record.prompt_tokens)
    .bind(record.completion_tokens)
    .bind(record.total_tokens)
    .bind(cost_cents)
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

fn preview_chars(text: &str, max_chars: usize) -> String {
    text.trim().chars().take(max_chars).collect()
}

fn route_target_for_purpose(purpose: ModelRoutePurpose) -> ModelRuntimeTarget {
    match purpose {
        ModelRoutePurpose::Embedding => ModelRuntimeTarget::Embedding,
        ModelRoutePurpose::Rerank => ModelRuntimeTarget::Reranker,
        ModelRoutePurpose::MediaGeneration => ModelRuntimeTarget::Draw,
        ModelRoutePurpose::Chat
        | ModelRoutePurpose::RagAnswer
        | ModelRoutePurpose::QueryRewrite
        | ModelRoutePurpose::EvalJudge
        | ModelRoutePurpose::CodeAgent
        | ModelRoutePurpose::GuardianReview => ModelRuntimeTarget::Llm,
    }
}

fn default_purpose_for_target(target: ModelRuntimeTarget) -> ModelRoutePurpose {
    match target {
        ModelRuntimeTarget::Llm => ModelRoutePurpose::Chat,
        ModelRuntimeTarget::Embedding => ModelRoutePurpose::Embedding,
        ModelRuntimeTarget::Reranker => ModelRoutePurpose::Rerank,
        ModelRuntimeTarget::Draw => ModelRoutePurpose::MediaGeneration,
    }
}

fn env_fallback_route_for_purpose(
    purpose: ModelRoutePurpose,
    config: &ModelRuntimeConfig,
) -> Option<ModelRuntimeRoute> {
    let target = route_target_for_purpose(purpose);
    let route = config.route(target)?;
    route.purposes().contains(&purpose).then(|| route.clone())
}

fn model_retry_policy_from_route_policy_status(
    status: &ModelRoutePolicyStatus,
) -> ModelRetryPolicy {
    ModelRetryPolicy {
        max_retries: (status.max_retries as usize).min(MAX_MODEL_RUNTIME_RETRIES),
    }
}

fn model_fallback_policy_decision_from_status(
    status: &ModelRoutePolicyStatus,
    fallback_route_id: Option<&str>,
) -> ModelFallbackPolicyDecision {
    let fallback_route_id = fallback_route_id
        .map(str::trim)
        .filter(|route_id| !route_id.is_empty())
        .map(str::to_owned);
    if !status.fallback_enabled {
        return ModelFallbackPolicyDecision {
            enabled: false,
            fallback_route_id,
            block_reason: Some("fallback_disabled".to_owned()),
        };
    }
    if !status.violations.is_empty() {
        return ModelFallbackPolicyDecision {
            enabled: false,
            fallback_route_id,
            block_reason: Some("policy_violation".to_owned()),
        };
    }
    if fallback_route_id.is_none() {
        return ModelFallbackPolicyDecision {
            enabled: false,
            fallback_route_id,
            block_reason: Some("missing_fallback_route".to_owned()),
        };
    }

    ModelFallbackPolicyDecision {
        enabled: true,
        fallback_route_id,
        block_reason: None,
    }
}

fn model_circuit_breaker_cooldown_seconds(plan: Option<&ModelRouteFallbackPlan>) -> Option<u32> {
    let plan = plan?;
    (plan.decision.enabled && plan.policy_status.circuit_breaker_seconds > 0)
        .then_some(plan.policy_status.circuit_breaker_seconds)
}

fn model_fallback_chain_can_visit(
    visited_route_ids: &HashSet<String>,
    next_route_id: &str,
    fallback_hops: usize,
) -> bool {
    fallback_hops < MAX_MODEL_FALLBACK_HOPS && !visited_route_ids.contains(next_route_id)
}

fn model_enabled_fallback_route_id(plan: Option<&ModelRouteFallbackPlan>) -> Option<&str> {
    plan.and_then(|plan| plan.decision.enabled.then_some(&plan.decision))
        .and_then(|decision| decision.fallback_route_id.as_deref())
}

fn model_circuit_breaker_registry() -> &'static Mutex<HashMap<String, Instant>> {
    MODEL_ROUTE_CIRCUIT_BREAKERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn model_circuit_breaker_open(route_id: &str, cooldown_seconds: u32) {
    if cooldown_seconds == 0 {
        return;
    }
    let opened_until = Instant::now() + Duration::from_secs(cooldown_seconds as u64);
    let mut registry = model_circuit_breaker_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    registry.insert(route_id.to_owned(), opened_until);
}

#[allow(dead_code)]
fn model_circuit_breaker_clear(route_id: &str) {
    let mut registry = model_circuit_breaker_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    registry.remove(route_id);
}

fn model_circuit_breaker_open_attempt(route: &ModelRuntimeRoute) -> Option<ModelProviderAttempt> {
    let now = Instant::now();
    let mut registry = model_circuit_breaker_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(opened_until) = registry.get(route.route_id()).copied() else {
        return None;
    };
    if opened_until <= now {
        registry.remove(route.route_id());
        return None;
    }

    Some(model_provider_attempt_circuit_open(
        route,
        opened_until.duration_since(now),
    ))
}

fn runtime_route_from_registry_row<F>(
    row: &ModelRuntimeRouteRow,
    mut get_env: F,
) -> Option<ModelRuntimeRoute>
where
    F: FnMut(&str) -> Option<String>,
{
    let purpose = ModelRoutePurpose::parse(&row.route_purpose)?;
    let target = route_target_for_purpose(purpose);
    let kind = ModelKind::parse(&row.model_kind)?;
    let provider = ModelProviderType::parse(&row.provider_type)?;
    let (api_key, env_keys) = resolve_credential_ref(row.credential_ref.as_deref(), &mut get_env)?;
    let base_url = row.deployment_endpoint.trim();
    if base_url.is_empty() {
        return None;
    }
    let endpoint = join_model_endpoint(base_url, row.api_path.as_deref());

    ModelRuntimeRoute::new(
        row.route_code.clone(),
        target,
        kind,
        provider,
        Some(row.model_name.clone()),
        base_url.to_owned(),
        endpoint,
        api_key,
        vec![purpose],
        env_keys,
    )
    .ok()
}

fn resolve_credential_ref<F>(
    credential_ref: Option<&str>,
    get_env: &mut F,
) -> Option<(String, Vec<String>)>
where
    F: FnMut(&str) -> Option<String>,
{
    let credential_ref = credential_ref?.trim();
    let env_key = credential_ref.strip_prefix("env:")?.trim();
    if env_key.is_empty() {
        return None;
    }
    let api_key = get_env(env_key)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())?;
    Some((api_key, vec![env_key.to_owned()]))
}

fn join_model_endpoint(base_url: &str, api_path: Option<&str>) -> String {
    let base_url = base_url.trim().trim_end_matches('/');
    let Some(path) = api_path
        .map(str::trim)
        .map(|path| path.trim_matches('/'))
        .filter(|path| !path.is_empty())
    else {
        return base_url.to_owned();
    };
    format!("{base_url}/{path}")
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

    check_target_with_route(client, route, target).await
}

async fn check_target_with_route(
    client: &reqwest::Client,
    route: &ModelRuntimeRoute,
    target: ModelRuntimeTarget,
) -> ModelHealthCheckResult {
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
    use novex_model::{
        ModelKind, ModelProviderType, ModelRoutePurpose, ModelRuntimeConfig, ModelRuntimeTarget,
    };

    use super::*;

    #[tokio::test]
    async fn model_runtime_service_can_be_bound_to_request_tenant() {
        let db = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();

        let service = ModelRuntimeService::for_tenant(db, 42);

        assert_eq!(service.tenant_id, 42);
    }

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
    fn effective_runtime_summary_merges_same_physical_llm_and_preserves_purpose_routes() {
        let chat_route = ModelRuntimeRoute::new(
            "runtime.llm.chat",
            ModelRuntimeTarget::Llm,
            ModelKind::Llm,
            ModelProviderType::DeepSeek,
            Some("deepseek-v4-flash".to_owned()),
            "https://api.deepseek.com",
            "https://api.deepseek.com/chat/completions",
            "sk-fake-llm-secret-508d",
            vec![ModelRoutePurpose::Chat],
            vec!["LLM_API_KEY".to_owned()],
        )
        .unwrap();
        let rag_answer_route = ModelRuntimeRoute::new(
            "runtime.llm.rag_answer",
            ModelRuntimeTarget::Llm,
            ModelKind::Llm,
            ModelProviderType::DeepSeek,
            Some("deepseek-v4-flash".to_owned()),
            "https://api.deepseek.com",
            "https://api.deepseek.com/chat/completions",
            "sk-fake-llm-secret-508d",
            vec![ModelRoutePurpose::RagAnswer],
            vec!["LLM_API_KEY".to_owned()],
        )
        .unwrap();
        let embedding_route = ModelRuntimeRoute::new(
            "runtime.embedding",
            ModelRuntimeTarget::Embedding,
            ModelKind::Embedding,
            ModelProviderType::DashScope,
            Some("text-embedding-v4".to_owned()),
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "https://dashscope.aliyuncs.com/compatible-mode/v1/embeddings",
            "sk-fake-embedding-secret-ffff",
            vec![ModelRoutePurpose::Embedding],
            vec!["EMBEDDING_API_KEY".to_owned()],
        )
        .unwrap();

        let summary = effective_runtime_summary_from_routes(
            vec![chat_route, rag_answer_route, embedding_route],
            Vec::new(),
        );

        let llm_routes = summary
            .routes
            .iter()
            .filter(|route| route.target == ModelRuntimeTarget::Llm)
            .collect::<Vec<_>>();
        assert_eq!(llm_routes.len(), 1);
        let llm = llm_routes[0];
        assert_eq!(llm.route_id, "runtime.llm");
        assert_eq!(
            llm.purposes,
            vec![ModelRoutePurpose::Chat, ModelRoutePurpose::RagAnswer]
        );
        assert_eq!(
            llm.purpose_route_ids.get("chat").map(String::as_str),
            Some("runtime.llm.chat")
        );
        assert_eq!(
            llm.purpose_route_ids.get("rag_answer").map(String::as_str),
            Some("runtime.llm.rag_answer")
        );
        assert_eq!(summary.routes.len(), 2);
        assert!(!format!("{summary:?}").contains("sk-fake-llm-secret-508d"));
    }

    #[test]
    fn dynamic_route_from_registry_row_uses_route_code_and_env_secret() {
        let row = dynamic_route_test_row(
            "tenant42.rag_answer",
            "rag_answer",
            "llm",
            Some("/chat/completions"),
            Some("env:LLM_PRIVATE_KEY"),
        );

        let route = runtime_route_from_registry_row(&row, |key| {
            (key == "LLM_PRIVATE_KEY").then(|| "sk-fake-private-secret-0001".to_owned())
        })
        .unwrap();
        let summary = route.summary();

        assert_eq!(summary.route_id, "tenant42.rag_answer");
        assert_eq!(summary.target, ModelRuntimeTarget::Llm);
        assert_eq!(summary.kind, ModelKind::Llm);
        assert_eq!(summary.provider, ModelProviderType::OpenAiCompatible);
        assert_eq!(summary.endpoint, "https://llm.internal/v1/chat/completions");
        assert_eq!(summary.model.as_deref(), Some("qwen-private"));
        assert_eq!(summary.masked_api_key, "sk-****0001");
        assert_eq!(summary.purposes, vec![ModelRoutePurpose::RagAnswer]);
        assert!(!format!("{route:?}").contains("sk-fake-private-secret-0001"));
    }

    #[test]
    fn dynamic_route_from_registry_row_skips_missing_env_secret() {
        let row = dynamic_route_test_row(
            "tenant42.chat",
            "chat",
            "llm",
            Some("/chat/completions"),
            Some("env:LLM_PRIVATE_KEY"),
        );

        let route = runtime_route_from_registry_row(&row, |_| None);

        assert!(route.is_none());
    }

    #[test]
    fn dynamic_route_purpose_maps_to_runtime_target() {
        assert_eq!(
            route_target_for_purpose(ModelRoutePurpose::Chat),
            ModelRuntimeTarget::Llm
        );
        assert_eq!(
            route_target_for_purpose(ModelRoutePurpose::GuardianReview),
            ModelRuntimeTarget::Llm
        );
        assert_eq!(
            route_target_for_purpose(ModelRoutePurpose::Embedding),
            ModelRuntimeTarget::Embedding
        );
        assert_eq!(
            route_target_for_purpose(ModelRoutePurpose::Rerank),
            ModelRuntimeTarget::Reranker
        );
        assert_eq!(
            route_target_for_purpose(ModelRoutePurpose::MediaGeneration),
            ModelRuntimeTarget::Draw
        );
    }

    #[test]
    fn guardian_review_model_route_maps_to_llm_runtime_target() {
        assert_eq!(
            route_target_for_purpose(ModelRoutePurpose::GuardianReview),
            ModelRuntimeTarget::Llm
        );
    }

    #[test]
    fn dynamic_route_falls_back_to_env_config_for_purpose() {
        let config = llm_test_config();

        let route = env_fallback_route_for_purpose(ModelRoutePurpose::Chat, &config).unwrap();

        assert_eq!(route.summary().route_id, "runtime.llm");
        assert_eq!(route.summary().target, ModelRuntimeTarget::Llm);
    }

    #[test]
    fn model_chat_tenant_bound_path_resolves_chat_route_purpose() {
        let source = include_str!("model_service.rs");

        assert!(source.contains("self.resolve_route_for_purpose(ModelRoutePurpose::Chat).await?"));
        assert!(source.contains("execute_normalized_chat_completion_with_route"));
    }

    #[test]
    fn model_chat_usage_accounting_matches_selected_route_code() {
        let source = include_str!("model_service.rs");

        assert!(source.contains("AND r.code = $2"));
        assert!(source.contains(".bind(&record.route_id)"));
    }

    #[test]
    fn model_chat_response_cost_runtime_attaches_route_cost() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("estimate_model_chat_response_cost_cents(&self.db"));
        assert!(source.contains("response.cost_cents ="));
        assert!(source.contains("chat_completion_for_purpose"));
    }

    #[test]
    fn model_route_retry_policy_caps_policy_max_retries() {
        let status = ModelRoutePolicyStatus {
            network_zone: "public".to_owned(),
            fallback_network_zone: None,
            fallback_enabled: false,
            cross_zone_fallback_allowed: false,
            max_retries: 10,
            circuit_breaker_seconds: 0,
            violations: vec![],
        };

        let policy = model_retry_policy_from_route_policy_status(&status);

        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.max_attempts(), 4);
    }

    #[test]
    fn model_runtime_retry_policy_reads_route_policy_source_contract() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("pub async fn retry_policy_for_purpose"));
        assert!(source.contains("profile.fallback_policy"));
        assert!(source.contains("evaluate_model_route_policy"));
    }

    #[test]
    fn model_route_fallback_policy_enables_valid_fallback_route() {
        let status = evaluate_model_route_policy(ModelRoutePolicyInput {
            network_zone: "private",
            fallback_network_zone: Some("private"),
            fallback_policy: &json!({ "enabled": true }),
            route_policy: &Value::Null,
        });

        let decision =
            model_fallback_policy_decision_from_status(&status, Some("runtime.llm.backup"));

        assert!(decision.enabled);
        assert_eq!(
            decision.fallback_route_id.as_deref(),
            Some("runtime.llm.backup")
        );
    }

    #[test]
    fn model_route_fallback_policy_blocks_policy_violations() {
        let status = evaluate_model_route_policy(ModelRoutePolicyInput {
            network_zone: "private",
            fallback_network_zone: Some("public"),
            fallback_policy: &json!({ "enabled": true }),
            route_policy: &Value::Null,
        });

        let decision =
            model_fallback_policy_decision_from_status(&status, Some("runtime.llm.backup"));

        assert!(!decision.enabled);
        assert_eq!(decision.block_reason.as_deref(), Some("policy_violation"));
    }

    #[test]
    fn model_route_fallback_source_contract_reads_configured_fallback_route() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("pub async fn fallback_plan_for_purpose"));
        assert!(source.contains("fallback_route.code AS fallback_route_code"));
        assert!(source.contains("evaluate_model_route_policy(ModelRoutePolicyInput"));
    }

    #[test]
    fn route_circuit_breaker_attempt_marks_open_route_as_skipped() {
        let route = llm_test_config()
            .route(ModelRuntimeTarget::Llm)
            .unwrap()
            .clone();
        model_circuit_breaker_clear(route.route_id());
        model_circuit_breaker_open(route.route_id(), 30);

        let attempt = model_circuit_breaker_open_attempt(&route).unwrap();

        assert_eq!(attempt.attempt_kind, "primary");
        assert_eq!(attempt.status, "skipped");
        assert_eq!(attempt.error_kind.as_deref(), Some("circuit_open"));
        assert_eq!(attempt.latency_ms, 0);
        model_circuit_breaker_clear(route.route_id());
    }

    #[test]
    fn route_circuit_breaker_cooldown_requires_enabled_fallback_and_positive_policy() {
        let disabled = ModelRouteFallbackPlan {
            primary_route_id: "runtime.llm".to_owned(),
            decision: ModelFallbackPolicyDecision {
                enabled: false,
                fallback_route_id: Some("runtime.llm.backup".to_owned()),
                block_reason: Some("fallback_disabled".to_owned()),
            },
            policy_status: ModelRoutePolicyStatus {
                network_zone: "public".to_owned(),
                fallback_network_zone: Some("public".to_owned()),
                fallback_enabled: false,
                cross_zone_fallback_allowed: false,
                max_retries: 0,
                circuit_breaker_seconds: 30,
                violations: vec![],
            },
        };

        assert_eq!(
            model_circuit_breaker_cooldown_seconds(Some(&disabled)),
            None
        );
    }

    #[test]
    fn route_circuit_breaker_source_contract_bypasses_primary_and_opens_after_failure() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("model_circuit_breaker_open_attempt(&current_route)"));
        assert!(source.contains("model_circuit_breaker_open(current_route.route_id()"));
        assert!(source.contains("model_circuit_breaker_cooldown_seconds(fallback_plan.as_ref())"));
    }

    #[test]
    fn persistent_route_circuit_breaker_source_contract_opens_runtime_state() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("async fn persistent_model_circuit_breaker_open"));
        assert!(source.contains("INSERT INTO ai_model_route_circuit_breaker"));
        assert!(source.contains("ON CONFLICT (tenant_id, route_id) DO UPDATE"));
    }

    #[test]
    fn persistent_route_circuit_breaker_source_contract_reads_runtime_state() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("async fn persistent_model_circuit_breaker_open_attempt"));
        assert!(source.contains("FROM ai_model_route_circuit_breaker"));
        assert!(source.contains("opened_until > NOW()"));
        assert!(source.contains("model_provider_attempt_circuit_open"));
    }

    #[test]
    fn persistent_route_circuit_breaker_source_contract_wires_runtime_chain() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains(".persistent_model_circuit_breaker_open_attempt(&current_route)"));
        assert!(source.contains(".persistent_model_circuit_breaker_open("));
        assert!(source
            .contains("model_circuit_breaker_open(current_route.route_id(), cooldown_seconds)"));
    }

    #[test]
    fn route_breaker_controls_source_contract_lists_tenant_breakers() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("pub async fn list_route_circuit_breakers"));
        assert!(source.contains("FROM ai_model_route_circuit_breaker"));
        assert!(source.contains("WHERE tenant_id = $1"));
        assert!(source.contains("ORDER BY opened_until DESC"));
    }

    #[test]
    fn route_breaker_controls_source_contract_clears_tenant_breaker_and_local_cache() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("pub async fn clear_route_circuit_breaker"));
        assert!(source.contains("DELETE FROM ai_model_route_circuit_breaker"));
        assert!(source.contains("WHERE tenant_id = $1"));
        assert!(source.contains("model_circuit_breaker_clear(route_id)"));
    }

    #[test]
    fn route_breaker_controls_source_contract_checks_persistent_before_local_cache() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        let persistent = source
            .find("persistent_model_circuit_breaker_open_attempt(&current_route)")
            .unwrap();
        let local = source
            .find("model_circuit_breaker_open_attempt(&current_route)")
            .unwrap();

        assert!(persistent < local);
    }

    #[test]
    fn model_ops_summary_source_contract_reads_route_health_usage_and_breakers() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("pub async fn model_ops_summary"));
        assert!(source.contains("FROM ai_model_route r"));
        assert!(source.contains("ai_model_route_circuit_breaker"));
        assert!(source.contains("ai_model_health_check"));
        assert!(source.contains("ai_model_usage"));
        assert!(source.contains("WHERE r.tenant_id = $1"));
        assert!(source.contains("INTERVAL '24 hours'"));
    }

    #[test]
    fn model_ops_summary_source_contract_reads_active_model_ops_alerts() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("ai_model_ops_alert alert"));
        assert!(source.contains("alert.resolved_at IS NULL"));
        assert!(source.contains("ORDER BY alert.last_seen_at DESC"));
        assert!(source.contains("model_ops_summary_from_rows("));
        assert!(source.contains("alert_rows,"));
    }

    #[test]
    fn model_ops_summary_from_rows_counts_open_breakers_and_degraded_routes() {
        let now =
            NaiveDateTime::parse_from_str("2026-06-17 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let summary = model_ops_summary_from_rows(
            vec![
                model_ops_route_row(
                    "runtime.llm.chat",
                    "chat",
                    1,
                    Some(now + chrono::Duration::minutes(5)),
                    Some("ok"),
                ),
                model_ops_route_row(
                    "runtime.embedding",
                    "embedding",
                    1,
                    None,
                    Some("provider returned HTTP 500"),
                ),
            ],
            vec![],
            now,
        );

        assert_eq!(summary.route_count, 2);
        assert_eq!(summary.active_route_count, 2);
        assert_eq!(summary.open_breaker_count, 1);
        assert_eq!(summary.degraded_route_count, 2);
        assert_eq!(summary.usage_24h.request_count, 5);
        assert_eq!(summary.usage_24h.total_tokens, 1700);
    }

    #[test]
    fn model_ops_summary_includes_active_alerts_and_route_counts() {
        let now =
            NaiveDateTime::parse_from_str("2026-06-17 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let summary = model_ops_summary_from_rows(
            vec![model_ops_route_row(
                "runtime.llm.chat",
                "chat",
                1,
                None,
                Some("ok"),
            )],
            vec![model_ops_alert_row(
                "model_health:llm:route:11",
                Some("runtime.llm.chat"),
                Some("chat"),
                "provider unavailable",
                now,
            )],
            now,
        );

        assert_eq!(summary.active_alert_count, 1);
        assert_eq!(summary.alerts.len(), 1);
        assert_eq!(summary.alerts[0].alert_key, "model_health:llm:route:11");
        assert_eq!(
            summary.alerts[0].route_id.as_deref(),
            Some("runtime.llm.chat")
        );
        assert_eq!(summary.alerts[0].message, "provider unavailable");
        assert_eq!(summary.routes[0].active_alert_count, 1);
    }

    #[test]
    fn model_ops_summary_marks_route_degraded_when_active_alert_exists() {
        let now =
            NaiveDateTime::parse_from_str("2026-06-17 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let summary = model_ops_summary_from_rows(
            vec![model_ops_route_row(
                "runtime.llm.chat",
                "chat",
                1,
                None,
                Some("ok"),
            )],
            vec![model_ops_alert_row(
                "model_health:llm:route:11",
                Some("runtime.llm.chat"),
                Some("chat"),
                "provider unavailable",
                now,
            )],
            now,
        );

        assert!(summary.routes[0].degraded);
        assert_eq!(summary.degraded_route_count, 1);
    }

    #[test]
    fn model_ops_alert_delivery_message_contains_operational_context() {
        let now =
            NaiveDateTime::parse_from_str("2026-06-17 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let alert = model_ops_alert_delivery_candidate("model_health:llm:route:11", now);

        let message = model_ops_alert_delivery_message(&alert);

        assert!(message.contains("Novex Model Alert"));
        assert!(message.contains("critical"));
        assert!(message.contains("runtime.llm.chat"));
        assert!(message.contains("provider unavailable"));
    }

    #[test]
    fn model_ops_alert_delivery_dry_run_result_preserves_feishu_payload() {
        let now =
            NaiveDateTime::parse_from_str("2026-06-17 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let alert = model_ops_alert_delivery_candidate("model_health:llm:route:11", now);

        let result = model_ops_alert_delivery_dry_run_result(&alert);

        assert_eq!(result.status, "dry_run");
        assert!(result.dry_run);
        assert_eq!(result.request_payload["toolCode"], "feishu.message.send");
        assert_eq!(result.response_payload["status"], "dry_run");
        assert!(result.error_message.is_none());
    }

    #[test]
    fn model_ops_alert_delivery_source_contract_scans_audits_and_records_delivery() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("pub async fn deliver_active_model_ops_alerts"));
        assert!(source.contains("FROM ai_model_ops_alert alert"));
        assert!(source.contains("NOT EXISTS"));
        assert!(source.contains("ai_model_ops_alert_delivery delivery"));
        assert!(source.contains("create_tool_call_audit"));
        assert!(source.contains("INSERT INTO ai_model_ops_alert_delivery"));
    }

    fn model_ops_route_row(
        route_code: &str,
        route_purpose: &str,
        status: i16,
        breaker_opened_until: Option<NaiveDateTime>,
        last_health_status: Option<&str>,
    ) -> ModelRouteOpsSummaryRow {
        ModelRouteOpsSummaryRow {
            route_code: route_code.to_owned(),
            route_purpose: route_purpose.to_owned(),
            provider_code: "deepseek".to_owned(),
            provider_type: "deep-seek".to_owned(),
            model_name: "deepseek-v4".to_owned(),
            network_zone: "public".to_owned(),
            status,
            breaker_opened_until,
            last_health_status: last_health_status.map(str::to_owned),
            last_health_checked_at: Some(
                NaiveDateTime::parse_from_str("2026-06-17 09:59:00", "%Y-%m-%d %H:%M:%S").unwrap(),
            ),
            last_health_latency_ms: Some(120),
            request_count_24h: if route_purpose == "chat" { 3 } else { 2 },
            total_tokens_24h: if route_purpose == "chat" { 1200 } else { 500 },
            cost_cents_24h: if route_purpose == "chat" { 1.5 } else { 0.25 },
            avg_latency_ms_24h: Some(if route_purpose == "chat" { 330.0 } else { 90.0 }),
        }
    }

    fn model_ops_alert_row(
        alert_key: &str,
        route_code: Option<&str>,
        route_purpose: Option<&str>,
        message: &str,
        now: NaiveDateTime,
    ) -> ModelOpsAlertRow {
        ModelOpsAlertRow {
            alert_key: alert_key.to_owned(),
            alert_kind: "model_health".to_owned(),
            severity: "critical".to_owned(),
            status: "active".to_owned(),
            route_code: route_code.map(str::to_owned),
            route_purpose: route_purpose.map(str::to_owned),
            provider_code: Some("deepseek".to_owned()),
            model_name: Some("deepseek-v4".to_owned()),
            source_ref: "health_check:99".to_owned(),
            event_payload: json!({"message": message}),
            first_seen_at: now,
            last_seen_at: now,
        }
    }

    fn model_ops_alert_delivery_candidate(
        alert_key: &str,
        now: NaiveDateTime,
    ) -> ModelOpsAlertDeliveryCandidateRow {
        ModelOpsAlertDeliveryCandidateRow {
            alert_id: 42,
            tenant_id: 1,
            alert_key: alert_key.to_owned(),
            alert_kind: "model_health".to_owned(),
            severity: "critical".to_owned(),
            route_code: Some("runtime.llm.chat".to_owned()),
            route_purpose: Some("chat".to_owned()),
            provider_code: Some("deepseek".to_owned()),
            model_name: Some("deepseek-v4".to_owned()),
            source_ref: "health_check:99".to_owned(),
            event_payload: json!({"message":"provider unavailable"}),
            first_seen_at: now,
            last_seen_at: now,
        }
    }

    #[test]
    fn model_health_persistence_source_contract_records_tenant_health_rows() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("persist_model_health_check_results"));
        assert!(source.contains("INSERT INTO ai_model_health_check"));
        assert!(source.contains("WHERE r.tenant_id = $1"));
        assert!(source.contains("default_purpose_for_target(result.target)"));
        assert!(source.contains("health_check_for_tenant"));
    }

    #[test]
    fn model_health_persistence_record_from_result_maps_status_and_metadata() {
        let now =
            NaiveDateTime::parse_from_str("2026-06-17 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let result = ModelHealthCheckResult {
            target: ModelRuntimeTarget::Llm,
            configured: true,
            ok: false,
            endpoint: Some("https://llm.example.com/v1/chat/completions".to_owned()),
            masked_api_key: Some("sk-****0001".to_owned()),
            http_status: Some(502),
            latency_ms: 123,
            message: "provider returned HTTP 502".to_owned(),
            detail: Some(json!({"choiceCount": 0})),
        };

        let record = model_health_check_record_from_result(1, 7, Some((11, 22, 33)), &result, now);

        assert_eq!(record.status, "provider returned HTTP 502");
        assert_eq!(record.http_status, Some(502));
        assert_eq!(record.latency_ms, Some(123));
        assert_eq!(record.detail["target"], "llm");
        assert_eq!(record.route_id, Some(11));
        assert_eq!(record.provider_id, Some(22));
        assert_eq!(record.model_profile_id, Some(33));
    }

    #[test]
    fn model_health_alert_record_from_failure_uses_stable_key_and_payload() {
        let now =
            NaiveDateTime::parse_from_str("2026-06-17 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let result = ModelHealthCheckResult {
            target: ModelRuntimeTarget::Llm,
            configured: true,
            ok: false,
            endpoint: Some("https://api.example.test".to_owned()),
            masked_api_key: Some("sk-***1234".to_owned()),
            http_status: Some(503),
            latency_ms: 123,
            message: "provider unavailable".to_owned(),
            detail: Some(json!({"provider":"example"})),
        };

        let record =
            model_ops_alert_record_from_health_check(1, 7, Some((11, 22, 33)), &result, 99, now);

        assert_eq!(record.tenant_id, 1);
        assert_eq!(record.alert_key, "model_health:llm:route:11");
        assert_eq!(record.alert_kind, "model_health");
        assert_eq!(record.severity, "critical");
        assert_eq!(record.status, "active");
        assert_eq!(record.route_id, Some(11));
        assert_eq!(record.provider_id, Some(22));
        assert_eq!(record.model_profile_id, Some(33));
        assert_eq!(record.source_ref, "health_check:99");
        assert_eq!(record.event_payload["message"], "provider unavailable");
        assert_eq!(record.event_payload["maskedApiKey"], "sk-***1234");
    }

    #[test]
    fn model_health_alert_key_uses_target_when_route_is_missing() {
        let result = ModelHealthCheckResult {
            target: ModelRuntimeTarget::Embedding,
            configured: false,
            ok: false,
            endpoint: None,
            masked_api_key: None,
            http_status: None,
            latency_ms: 0,
            message: "missing route".to_owned(),
            detail: None,
        };

        assert_eq!(
            model_ops_alert_key_from_health_check(1, None, &result),
            "model_health:embedding:tenant:1"
        );
    }

    #[test]
    fn model_health_alert_persistence_source_contract_upserts_and_resolves_active_alerts() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("record_model_ops_alert_for_health_check"));
        assert!(source.contains("upsert_model_ops_alert"));
        assert!(source.contains("resolve_model_ops_alert"));
        assert!(source.contains("ON CONFLICT (tenant_id, alert_key) WHERE resolved_at IS NULL"));
        assert!(source.contains("resolved_at = $4"));
        assert!(source.contains("persist_model_health_check_record(&self.db, &record).await?"));
    }

    #[test]
    fn refresh_active_tenant_model_health_source_contract_reads_active_tenants() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("pub async fn refresh_active_tenant_model_health"));
        assert!(source.contains("SELECT DISTINCT tenant_id"));
        assert!(source.contains("FROM ai_model_route"));
        assert!(source.contains("WHERE status = 1"));
        assert!(source.contains("health_check_for_tenant(ModelHealthCheckCommand"));
    }

    #[test]
    fn model_health_automation_migration_defines_alert_table_and_seed_job() {
        let migration =
            include_str!("../../../migrations/202606170004_create_ai_model_ops_alert.sql");

        assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_model_ops_alert"));
        assert!(migration.contains("uk_ai_model_ops_alert_active_key"));
        assert!(migration.contains("WHERE resolved_at IS NULL"));
        assert!(migration.contains("INSERT INTO sys_job"));
        assert!(migration.contains("'ai.model.health_check'"));
        assert!(migration.contains("'*/5 * * * * *'"));
    }

    #[test]
    fn model_ops_alert_delivery_migration_defines_table_and_seed_job() {
        let migration =
            include_str!("../../../migrations/202606170005_create_ai_model_ops_alert_delivery.sql");

        assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_model_ops_alert_delivery"));
        assert!(migration.contains("idx_ai_model_ops_alert_delivery_alert_id"));
        assert!(migration.contains("idx_ai_model_ops_alert_delivery_channel_status"));
        assert!(migration.contains("INSERT INTO sys_job"));
        assert!(migration.contains("'ai.model.alert_delivery'"));
        assert!(migration.contains("'AI Model Alert Delivery'"));
    }

    #[test]
    fn multi_hop_fallback_allows_bounded_new_routes() {
        let mut visited = std::collections::HashSet::from(["runtime.llm".to_owned()]);

        assert!(model_fallback_chain_can_visit(
            &visited,
            "runtime.llm.backup",
            0,
        ));
        visited.insert("runtime.llm.backup".to_owned());
        assert!(model_fallback_chain_can_visit(
            &visited,
            "runtime.llm.global",
            MAX_MODEL_FALLBACK_HOPS - 1,
        ));
    }

    #[test]
    fn multi_hop_fallback_blocks_cycles_and_hop_overflow() {
        let visited = std::collections::HashSet::from(["runtime.llm".to_owned()]);

        assert!(!model_fallback_chain_can_visit(&visited, "runtime.llm", 0));
        assert!(!model_fallback_chain_can_visit(
            &visited,
            "runtime.llm.global",
            MAX_MODEL_FALLBACK_HOPS,
        ));
    }

    #[test]
    fn multi_hop_fallback_source_contract_iterates_route_chain() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("while fallback_hops <= MAX_MODEL_FALLBACK_HOPS"));
        assert!(source.contains("model_fallback_chain_can_visit(&visited_route_ids"));
        assert!(source.contains(
            "fallback_plan_for_purpose_with_route_id(purpose, Some(current_route.route_id()))"
        ));
        assert!(source.contains("attempt_kind = if fallback_hops == 0"));
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
                fallback_policy: Value::Null,
                status: 1,
            }],
            vec![ModelRouteRegistryRow {
                id: 30,
                code: "runtime.llm.chat".to_owned(),
                route_purpose: "chat".to_owned(),
                model_profile_id: 20,
                priority: 100,
                fallback_route_id: None,
                status: 1,
                policy: Value::Null,
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
    fn model_registry_summary_marks_cross_zone_fallback_policy_violation() {
        let summary = ModelRuntimeService::registry_summary_from_rows(
            vec![ModelProviderRegistryRow {
                id: 1,
                code: "private-llm".to_owned(),
                name: "Private LLM".to_owned(),
                provider_type: "openai-compatible".to_owned(),
                status: 1,
            }],
            vec![
                ModelDeploymentRegistryRow {
                    id: 10,
                    provider_id: 1,
                    code: "llm-private".to_owned(),
                    name: "Private LLM".to_owned(),
                    endpoint: "https://llm.internal".to_owned(),
                    network_zone: "private".to_owned(),
                    status: 1,
                },
                ModelDeploymentRegistryRow {
                    id: 11,
                    provider_id: 1,
                    code: "llm-public".to_owned(),
                    name: "Public LLM".to_owned(),
                    endpoint: "https://api.example.com".to_owned(),
                    network_zone: "public".to_owned(),
                    status: 1,
                },
            ],
            vec![
                ModelProfileRegistryRow {
                    id: 20,
                    deployment_id: 10,
                    code: "private-chat".to_owned(),
                    name: "Private Chat".to_owned(),
                    model_name: "private-chat".to_owned(),
                    model_kind: "llm".to_owned(),
                    fallback_policy: json!({
                        "enabled": true,
                        "maxRetries": 2,
                        "circuitBreakerSeconds": 45
                    }),
                    status: 1,
                },
                ModelProfileRegistryRow {
                    id: 21,
                    deployment_id: 11,
                    code: "public-chat".to_owned(),
                    name: "Public Chat".to_owned(),
                    model_name: "public-chat".to_owned(),
                    model_kind: "llm".to_owned(),
                    fallback_policy: Value::Null,
                    status: 1,
                },
            ],
            vec![
                ModelRouteRegistryRow {
                    id: 30,
                    code: "runtime.llm.private".to_owned(),
                    route_purpose: "chat".to_owned(),
                    model_profile_id: 20,
                    priority: 100,
                    fallback_route_id: Some(31),
                    status: 1,
                    policy: Value::Null,
                    credential_ref: None,
                    masked_value: None,
                },
                ModelRouteRegistryRow {
                    id: 31,
                    code: "runtime.llm.public".to_owned(),
                    route_purpose: "chat".to_owned(),
                    model_profile_id: 21,
                    priority: 200,
                    fallback_route_id: None,
                    status: 1,
                    policy: Value::Null,
                    credential_ref: None,
                    masked_value: None,
                },
            ],
        );

        assert_eq!(summary.profiles[0].fallback_policy["enabled"], true);
        assert_eq!(summary.routes[0].policy_status.network_zone, "private");
        assert!(summary.routes[0].policy_status.fallback_enabled);
        assert_eq!(
            summary.routes[0].policy_status.violations,
            vec!["cross_zone_fallback_not_allowed".to_owned()]
        );
        assert!(summary.routes[1].policy_status.violations.is_empty());
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
                fallback_route_id: None,
                status: 1,
                policy: Value::Null,
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
    fn rerank_response_parser_maps_dashscope_result_scores() {
        let body = serde_json::json!({
            "results": [
                {"index": 2, "relevance_score": 0.91},
                {"index": 0, "score": 0.72}
            ],
            "usage": {"total_tokens": 18}
        });

        let scores = ModelRuntimeService::parse_rerank_scores(&body);

        assert_eq!(scores.len(), 2);
        assert_eq!(scores[0].index, 2);
        assert!((scores[0].score - 0.91).abs() < f32::EPSILON);
        assert_eq!(scores[1].index, 0);
        assert!((scores[1].score - 0.72).abs() < f32::EPSILON);
    }

    #[test]
    fn embedding_response_parser_maps_openai_compatible_vectors() {
        let body = serde_json::json!({
            "data": [
                {"index": 1, "embedding": [0.1, -0.2, 0.3]},
                {"index": 0, "embedding": ["0.4", "0.5"]}
            ],
            "usage": {"total_tokens": 12}
        });

        let vectors = ModelRuntimeService::parse_embedding_vectors(&body);

        assert_eq!(vectors.len(), 2);
        assert_eq!(vectors[0].index, 1);
        assert_eq!(vectors[0].vector, vec![0.1, -0.2, 0.3]);
        assert_eq!(vectors[1].index, 0);
        assert_eq!(vectors[1].vector, vec![0.4, 0.5]);
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
            ..ModelChatCommand::default()
        })
        .unwrap();

        assert_eq!(command.messages[0].role, "system");
        assert_eq!(command.messages[0].content, "You are Novex.");
        assert_eq!(command.messages[1].role, "user");
        assert_eq!(command.temperature, Some(1.0));
        assert_eq!(command.max_tokens, Some(4096));
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
            ..ModelChatCommand::default()
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
    fn model_chat_payload_omits_provider_metadata_for_regular_chat() {
        let route = openai_compatible_llm_route();
        let command = normalize_model_chat_command(ModelChatCommand {
            messages: vec![ModelChatMessage {
                role: "user".to_owned(),
                content: "hello".to_owned(),
            }],
            ..ModelChatCommand::default()
        })
        .unwrap();

        let payload = model_chat_request_payload(&route, &command);

        assert!(payload.get("metadata").is_none());
    }

    #[test]
    fn model_chat_payload_serializes_compaction_transport_metadata_for_openai_compatible_route() {
        let route = openai_compatible_llm_route();
        let command = normalize_model_chat_command(ModelChatCommand {
            messages: vec![ModelChatMessage {
                role: "user".to_owned(),
                content: "compact this agent context".to_owned(),
            }],
            request_metadata: Some(test_compaction_request_metadata()),
            ..ModelChatCommand::default()
        })
        .unwrap();

        let payload = model_chat_request_payload(&route, &command);

        assert_eq!(payload["metadata"]["request_kind"], "compaction");
        assert_eq!(
            payload["metadata"]["compaction_implementation"],
            "responses_compaction_v2"
        );
        assert_eq!(
            payload["metadata"]["compaction_reason"],
            "observation_threshold"
        );
        assert_eq!(
            payload["metadata"]["compaction_phase"],
            "model_loop_follow_up"
        );
        assert_eq!(payload["metadata"]["compaction_window_id"], "1");
        assert_eq!(payload["metadata"]["input_history_count"], "2");
        assert_eq!(payload["metadata"]["retained_history_count"], "1");
        assert_eq!(payload["metadata"]["tool_codes"], "rag.search");
    }

    #[test]
    fn model_chat_payload_omits_provider_metadata_for_unsupported_provider() {
        let route = llm_test_config()
            .route(ModelRuntimeTarget::Llm)
            .unwrap()
            .clone();
        let command = normalize_model_chat_command(ModelChatCommand {
            messages: vec![ModelChatMessage {
                role: "user".to_owned(),
                content: "compact this agent context".to_owned(),
            }],
            request_metadata: Some(test_compaction_request_metadata()),
            ..ModelChatCommand::default()
        })
        .unwrap();

        let payload = model_chat_request_payload(&route, &command);

        assert_eq!(route.provider(), ModelProviderType::DeepSeek);
        assert!(payload.get("metadata").is_none());
    }

    #[test]
    fn model_chat_command_accepts_existing_conversation_id() {
        let command = normalize_model_chat_command(ModelChatCommand {
            conversation_id: Some(88),
            messages: vec![ModelChatMessage {
                role: "user".to_owned(),
                content: "继续刚才的话题".to_owned(),
            }],
            ..ModelChatCommand::default()
        })
        .unwrap();

        assert_eq!(command.conversation_id, Some(88));
    }

    #[test]
    fn model_chat_command_normalizes_file_contexts() {
        let command = normalize_model_chat_command(ModelChatCommand {
            messages: vec![ModelChatMessage {
                role: "user".to_owned(),
                content: "总结这个文件".to_owned(),
            }],
            file_contexts: vec![ModelChatFileContext {
                name: "  handbook.md  ".to_owned(),
                content_type: "  text/markdown  ".to_owned(),
                content: "  # 入职手册\n第一天完成安全培训。  ".to_owned(),
            }],
            ..ModelChatCommand::default()
        })
        .unwrap();

        assert_eq!(command.file_contexts.len(), 1);
        assert_eq!(command.file_contexts[0].name, "handbook.md");
        assert_eq!(command.file_contexts[0].content_type, "text/markdown");
        assert_eq!(
            command.file_contexts[0].content,
            "# 入职手册\n第一天完成安全培训。"
        );
    }

    #[test]
    fn model_chat_payload_injects_file_context_before_user_messages() {
        let route = llm_test_config()
            .route(ModelRuntimeTarget::Llm)
            .unwrap()
            .clone();
        let command = normalize_model_chat_command(ModelChatCommand {
            messages: vec![ModelChatMessage {
                role: "user".to_owned(),
                content: "总结这个文件".to_owned(),
            }],
            file_contexts: vec![ModelChatFileContext {
                name: "handbook.md".to_owned(),
                content_type: "text/markdown".to_owned(),
                content: "# 入职手册\n第一天完成安全培训。".to_owned(),
            }],
            ..ModelChatCommand::default()
        })
        .unwrap();

        let payload = model_chat_request_payload(&route, &command);

        assert_eq!(payload["messages"][0]["role"], "system");
        assert!(payload["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("[File: handbook.md | text/markdown]"));
        assert!(payload["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("第一天完成安全培训。"));
        assert_eq!(payload["messages"][1]["role"], "user");
        assert_eq!(payload["messages"][1]["content"], "总结这个文件");
    }

    #[test]
    fn model_chat_payload_includes_optional_response_format() {
        let route = llm_test_config()
            .route(ModelRuntimeTarget::Llm)
            .unwrap()
            .clone();
        let command = normalize_model_chat_command(ModelChatCommand {
            messages: vec![ModelChatMessage {
                role: "user".to_owned(),
                content: "返回 JSON".to_owned(),
            }],
            response_format: Some(json!({ "type": "json_object" })),
            ..ModelChatCommand::default()
        })
        .unwrap();

        let payload = model_chat_request_payload(&route, &command);

        assert_eq!(payload["response_format"]["type"], "json_object");
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

        let response = model_chat_response_from_provider(&route, body, 42, Some(77)).unwrap();

        assert_eq!(response.answer, "Novex can run pure model chat.");
        assert_eq!(response.conversation_id, Some(77));
        assert_eq!(response.route_id, "runtime.llm");
        assert_eq!(response.provider, "deep-seek");
        assert_eq!(response.model.as_deref(), Some("deepseek-v4-flash"));
        assert_eq!(response.latency_ms, 42);
        assert_eq!(response.usage.prompt_tokens, Some(11));
        assert_eq!(response.usage.completion_tokens, Some(7));
        assert_eq!(response.usage.total_tokens, Some(18));
        assert!(!format!("{response:?}").contains("sk-fake-llm-secret-508d"));
    }

    #[test]
    fn model_chat_response_normalizes_provider_usage_aliases_and_total() {
        let route = llm_test_config()
            .route(ModelRuntimeTarget::Llm)
            .unwrap()
            .clone();
        let body = json!({
            "choices": [
                { "message": { "content": "Novex normalized provider usage." } }
            ],
            "usage": {
                "input_tokens": "11",
                "outputTokens": 7
            }
        });

        let response = model_chat_response_from_provider(&route, body, 42, None).unwrap();

        assert_eq!(response.usage.prompt_tokens, Some(11));
        assert_eq!(response.usage.completion_tokens, Some(7));
        assert_eq!(response.usage.total_tokens, Some(18));
    }

    #[test]
    fn model_chat_response_accepts_provider_text_fallback() {
        let route = llm_test_config()
            .route(ModelRuntimeTarget::Llm)
            .unwrap()
            .clone();
        let body = json!({
            "choices": [
                {
                    "message": { "content": "" },
                    "text": "Novex accepted a provider text fallback."
                }
            ]
        });

        let response = model_chat_response_from_provider(&route, body, 42, None).unwrap();

        assert_eq!(response.answer, "Novex accepted a provider text fallback.");
    }

    #[test]
    fn provider_lifecycle_attempt_records_success_metadata() {
        let route = llm_test_config()
            .route(ModelRuntimeTarget::Llm)
            .unwrap()
            .clone();
        let attempt = model_provider_attempt_succeeded("fallback", &route, 42);

        assert_eq!(attempt.attempt_kind, "fallback");
        assert_eq!(attempt.route_id, "runtime.llm");
        assert_eq!(attempt.provider, "deep-seek");
        assert_eq!(attempt.status, "succeeded");
        assert_eq!(attempt.latency_ms, 42);
        assert!(attempt.message.is_none());
    }

    #[test]
    fn provider_lifecycle_attempt_records_retryable_http_failure() {
        let route = llm_test_config()
            .route(ModelRuntimeTarget::Llm)
            .unwrap()
            .clone();
        let err = AppError::bad_request("LLM 模型调用失败: HTTP 502");
        let attempt = model_provider_attempt_failed("primary", &route, &err, 12);

        assert_eq!(attempt.status, "failed");
        assert_eq!(attempt.error_kind.as_deref(), Some("provider_http"));
        assert_eq!(attempt.http_status, Some(502));
        assert!(model_provider_error_is_fallback_candidate(&err));
    }

    #[test]
    fn provider_lifecycle_source_contract_fallback_wraps_chat_completion() {
        let source = include_str!("model_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("fallback_plan_for_purpose_with_route_id(purpose"));
        assert!(source.contains("model_provider_error_is_fallback_candidate"));
        assert!(source.contains("attempt_kind = if fallback_hops == 0"));
        assert!(source.contains("\"fallback\""));
    }

    #[test]
    fn model_chat_history_records_capture_latest_turn_metadata() {
        let now = chrono::NaiveDateTime::parse_from_str("2026-06-05 10:00:00", "%Y-%m-%d %H:%M:%S")
            .unwrap();
        let command = normalize_model_chat_command(ModelChatCommand {
            conversation_id: None,
            messages: vec![
                ModelChatMessage {
                    role: "system".to_owned(),
                    content: "You are Novex.".to_owned(),
                },
                ModelChatMessage {
                    role: "user".to_owned(),
                    content: "  介绍一下模型对话历史能力  ".to_owned(),
                },
            ],
            file_contexts: vec![ModelChatFileContext {
                name: "roadmap.md".to_owned(),
                content_type: "text/markdown".to_owned(),
                content: "# Roadmap\nM1 会话历史".to_owned(),
            }],
            ..ModelChatCommand::default()
        })
        .unwrap();
        let response = ModelChatResp {
            conversation_id: Some(42),
            answer: "Novex records chat turns.".to_owned(),
            route_id: "runtime.llm".to_owned(),
            provider: "deep-seek".to_owned(),
            model: Some("deepseek-v4-flash".to_owned()),
            latency_ms: 42,
            usage: ModelChatUsage {
                prompt_tokens: Some(11),
                completion_tokens: Some(7),
                total_tokens: Some(18),
            },
            cost_cents: None,
            provider_attempts: vec![],
        };

        let records = model_chat_history_records(1, 9, &command, &response, now).unwrap();

        assert_eq!(records.conversation.id, 42);
        assert_eq!(records.conversation.title, "介绍一下模型对话历史能力");
        assert_eq!(records.conversation.message_count_increment, 2);
        assert_eq!(
            records.conversation.last_message_preview,
            "Novex records chat turns."
        );
        assert_eq!(records.messages.len(), 2);
        assert_eq!(records.messages[0].role, "user");
        assert_eq!(records.messages[0].content, "介绍一下模型对话历史能力");
        assert_eq!(
            records.messages[0].metadata["fileContexts"][0]["name"],
            "roadmap.md"
        );
        assert_eq!(
            records.messages[0].metadata["fileContexts"][0]["contentType"],
            "text/markdown"
        );
        assert_eq!(
            records.messages[0].metadata["fileContexts"][0]["charCount"],
            17
        );
        assert!(!records.messages[0]
            .metadata
            .to_string()
            .contains("M1 会话历史"));
        assert_eq!(records.messages[1].role, "assistant");
        assert_eq!(records.messages[1].route_id.as_deref(), Some("runtime.llm"));
        assert_eq!(records.messages[1].token_count, 7);
        assert_eq!(records.messages[1].metadata["latencyMs"], 42);
    }

    #[test]
    fn model_chat_history_migration_defines_conversation_and_message_tables() {
        let migration =
            include_str!("../../../migrations/202606050021_create_ai_model_chat_history.sql");

        for field in [
            "ai_model_chat_conversation",
            "ai_model_chat_message",
            "conversation_id",
            "last_message_preview",
            "message_count",
            "route_id",
            "model",
        ] {
            assert!(migration.contains(field), "missing {field}");
        }
    }

    #[test]
    fn model_chat_usage_record_maps_tokens_latency_and_route_without_content() {
        let now = chrono::NaiveDateTime::parse_from_str("2026-06-05 10:00:00", "%Y-%m-%d %H:%M:%S")
            .unwrap();
        let response = ModelChatResp {
            conversation_id: Some(42),
            answer: "Do not persist this answer".to_owned(),
            route_id: "runtime.llm".to_owned(),
            provider: "deep-seek".to_owned(),
            model: Some("deepseek-v4-flash".to_owned()),
            latency_ms: 42,
            usage: ModelChatUsage {
                prompt_tokens: Some(11),
                completion_tokens: Some(7),
                total_tokens: Some(18),
            },
            cost_cents: None,
            provider_attempts: vec![],
        };

        let record = model_chat_usage_record(1, 99, &response, now, "ai.models.chat");

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
    fn model_chat_cost_cents_from_spec_uses_response_usage() {
        let response = test_model_chat_response();
        let cost_spec = json!({
            "unit": "tokens",
            "promptCentsPer1kTokens": 0.1,
            "completionCentsPer1kTokens": 0.2
        });

        let cost_cents = model_chat_cost_cents_from_spec(&cost_spec, &response).unwrap();

        assert!((cost_cents - 0.5).abs() < 0.000_001);
    }

    #[test]
    fn model_chat_cost_cents_from_spec_ignores_missing_spec() {
        let response = test_model_chat_response();

        assert_eq!(
            model_chat_cost_cents_from_spec(&Value::Null, &response),
            None
        );
        assert_eq!(model_chat_cost_cents_from_spec(&json!({}), &response), None);
    }

    #[test]
    fn model_chat_usage_record_binds_request_tenant_and_source() {
        let now = chrono::NaiveDateTime::parse_from_str("2026-06-05 10:00:00", "%Y-%m-%d %H:%M:%S")
            .unwrap();
        let response = ModelChatResp {
            conversation_id: None,
            answer: "Tenant scoped answer".to_owned(),
            route_id: "runtime.llm".to_owned(),
            provider: "deep-seek".to_owned(),
            model: Some("deepseek-v4-flash".to_owned()),
            latency_ms: 24,
            usage: ModelChatUsage {
                prompt_tokens: Some(3),
                completion_tokens: Some(5),
                total_tokens: Some(8),
            },
            cost_cents: None,
            provider_attempts: vec![],
        };

        let record = model_chat_usage_record(42, 99, &response, now, "ai.chatFlow.model");

        assert_eq!(record.tenant_id, 42);
        assert_eq!(record.user_id, 99);
        assert_eq!(record.metadata["source"], "ai.chatFlow.model");
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

    fn openai_compatible_llm_route() -> ModelRuntimeRoute {
        ModelRuntimeRoute::new(
            "tenant42.code_agent",
            ModelRuntimeTarget::Llm,
            ModelKind::Llm,
            ModelProviderType::OpenAiCompatible,
            Some("gpt-compatible".to_owned()),
            "https://llm.internal/v1",
            "https://llm.internal/v1/chat/completions",
            "sk-fake-private-secret-0001",
            vec![ModelRoutePurpose::CodeAgent],
            vec!["LLM_PRIVATE_KEY".to_owned()],
        )
        .unwrap()
    }

    fn test_compaction_request_metadata() -> ModelChatRequestMetadata {
        ModelChatRequestMetadata::remote_compaction(ModelChatCompactionMetadata {
            implementation: "responses_compaction_v2".to_owned(),
            trigger: "auto".to_owned(),
            reason: "observation_threshold".to_owned(),
            phase: "model_loop_follow_up".to_owned(),
            strategy: "memento".to_owned(),
            window_id: 1,
            input_history_count: 2,
            retained_history_count: 1,
            compacted_item_count: 2,
            retained_item_count: 1,
            tool_codes: vec!["rag.search".to_owned()],
        })
    }

    fn dynamic_route_test_row(
        route_code: &str,
        route_purpose: &str,
        model_kind: &str,
        api_path: Option<&str>,
        credential_ref: Option<&str>,
    ) -> ModelRuntimeRouteRow {
        ModelRuntimeRouteRow {
            route_id: 30,
            route_code: route_code.to_owned(),
            route_purpose: route_purpose.to_owned(),
            provider_type: "openai-compatible".to_owned(),
            model_profile_id: 20,
            model_name: "qwen-private".to_owned(),
            model_kind: model_kind.to_owned(),
            deployment_endpoint: "https://llm.internal/v1".to_owned(),
            api_path: api_path.map(str::to_owned),
            credential_ref: credential_ref.map(str::to_owned),
        }
    }

    fn test_model_chat_response() -> ModelChatResp {
        ModelChatResp {
            conversation_id: None,
            answer: "ok".to_owned(),
            route_id: "runtime.llm".to_owned(),
            provider: "deep-seek".to_owned(),
            model: Some("deepseek-v4-flash".to_owned()),
            latency_ms: 42,
            usage: ModelChatUsage {
                prompt_tokens: Some(1000),
                completion_tokens: Some(2000),
                total_tokens: Some(3000),
            },
            cost_cents: None,
            provider_attempts: vec![],
        }
    }
}
