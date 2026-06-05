use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-eval";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalTargetKind {
    Rag,
    Intent,
    Tool,
    ReAct,
    Safety,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalMetricKind {
    RetrievalRecall,
    CitationAccuracy,
    Faithfulness,
    IntentAccuracy,
    ToolAccuracy,
    Cost,
    Latency,
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Eval",
        "ai-foundation",
        "Eval dataset, case, runner, metrics, report, and regression boundary.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::FoundationStatus;

    #[test]
    fn module_describes_eval_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-eval");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }
}
