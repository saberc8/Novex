use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelProviderType {
    OpenAiCompatible,
    AzureOpenAi,
    DashScope,
    DeepSeek,
    LocalRuntime,
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
}
