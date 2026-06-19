use novex_model::{ModelRuntimeConfig, ModelRuntimeTarget};
use serde::{Deserialize, Serialize};

pub const LOCAL_EMBEDDING_ROUTE: &str = "local-keyword";
pub const LOCAL_RERANK_ROUTE: &str = "none";
pub const LOCAL_ANSWER_ROUTE: &str = "local-extractive";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RagModelRoutes {
    pub embedding_model_route: String,
    pub rerank_model_route: String,
    pub answer_model_route: String,
}

impl RagModelRoutes {
    pub fn from_runtime_config(config: &ModelRuntimeConfig) -> Self {
        Self {
            embedding_model_route: runtime_route_id(config, ModelRuntimeTarget::Embedding)
                .unwrap_or_else(|| LOCAL_EMBEDDING_ROUTE.to_owned()),
            rerank_model_route: runtime_route_id(config, ModelRuntimeTarget::Reranker)
                .unwrap_or_else(|| LOCAL_RERANK_ROUTE.to_owned()),
            answer_model_route: runtime_route_id(config, ModelRuntimeTarget::Llm)
                .unwrap_or_else(|| LOCAL_ANSWER_ROUTE.to_owned()),
        }
    }

    pub fn local() -> Self {
        Self {
            embedding_model_route: LOCAL_EMBEDDING_ROUTE.to_owned(),
            rerank_model_route: LOCAL_RERANK_ROUTE.to_owned(),
            answer_model_route: LOCAL_ANSWER_ROUTE.to_owned(),
        }
    }
}

fn runtime_route_id(config: &ModelRuntimeConfig, target: ModelRuntimeTarget) -> Option<String> {
    config.route(target).map(|route| route.summary().route_id)
}
