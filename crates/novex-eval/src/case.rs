use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalTargetKind {
    Rag,
    Intent,
    Tool,
    ReAct,
    Safety,
    CustomerService,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalMetricKind {
    RetrievalRecall,
    CitationAccuracy,
    Faithfulness,
    IntentAccuracy,
    ToolAccuracy,
    Cost,
    Latency,
    GroundedResolution,
    HandoffAccuracy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalCaseInput {
    pub target_kind: EvalTargetKind,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct EvalCaseExpected {
    pub answer_contains: Vec<String>,
    pub citations: Vec<String>,
    pub intent: Option<String>,
    pub tool_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct EvalCaseActual {
    pub answer: Option<String>,
    pub citations: Vec<String>,
    pub intent: Option<String>,
    pub tool_code: Option<String>,
    pub cost_cents: u32,
    pub latency_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceEvalPolicy {
    pub answer_snippet_max_chars: usize,
    pub include_latency_cost_tags: bool,
}

impl Default for TraceEvalPolicy {
    fn default() -> Self {
        Self {
            answer_snippet_max_chars: 120,
            include_latency_cost_tags: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalCaseCandidate {
    pub target_kind: EvalTargetKind,
    pub metric_kind: EvalMetricKind,
    pub prompt: String,
    pub expected: EvalCaseExpected,
    pub tags: Value,
}
