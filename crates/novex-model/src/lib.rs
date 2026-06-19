mod cost;
mod policy;
mod provider;
mod route;
mod taxonomy;
mod usage;
mod util;

use novex_ai_core::FoundationModule;

pub use cost::{estimate_model_cost_cents, ModelUsageCostInput};
pub use policy::{evaluate_model_route_policy, ModelRoutePolicyInput, ModelRoutePolicyStatus};
pub use provider::{
    ModelEmbeddingVector, ModelMediaImageGenerationResp, ModelProviderStreamChunk, ModelRerankScore,
};
pub use route::{
    ModelRuntimeConfig, ModelRuntimeRoute, ModelRuntimeRouteSummary, ModelRuntimeSummary,
};
pub use taxonomy::{ModelKind, ModelProviderType, ModelRoutePurpose, ModelRuntimeTarget};
pub use usage::{
    estimate_model_text_tokens, normalize_model_provider_usage, ModelTokenUsage,
    ModelTokenUsageCounts,
};
pub use util::mask_api_key;

pub const CRATE_ID: &str = "novex-model";

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Model Registry",
        "ai-foundation",
        "Model providers, deployments, profiles, routing, usage, and health boundaries.",
    )
}
