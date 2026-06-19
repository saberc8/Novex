use crate::util::normalize_registry_token;
use serde::{Deserialize, Serialize};

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
    GuardianReview,
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
            Self::GuardianReview => "guardian_review",
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
            "guardian_review" | "guardian" => Some(Self::GuardianReview),
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
