use crate::taxonomy::{ModelKind, ModelProviderType, ModelRoutePurpose, ModelRuntimeTarget};
use crate::util::{join_url, mask_api_key, normalize_base_url};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::{env, fmt};

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
        let purpose_route_ids = self
            .purposes
            .iter()
            .map(|purpose| (purpose.as_str().to_owned(), self.route_id.clone()))
            .collect();

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
            purpose_route_ids,
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
                    ModelRoutePurpose::GuardianReview,
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
    pub purpose_route_ids: BTreeMap<String, String>,
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
