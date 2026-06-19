mod case;
mod report;
mod score;
mod text;
mod trace_extract;

use novex_ai_core::FoundationModule;

pub use case::{
    EvalCaseActual, EvalCaseCandidate, EvalCaseExpected, EvalCaseInput, EvalMetricKind,
    EvalTargetKind, TraceEvalPolicy,
};
pub use report::{build_regression_report, RegressionReport};
pub use score::{
    score_case, score_cost_case, score_customer_service_grounded_resolution_case,
    score_customer_service_handoff_accuracy_case, score_intent_case, score_latency_case,
    score_rag_case, score_retrieval_recall_case, score_tool_case, EvalCaseScore,
};
pub use trace_extract::actual_from_trace_bundle;

pub const CRATE_ID: &str = "novex-eval";

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Eval",
        "ai-foundation",
        "Eval dataset, case, runner, metrics, report, and regression boundary.",
    )
}
