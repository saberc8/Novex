use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{env, fmt};

pub const CRATE_ID: &str = "novex-model";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Llm,
    Embedding,
    Rerank,
    Vlm,
    Asr,
    Tts,
    MediaGeneration,
}

impl ModelKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Llm => "llm",
            Self::Embedding => "embedding",
            Self::Rerank => "rerank",
            Self::Vlm => "vlm",
            Self::Asr => "asr",
            Self::Tts => "tts",
            Self::MediaGeneration => "media_generation",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match normalize_registry_token(value).as_str() {
            "llm" => Some(Self::Llm),
            "embedding" => Some(Self::Embedding),
            "rerank" | "reranker" => Some(Self::Rerank),
            "vlm" => Some(Self::Vlm),
            "asr" => Some(Self::Asr),
            "tts" => Some(Self::Tts),
            "media_generation" | "media" | "image_generation" => Some(Self::MediaGeneration),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelProviderType {
    OpenAiCompatible,
    AzureOpenAi,
    DashScope,
    DeepSeek,
    LocalRuntime,
    RightCodeDraw,
}

impl ModelProviderType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "openai-compatible",
            Self::AzureOpenAi => "azure-openai",
            Self::DashScope => "dash-scope",
            Self::DeepSeek => "deep-seek",
            Self::LocalRuntime => "local-runtime",
            Self::RightCodeDraw => "right-code-draw",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match normalize_registry_token(value).as_str() {
            "openai_compatible" | "open_ai_compatible" => Some(Self::OpenAiCompatible),
            "azure_openai" | "azure_open_ai" => Some(Self::AzureOpenAi),
            "dash_scope" | "dashscope" => Some(Self::DashScope),
            "deep_seek" | "deepseek" => Some(Self::DeepSeek),
            "local_runtime" => Some(Self::LocalRuntime),
            "right_code_draw" => Some(Self::RightCodeDraw),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRoutePurpose {
    Chat,
    RagAnswer,
    QueryRewrite,
    Embedding,
    Rerank,
    EvalJudge,
    CodeAgent,
    MediaGeneration,
}

impl ModelRoutePurpose {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::RagAnswer => "rag_answer",
            Self::QueryRewrite => "query_rewrite",
            Self::Embedding => "embedding",
            Self::Rerank => "rerank",
            Self::EvalJudge => "eval_judge",
            Self::CodeAgent => "code_agent",
            Self::MediaGeneration => "media_generation",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match normalize_registry_token(value).as_str() {
            "chat" => Some(Self::Chat),
            "rag_answer" | "rag" => Some(Self::RagAnswer),
            "query_rewrite" => Some(Self::QueryRewrite),
            "embedding" => Some(Self::Embedding),
            "rerank" | "reranker" => Some(Self::Rerank),
            "eval_judge" | "judge" => Some(Self::EvalJudge),
            "code_agent" => Some(Self::CodeAgent),
            "media_generation" | "image_generation" => Some(Self::MediaGeneration),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRuntimeTarget {
    Llm,
    Embedding,
    Reranker,
    Draw,
}

impl ModelRuntimeTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Llm => "llm",
            Self::Embedding => "embedding",
            Self::Reranker => "reranker",
            Self::Draw => "draw",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "llm" => Some(Self::Llm),
            "embedding" => Some(Self::Embedding),
            "reranker" | "rerank" => Some(Self::Reranker),
            "draw" | "right_code_draw" | "right-code-draw" => Some(Self::Draw),
            _ => None,
        }
    }

    pub const fn all() -> [Self; 4] {
        [Self::Llm, Self::Embedding, Self::Reranker, Self::Draw]
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ModelRuntimeRoute {
    route_id: String,
    target: ModelRuntimeTarget,
    kind: ModelKind,
    provider: ModelProviderType,
    model: Option<String>,
    base_url: String,
    endpoint: String,
    api_key: String,
    purposes: Vec<ModelRoutePurpose>,
    env_keys: Vec<String>,
}

impl fmt::Debug for ModelRuntimeRoute {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModelRuntimeRoute")
            .field("route_id", &self.route_id)
            .field("target", &self.target)
            .field("kind", &self.kind)
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("endpoint", &self.endpoint)
            .field("masked_api_key", &mask_api_key(&self.api_key))
            .field("purposes", &self.purposes)
            .field("env_keys", &self.env_keys)
            .finish()
    }
}

impl ModelRuntimeRoute {
    pub fn new(
        route_id: impl Into<String>,
        target: ModelRuntimeTarget,
        kind: ModelKind,
        provider: ModelProviderType,
        model: Option<String>,
        base_url: impl Into<String>,
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        purposes: Vec<ModelRoutePurpose>,
        env_keys: Vec<String>,
    ) -> Result<Self, String> {
        let route_id = route_id.into().trim().to_owned();
        let base_url = normalize_base_url(&base_url.into());
        let endpoint = endpoint.into().trim().to_owned();
        let api_key = api_key.into().trim().to_owned();
        if route_id.is_empty() {
            return Err("route_id is required".to_owned());
        }
        if endpoint.is_empty() {
            return Err("endpoint is required".to_owned());
        }
        if api_key.is_empty() {
            return Err("api_key is required".to_owned());
        }

        Ok(Self {
            route_id,
            target,
            kind,
            provider,
            model: model
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            base_url,
            endpoint,
            api_key,
            purposes,
            env_keys,
        })
    }

    pub fn route_id(&self) -> &str {
        &self.route_id
    }

    pub const fn target(&self) -> ModelRuntimeTarget {
        self.target
    }

    pub const fn kind(&self) -> ModelKind {
        self.kind
    }

    pub const fn provider(&self) -> ModelProviderType {
        self.provider
    }

    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn purposes(&self) -> &[ModelRoutePurpose] {
        &self.purposes
    }

    pub fn summary(&self) -> ModelRuntimeRouteSummary {
        ModelRuntimeRouteSummary {
            target: self.target,
            route_id: self.route_id.clone(),
            kind: self.kind,
            provider: self.provider,
            model: self.model.clone(),
            base_url: self.base_url.clone(),
            endpoint: self.endpoint.clone(),
            masked_api_key: mask_api_key(&self.api_key),
            purposes: self.purposes.clone(),
            env_keys: self.env_keys.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRuntimeConfig {
    routes: Vec<ModelRuntimeRoute>,
    missing_env: Vec<String>,
}

impl ModelRuntimeConfig {
    pub fn from_env() -> Self {
        Self::from_env_map(|key| env::var(key).ok())
    }

    pub fn from_env_map<F>(mut get: F) -> Self
    where
        F: FnMut(&str) -> Option<String>,
    {
        let mut routes = Vec::new();
        let mut missing_env = Vec::new();

        add_route(
            &mut routes,
            &mut missing_env,
            &mut get,
            RouteSpec {
                target: ModelRuntimeTarget::Llm,
                kind: ModelKind::Llm,
                provider: ModelProviderType::DeepSeek,
                api_key_env: "LLM_API_KEY",
                base_url_env: "LLM_BASE_URL",
                model_env: Some("LLM_MODEL"),
                endpoint_path: "chat/completions",
                purposes: vec![
                    ModelRoutePurpose::Chat,
                    ModelRoutePurpose::RagAnswer,
                    ModelRoutePurpose::EvalJudge,
                    ModelRoutePurpose::CodeAgent,
                ],
            },
        );
        add_route(
            &mut routes,
            &mut missing_env,
            &mut get,
            RouteSpec {
                target: ModelRuntimeTarget::Embedding,
                kind: ModelKind::Embedding,
                provider: ModelProviderType::DashScope,
                api_key_env: "EMBEDDING_API_KEY",
                base_url_env: "EMBEDDING_BASE_URL",
                model_env: Some("EMBEDDING_MODEL"),
                endpoint_path: "embeddings",
                purposes: vec![ModelRoutePurpose::Embedding],
            },
        );
        add_route(
            &mut routes,
            &mut missing_env,
            &mut get,
            RouteSpec {
                target: ModelRuntimeTarget::Reranker,
                kind: ModelKind::Rerank,
                provider: ModelProviderType::DashScope,
                api_key_env: "RERANKER_API_KEY",
                base_url_env: "RERANKER_BASE_URL",
                model_env: Some("RERANKER_MODEL"),
                endpoint_path: "reranks",
                purposes: vec![ModelRoutePurpose::Rerank],
            },
        );
        add_route(
            &mut routes,
            &mut missing_env,
            &mut get,
            RouteSpec {
                target: ModelRuntimeTarget::Draw,
                kind: ModelKind::MediaGeneration,
                provider: ModelProviderType::RightCodeDraw,
                api_key_env: "RIGHT_CODE_DRAW_API_KEY",
                base_url_env: "RIGHT_CODE_DRAW_BASE_URL",
                model_env: None,
                endpoint_path: "",
                purposes: vec![ModelRoutePurpose::MediaGeneration],
            },
        );

        Self {
            routes,
            missing_env,
        }
    }

    pub fn routes(&self) -> &[ModelRuntimeRoute] {
        &self.routes
    }

    pub fn missing_env(&self) -> &[String] {
        &self.missing_env
    }

    pub fn route(&self, target: ModelRuntimeTarget) -> Option<&ModelRuntimeRoute> {
        self.routes.iter().find(|route| route.target == target)
    }

    pub fn summary(&self) -> ModelRuntimeSummary {
        ModelRuntimeSummary {
            routes: self.routes.iter().map(ModelRuntimeRoute::summary).collect(),
            missing_env: self.missing_env.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRuntimeSummary {
    pub routes: Vec<ModelRuntimeRouteSummary>,
    pub missing_env: Vec<String>,
}

impl ModelRuntimeSummary {
    pub fn route(&self, target: ModelRuntimeTarget) -> Option<&ModelRuntimeRouteSummary> {
        self.routes.iter().find(|route| route.target == target)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRuntimeRouteSummary {
    pub target: ModelRuntimeTarget,
    pub route_id: String,
    pub kind: ModelKind,
    pub provider: ModelProviderType,
    pub model: Option<String>,
    pub base_url: String,
    pub endpoint: String,
    pub masked_api_key: String,
    pub purposes: Vec<ModelRoutePurpose>,
    pub env_keys: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTokenUsage {
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
}

impl ModelTokenUsage {
    pub fn accounting_counts(&self) -> ModelTokenUsageCounts {
        let prompt_tokens = self.prompt_tokens.unwrap_or_default().max(0);
        let completion_tokens = self.completion_tokens.unwrap_or_default().max(0);
        let derived_total = prompt_tokens.saturating_add(completion_tokens);
        let total_tokens = self
            .total_tokens
            .unwrap_or(derived_total)
            .max(0)
            .max(derived_total);

        ModelTokenUsageCounts {
            prompt_tokens,
            completion_tokens,
            total_tokens,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTokenUsageCounts {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageCostInput {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub vector_count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelRoutePolicyInput<'a> {
    pub network_zone: &'a str,
    pub fallback_network_zone: Option<&'a str>,
    pub fallback_policy: &'a Value,
    pub route_policy: &'a Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRoutePolicyStatus {
    pub network_zone: String,
    pub fallback_network_zone: Option<String>,
    pub fallback_enabled: bool,
    pub cross_zone_fallback_allowed: bool,
    pub max_retries: u32,
    pub circuit_breaker_seconds: u32,
    pub violations: Vec<String>,
}

pub fn normalize_model_provider_usage(body: &Value) -> ModelTokenUsage {
    let usage = body.get("usage").unwrap_or(body);
    let prompt_tokens = json_i64_field(
        usage,
        &[
            "prompt_tokens",
            "promptTokens",
            "input_tokens",
            "inputTokens",
        ],
    );
    let completion_tokens = json_i64_field(
        usage,
        &[
            "completion_tokens",
            "completionTokens",
            "output_tokens",
            "outputTokens",
        ],
    );
    let total_tokens = json_i64_field(usage, &["total_tokens", "totalTokens"]).or_else(|| {
        match (prompt_tokens, completion_tokens) {
            (Some(prompt_tokens), Some(completion_tokens)) => {
                Some(prompt_tokens.saturating_add(completion_tokens))
            }
            _ => None,
        }
    });

    ModelTokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
    }
}

pub fn estimate_model_text_tokens(text: &str) -> i32 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }

    let whitespace_count = trimmed.split_whitespace().count();
    if whitespace_count > 1 {
        whitespace_count.min(i32::MAX as usize) as i32
    } else {
        trimmed.chars().count().min(i32::MAX as usize) as i32
    }
}

pub fn estimate_model_cost_cents(cost_spec: &Value, input: &ModelUsageCostInput) -> f64 {
    let unit = json_string_field(cost_spec, &["unit"])
        .unwrap_or_default()
        .to_ascii_lowercase();
    let request_cost = non_negative(input.request_count)
        * json_f64_field(
            cost_spec,
            &["requestCents", "request_cents", "centsPerRequest"],
        )
        .unwrap_or_default();

    let cost = match unit.as_str() {
        "token" | "tokens" => token_cost_cents(cost_spec, input) + request_cost,
        "request" | "requests" => request_cost,
        "vector" | "vectors" => {
            non_negative(input.vector_count)
                * json_f64_field(
                    cost_spec,
                    &["vectorCents", "vector_cents", "centsPerVector"],
                )
                .unwrap_or_default()
        }
        _ => request_cost,
    };

    if cost.is_finite() {
        cost.max(0.0)
    } else {
        0.0
    }
}

pub fn evaluate_model_route_policy(input: ModelRoutePolicyInput<'_>) -> ModelRoutePolicyStatus {
    let network_zone = normalize_network_zone(input.network_zone);
    let fallback_network_zone = input.fallback_network_zone.map(normalize_network_zone);
    let fallback_enabled = policy_bool_field(
        input.route_policy,
        input.fallback_policy,
        &["fallbackEnabled", "fallback_enabled", "enabled"],
    )
    .unwrap_or(false);
    let cross_zone_fallback_allowed = policy_bool_field(
        input.route_policy,
        input.fallback_policy,
        &[
            "allowCrossZone",
            "allow_cross_zone",
            "allowCrossNetworkZone",
            "allow_cross_network_zone",
            "crossZoneFallback",
            "cross_zone_fallback",
        ],
    )
    .unwrap_or(false);
    let max_retries = policy_u32_field(
        input.route_policy,
        input.fallback_policy,
        &["maxRetries", "max_retries", "retryCount", "retry_count"],
    )
    .unwrap_or(0);
    let circuit_breaker_seconds = policy_u32_field(
        input.route_policy,
        input.fallback_policy,
        &[
            "circuitBreakerSeconds",
            "circuit_breaker_seconds",
            "circuitBreakerCooldownSeconds",
            "circuit_breaker_cooldown_seconds",
        ],
    )
    .unwrap_or(0);

    let mut violations = Vec::new();
    if fallback_enabled
        && fallback_network_zone
            .as_deref()
            .is_some_and(|fallback_zone| fallback_zone != network_zone)
        && !cross_zone_fallback_allowed
    {
        violations.push("cross_zone_fallback_not_allowed".to_owned());
    }

    ModelRoutePolicyStatus {
        network_zone,
        fallback_network_zone,
        fallback_enabled,
        cross_zone_fallback_allowed,
        max_retries,
        circuit_breaker_seconds,
        violations,
    }
}

fn token_cost_cents(cost_spec: &Value, input: &ModelUsageCostInput) -> f64 {
    let prompt_rate = json_f64_field(
        cost_spec,
        &[
            "promptCentsPer1kTokens",
            "promptTokenCentsPer1k",
            "inputCentsPer1kTokens",
            "inputTokenCentsPer1k",
        ],
    );
    let completion_rate = json_f64_field(
        cost_spec,
        &[
            "completionCentsPer1kTokens",
            "completionTokenCentsPer1k",
            "outputCentsPer1kTokens",
            "outputTokenCentsPer1k",
        ],
    );
    if prompt_rate.is_some() || completion_rate.is_some() {
        return non_negative(input.prompt_tokens) * prompt_rate.unwrap_or_default() / 1000.0
            + non_negative(input.completion_tokens) * completion_rate.unwrap_or_default() / 1000.0;
    }

    non_negative(input.total_tokens)
        * json_f64_field(
            cost_spec,
            &["totalCentsPer1kTokens", "totalTokenCentsPer1k"],
        )
        .unwrap_or_default()
        / 1000.0
}

fn json_i64_field(value: &Value, keys: &[&str]) -> Option<i64> {
    json_field(value, keys).and_then(json_i64)
}

fn json_f64_field(value: &Value, keys: &[&str]) -> Option<f64> {
    json_field(value, keys).and_then(json_f64)
}

fn json_string_field(value: &Value, keys: &[&str]) -> Option<String> {
    json_field(value, keys)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn json_bool_field(value: &Value, keys: &[&str]) -> Option<bool> {
    json_field(value, keys).and_then(json_bool)
}

fn policy_bool_field(route_policy: &Value, fallback_policy: &Value, keys: &[&str]) -> Option<bool> {
    json_bool_field(route_policy, keys).or_else(|| json_bool_field(fallback_policy, keys))
}

fn policy_u32_field(route_policy: &Value, fallback_policy: &Value, keys: &[&str]) -> Option<u32> {
    json_i64_field(route_policy, keys)
        .or_else(|| json_i64_field(fallback_policy, keys))
        .map(|value| value.min(u32::MAX as i64) as u32)
}

fn json_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let object = value.as_object()?;
    for key in keys {
        if let Some(value) = object.get(*key) {
            return Some(value);
        }
    }

    let normalized_keys = keys
        .iter()
        .map(|key| normalize_json_key(key))
        .collect::<Vec<_>>();
    object.iter().find_map(|(key, value)| {
        normalized_keys
            .iter()
            .any(|expected| *expected == normalize_json_key(key))
            .then_some(value)
    })
}

fn normalize_json_key(key: &str) -> String {
    key.chars()
        .filter(|ch| !matches!(ch, '_' | '-'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn json_i64(value: &Value) -> Option<i64> {
    let parsed = value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_str()?.trim().parse::<i64>().ok())?;
    Some(parsed.max(0))
}

fn json_f64(value: &Value) -> Option<f64> {
    let parsed = value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())?;
    parsed.is_finite().then_some(parsed.max(0.0))
}

fn json_bool(value: &Value) -> Option<bool> {
    if let Some(value) = value.as_bool() {
        return Some(value);
    }
    if let Some(value) = value.as_i64() {
        return Some(value > 0);
    }
    if let Some(value) = value.as_u64() {
        return Some(value > 0);
    }

    match value.as_str()?.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "y" | "enabled" | "allow" | "allowed" => Some(true),
        "false" | "0" | "no" | "n" | "disabled" | "deny" | "denied" => Some(false),
        _ => None,
    }
}

fn normalize_network_zone(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() {
        "unknown".to_owned()
    } else {
        value
    }
}

fn normalize_registry_token(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

fn non_negative(value: i64) -> f64 {
    value.max(0) as f64
}

struct RouteSpec {
    target: ModelRuntimeTarget,
    kind: ModelKind,
    provider: ModelProviderType,
    api_key_env: &'static str,
    base_url_env: &'static str,
    model_env: Option<&'static str>,
    endpoint_path: &'static str,
    purposes: Vec<ModelRoutePurpose>,
}

fn add_route<F>(
    routes: &mut Vec<ModelRuntimeRoute>,
    missing_env: &mut Vec<String>,
    get: &mut F,
    spec: RouteSpec,
) where
    F: FnMut(&str) -> Option<String>,
{
    let api_key = read_env(get, missing_env, spec.api_key_env);
    let base_url = read_env(get, missing_env, spec.base_url_env);
    let model = if let Some(model_env) = spec.model_env {
        read_env(get, missing_env, model_env).map(Some)
    } else {
        Some(None)
    };

    let (Some(api_key), Some(base_url), Some(model)) = (api_key, base_url, model) else {
        return;
    };

    let base_url = normalize_base_url(&base_url);
    let endpoint = join_url(&base_url, spec.endpoint_path);
    let mut env_keys = vec![spec.api_key_env.to_owned(), spec.base_url_env.to_owned()];
    if let Some(model_env) = spec.model_env {
        env_keys.push(model_env.to_owned());
    }

    routes.push(ModelRuntimeRoute {
        route_id: format!("runtime.{}", spec.target.as_str()),
        target: spec.target,
        kind: spec.kind,
        provider: spec.provider,
        model,
        base_url,
        endpoint,
        api_key,
        purposes: spec.purposes,
        env_keys,
    });
}

fn read_env<F>(get: &mut F, missing_env: &mut Vec<String>, key: &'static str) -> Option<String>
where
    F: FnMut(&str) -> Option<String>,
{
    let value = get(key)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if value.is_none() {
        missing_env.push(key.to_owned());
    }
    value
}

fn normalize_base_url(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').to_owned()
}

fn join_url(base_url: &str, path: &str) -> String {
    let base_url = normalize_base_url(base_url);
    let path = path.trim().trim_matches('/');
    if path.is_empty() {
        base_url
    } else {
        format!("{base_url}/{path}")
    }
}

pub fn mask_api_key(api_key: &str) -> String {
    let chars = api_key.chars().collect::<Vec<_>>();
    if chars.len() <= 8 {
        return "****".to_owned();
    }

    let prefix = chars.iter().take(3).collect::<String>();
    let suffix = chars
        .iter()
        .skip(chars.len().saturating_sub(4))
        .collect::<String>();
    format!("{prefix}****{suffix}")
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Model Registry",
        "ai-foundation",
        "Model providers, deployments, profiles, routing, usage, and health boundaries.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::FoundationStatus;

    #[test]
    fn module_describes_model_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-model");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }

    #[test]
    fn runtime_config_maps_user_env_to_masked_routes() {
        let env = [
            ("LLM_API_KEY", "sk-fake-llm-secret-508d"),
            ("LLM_BASE_URL", "https://api.deepseek.com"),
            ("LLM_MODEL", "deepseek-v4-flash"),
            ("EMBEDDING_API_KEY", "sk-fake-embedding-secret-ffff"),
            (
                "EMBEDDING_BASE_URL",
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
            ),
            ("EMBEDDING_MODEL", "text-embedding-v4"),
            ("RERANKER_API_KEY", "sk-fake-reranker-secret-ffff"),
            (
                "RERANKER_BASE_URL",
                "https://dashscope.aliyuncs.com/compatible-api/v1",
            ),
            ("RERANKER_MODEL", "qwen3-rerank"),
            ("RIGHT_CODE_DRAW_API_KEY", "sk-fake-draw-secret-2064"),
            ("RIGHT_CODE_DRAW_BASE_URL", "https://www.right.codes/draw"),
        ];
        let config = ModelRuntimeConfig::from_env_map(|key| {
            env.iter()
                .find_map(|(env_key, value)| (*env_key == key).then(|| (*value).to_owned()))
        });

        let summary = config.summary();

        assert!(summary.missing_env.is_empty());
        assert_eq!(summary.routes.len(), 4);

        let llm = summary.route(ModelRuntimeTarget::Llm).unwrap();
        assert_eq!(llm.provider, ModelProviderType::DeepSeek);
        assert_eq!(llm.kind, ModelKind::Llm);
        assert_eq!(llm.model.as_deref(), Some("deepseek-v4-flash"));
        assert_eq!(llm.endpoint, "https://api.deepseek.com/chat/completions");
        assert_eq!(llm.masked_api_key, "sk-****508d");
        assert_eq!(
            llm.purposes,
            vec![
                ModelRoutePurpose::Chat,
                ModelRoutePurpose::RagAnswer,
                ModelRoutePurpose::EvalJudge,
                ModelRoutePurpose::CodeAgent,
            ]
        );

        let embedding = summary.route(ModelRuntimeTarget::Embedding).unwrap();
        assert_eq!(
            embedding.endpoint,
            "https://dashscope.aliyuncs.com/compatible-mode/v1/embeddings"
        );
        assert_eq!(embedding.masked_api_key, "sk-****ffff");

        let reranker = summary.route(ModelRuntimeTarget::Reranker).unwrap();
        assert_eq!(
            reranker.endpoint,
            "https://dashscope.aliyuncs.com/compatible-api/v1/reranks"
        );

        let draw = summary.route(ModelRuntimeTarget::Draw).unwrap();
        assert_eq!(draw.provider, ModelProviderType::RightCodeDraw);
        assert_eq!(draw.kind, ModelKind::MediaGeneration);
        assert_eq!(draw.model, None);
        assert_eq!(draw.endpoint, "https://www.right.codes/draw");

        let debug = format!("{config:?}");
        assert!(!debug.contains("sk-fake-llm-secret-508d"));
        assert!(debug.contains("sk-****508d"));
    }

    #[test]
    fn runtime_config_reports_missing_env_without_creating_partial_routes() {
        let config = ModelRuntimeConfig::from_env_map(|key| {
            (key == "LLM_API_KEY").then(|| "sk-fake-llm-secret-508d".to_owned())
        });

        let summary = config.summary();

        assert!(summary.routes.is_empty());
        assert_eq!(
            summary.missing_env,
            vec![
                "LLM_BASE_URL",
                "LLM_MODEL",
                "EMBEDDING_API_KEY",
                "EMBEDDING_BASE_URL",
                "EMBEDDING_MODEL",
                "RERANKER_API_KEY",
                "RERANKER_BASE_URL",
                "RERANKER_MODEL",
                "RIGHT_CODE_DRAW_API_KEY",
                "RIGHT_CODE_DRAW_BASE_URL",
            ]
        );
    }

    #[test]
    fn dynamic_route_constructor_preserves_registry_route_id() {
        let route = ModelRuntimeRoute::new(
            "tenant42.rag_answer",
            ModelRuntimeTarget::Llm,
            ModelKind::Llm,
            ModelProviderType::OpenAiCompatible,
            Some("qwen-private".to_owned()),
            "https://llm.internal/v1",
            "https://llm.internal/v1/chat/completions",
            "sk-fake-private-secret-0001",
            vec![ModelRoutePurpose::RagAnswer],
            vec!["LLM_PRIVATE_KEY".to_owned()],
        )
        .unwrap();

        let summary = route.summary();

        assert_eq!(summary.route_id, "tenant42.rag_answer");
        assert_eq!(summary.target, ModelRuntimeTarget::Llm);
        assert_eq!(summary.provider, ModelProviderType::OpenAiCompatible);
        assert_eq!(summary.model.as_deref(), Some("qwen-private"));
        assert_eq!(summary.masked_api_key, "sk-****0001");
        assert_eq!(summary.env_keys, vec!["LLM_PRIVATE_KEY"]);
        assert!(!format!("{route:?}").contains("sk-fake-private-secret-0001"));
    }

    #[test]
    fn dynamic_route_parsers_accept_registry_values() {
        assert_eq!(
            ModelRoutePurpose::parse("rag_answer"),
            Some(ModelRoutePurpose::RagAnswer)
        );
        assert_eq!(
            ModelRoutePurpose::parse("rerank"),
            Some(ModelRoutePurpose::Rerank)
        );
        assert_eq!(ModelRoutePurpose::Chat.as_str(), "chat");
        assert_eq!(
            ModelKind::parse("media_generation"),
            Some(ModelKind::MediaGeneration)
        );
        assert_eq!(
            ModelProviderType::parse("openai-compatible"),
            Some(ModelProviderType::OpenAiCompatible)
        );
        assert_eq!(
            ModelProviderType::parse("deep-seek"),
            Some(ModelProviderType::DeepSeek)
        );
    }

    #[test]
    fn model_usage_normalizes_provider_token_aliases_and_estimates_text_tokens() {
        let body = serde_json::json!({
            "usage": {
                "input_tokens": "11",
                "outputTokens": 7
            }
        });

        let usage = normalize_model_provider_usage(&body);

        assert_eq!(usage.prompt_tokens, Some(11));
        assert_eq!(usage.completion_tokens, Some(7));
        assert_eq!(usage.total_tokens, Some(18));
        assert_eq!(usage.accounting_counts().total_tokens, 18);
        assert_eq!(estimate_model_text_tokens("hello world"), 2);
        assert_eq!(estimate_model_text_tokens("你好"), 2);
    }

    #[test]
    fn model_usage_cost_estimate_applies_token_cost_spec() {
        let cost_spec = serde_json::json!({
            "unit": "token",
            "promptCentsPer1kTokens": 0.2,
            "completionCentsPer1kTokens": 0.8,
            "requestCents": 0.05
        });
        let input = ModelUsageCostInput {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
            request_count: 1,
            vector_count: 0,
        };

        let cost_cents = estimate_model_cost_cents(&cost_spec, &input);

        assert!((cost_cents - 0.65).abs() < 0.000_001);
    }

    #[test]
    fn model_route_policy_defaults_to_disabled_fallback() {
        let status = evaluate_model_route_policy(ModelRoutePolicyInput {
            network_zone: "public",
            fallback_network_zone: None,
            fallback_policy: &Value::Null,
            route_policy: &Value::Null,
        });

        assert_eq!(status.network_zone, "public");
        assert!(!status.fallback_enabled);
        assert!(!status.cross_zone_fallback_allowed);
        assert_eq!(status.max_retries, 0);
        assert_eq!(status.circuit_breaker_seconds, 0);
        assert!(status.violations.is_empty());
    }

    #[test]
    fn model_route_policy_blocks_cross_zone_fallback_without_explicit_policy() {
        let policy = serde_json::json!({
            "enabled": true,
            "maxRetries": 2,
            "circuitBreakerSeconds": 45
        });

        let status = evaluate_model_route_policy(ModelRoutePolicyInput {
            network_zone: "private",
            fallback_network_zone: Some("public"),
            fallback_policy: &policy,
            route_policy: &Value::Null,
        });

        assert!(status.fallback_enabled);
        assert_eq!(status.max_retries, 2);
        assert_eq!(status.circuit_breaker_seconds, 45);
        assert!(!status.cross_zone_fallback_allowed);
        assert_eq!(
            status.violations,
            vec!["cross_zone_fallback_not_allowed".to_owned()]
        );
    }

    #[test]
    fn model_route_policy_allows_cross_zone_fallback_when_policy_explicit() {
        let policy = serde_json::json!({
            "enabled": true,
            "allowCrossZone": true,
            "max_retries": 1,
            "circuit_breaker_seconds": 30
        });

        let status = evaluate_model_route_policy(ModelRoutePolicyInput {
            network_zone: "private",
            fallback_network_zone: Some("public"),
            fallback_policy: &policy,
            route_policy: &Value::Null,
        });

        assert!(status.fallback_enabled);
        assert!(status.cross_zone_fallback_allowed);
        assert_eq!(status.max_retries, 1);
        assert_eq!(status.circuit_breaker_seconds, 30);
        assert!(status.violations.is_empty());
    }
}
