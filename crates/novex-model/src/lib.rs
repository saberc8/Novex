use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};
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
    target: ModelRuntimeTarget,
    kind: ModelKind,
    provider: ModelProviderType,
    model: Option<String>,
    base_url: String,
    endpoint: String,
    api_key: String,
    purposes: Vec<ModelRoutePurpose>,
    env_keys: Vec<&'static str>,
}

impl fmt::Debug for ModelRuntimeRoute {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModelRuntimeRoute")
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
            route_id: format!("runtime.{}", self.target.as_str()),
            kind: self.kind,
            provider: self.provider,
            model: self.model.clone(),
            base_url: self.base_url.clone(),
            endpoint: self.endpoint.clone(),
            masked_api_key: mask_api_key(&self.api_key),
            purposes: self.purposes.clone(),
            env_keys: self.env_keys.iter().map(|key| (*key).to_owned()).collect(),
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
    let mut env_keys = vec![spec.api_key_env, spec.base_url_env];
    if let Some(model_env) = spec.model_env {
        env_keys.push(model_env);
    }

    routes.push(ModelRuntimeRoute {
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
}
