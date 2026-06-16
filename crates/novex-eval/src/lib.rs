use std::collections::BTreeMap;

use novex_ai_core::FoundationModule;
use novex_trace::{TraceBundle, TraceEventKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const CRATE_ID: &str = "novex-eval";

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

impl EvalCaseCandidate {
    pub fn from_trace_bundle(bundle: &TraceBundle) -> Self {
        Self::from_trace_bundle_with_policy(bundle, TraceEvalPolicy::default())
    }

    pub fn from_trace_bundle_with_policy(bundle: &TraceBundle, policy: TraceEvalPolicy) -> Self {
        let prompt = trace_event_payload_text(bundle, TraceEventKind::UserMessage, "content")
            .unwrap_or_default();
        let tool_code = trace_event_payload_text(bundle, TraceEventKind::ToolCall, "toolCode");
        let final_answer =
            trace_last_event_payload_text(bundle, TraceEventKind::FinalAnswer, "answer");
        let answer_contains = final_answer
            .as_deref()
            .map(|answer| trace_answer_snippet(answer, policy.answer_snippet_max_chars))
            .filter(|answer| !answer.is_empty())
            .into_iter()
            .collect();
        let citations = trace_bundle_citations(bundle);
        let summary = bundle.replay_summary();
        let mut tags = serde_json::Map::new();
        tags.insert("source".to_owned(), json!("agent_trace"));
        tags.insert("traceId".to_owned(), json!(bundle.trace_id));
        tags.insert("toolCallCount".to_owned(), json!(summary.tool_call_count));
        tags.insert("finalStatus".to_owned(), json!(summary.final_status));
        tags.insert(
            "hasApprovalPause".to_owned(),
            json!(summary.has_approval_pause),
        );
        if let Some(tool_code) = tool_code.as_deref() {
            tags.insert("toolCode".to_owned(), json!(tool_code));
        }
        let retrieval_count = trace_event_count(bundle, TraceEventKind::Retrieval);
        let compaction_count = trace_event_count(bundle, TraceEventKind::ContextCompaction);
        let cancelled = trace_event_count(bundle, TraceEventKind::Cancellation) > 0;
        tags.insert("retrievalCount".to_owned(), json!(retrieval_count));
        tags.insert("compactionCount".to_owned(), json!(compaction_count));
        tags.insert("cancelled".to_owned(), json!(cancelled));
        if let Some(cancel_reason) = trace_first_cancellation_reason(bundle) {
            tags.insert("cancelReason".to_owned(), json!(cancel_reason));
        }
        let inference_summary = trace_inference_summary(bundle);
        tags.insert("inferenceCount".to_owned(), json!(inference_summary.count));
        if let Some(route_id) = inference_summary.route_id.as_deref() {
            tags.insert("modelRouteId".to_owned(), json!(route_id));
        }
        if let Some(provider) = inference_summary.provider.as_deref() {
            tags.insert("modelProvider".to_owned(), json!(provider));
        }
        if let Some(model) = inference_summary.model.as_deref() {
            tags.insert("modelName".to_owned(), json!(model));
        }
        if inference_summary.count > 0 {
            tags.insert(
                "promptTokens".to_owned(),
                json!(inference_summary.prompt_tokens),
            );
            tags.insert(
                "completionTokens".to_owned(),
                json!(inference_summary.completion_tokens),
            );
            tags.insert(
                "totalTokens".to_owned(),
                json!(inference_summary.total_tokens),
            );
        }
        if policy.include_latency_cost_tags {
            tags.insert(
                "latencyMs".to_owned(),
                if inference_summary.count > 0 {
                    json!(inference_summary.latency_ms)
                } else {
                    Value::Null
                },
            );
            tags.insert(
                "costCents".to_owned(),
                inference_summary
                    .cost_cents
                    .map(|cost_cents| json!(cost_cents))
                    .unwrap_or(Value::Null),
            );
        }

        Self {
            target_kind: EvalTargetKind::ReAct,
            metric_kind: if tool_code.is_some() {
                EvalMetricKind::ToolAccuracy
            } else {
                EvalMetricKind::Faithfulness
            },
            prompt,
            expected: EvalCaseExpected {
                answer_contains,
                citations,
                intent: None,
                tool_code,
            },
            tags: Value::Object(tags),
        }
    }
}

pub fn actual_from_trace_bundle(bundle: &TraceBundle) -> EvalCaseActual {
    EvalCaseActual {
        answer: trace_last_event_payload_text(bundle, TraceEventKind::FinalAnswer, "answer"),
        citations: trace_bundle_citations(bundle),
        intent: None,
        tool_code: trace_event_payload_text(bundle, TraceEventKind::ToolCall, "toolCode"),
        cost_cents: 0,
        latency_ms: 0,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalCaseScore {
    pub case_id: String,
    pub target_kind: EvalTargetKind,
    pub metric: EvalMetricKind,
    pub score: f64,
    pub passed: bool,
    pub reason: String,
    pub cost_cents: u32,
    pub latency_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionReport {
    pub total_cases: usize,
    pub passed_cases: usize,
    pub failed_cases: usize,
    pub average_score: f64,
    pub metric_breakdown: BTreeMap<EvalMetricKind, f64>,
    pub total_cost_cents: u32,
    pub total_latency_ms: u32,
}

pub fn score_case(
    case_id: impl Into<String>,
    target_kind: EvalTargetKind,
    expected: &EvalCaseExpected,
    actual: &EvalCaseActual,
) -> EvalCaseScore {
    let mut score = match target_kind {
        EvalTargetKind::Rag => score_rag_case(expected, actual),
        EvalTargetKind::Intent => score_intent_case(expected, actual),
        EvalTargetKind::Tool => score_tool_case(expected, actual),
        EvalTargetKind::CustomerService => {
            score_customer_service_grounded_resolution_case(String::new(), expected, actual)
        }
        EvalTargetKind::ReAct | EvalTargetKind::Safety => {
            score_exact_answer_case(target_kind, expected, actual)
        }
    };
    score.case_id = case_id.into();
    score
}

pub fn score_rag_case(expected: &EvalCaseExpected, actual: &EvalCaseActual) -> EvalCaseScore {
    let answer_ok = expected.answer_contains.iter().all(|needle| {
        actual
            .answer
            .as_deref()
            .is_some_and(|answer| contains_case_insensitive(answer, needle))
    });
    let citation_ok = expected
        .citations
        .iter()
        .all(|citation| actual.citations.iter().any(|actual| actual == citation));
    let score = match (answer_ok, citation_ok) {
        (true, true) => 1.0,
        (true, false) | (false, true) => 0.5,
        (false, false) => 0.0,
    };

    EvalCaseScore {
        case_id: String::new(),
        target_kind: EvalTargetKind::Rag,
        metric: EvalMetricKind::CitationAccuracy,
        score,
        passed: score >= 1.0,
        reason: if score >= 1.0 {
            "answer and citation matched".to_owned()
        } else {
            "answer or citation mismatch".to_owned()
        },
        cost_cents: actual.cost_cents,
        latency_ms: actual.latency_ms,
    }
}

pub fn score_intent_case(expected: &EvalCaseExpected, actual: &EvalCaseActual) -> EvalCaseScore {
    let passed = expected.intent == actual.intent;
    EvalCaseScore {
        case_id: String::new(),
        target_kind: EvalTargetKind::Intent,
        metric: EvalMetricKind::IntentAccuracy,
        score: if passed { 1.0 } else { 0.0 },
        passed,
        reason: if passed {
            "intent matched".to_owned()
        } else {
            "intent mismatch".to_owned()
        },
        cost_cents: actual.cost_cents,
        latency_ms: actual.latency_ms,
    }
}

pub fn score_tool_case(expected: &EvalCaseExpected, actual: &EvalCaseActual) -> EvalCaseScore {
    let passed = expected.tool_code == actual.tool_code;
    EvalCaseScore {
        case_id: String::new(),
        target_kind: EvalTargetKind::Tool,
        metric: EvalMetricKind::ToolAccuracy,
        score: if passed { 1.0 } else { 0.0 },
        passed,
        reason: if passed {
            "tool matched".to_owned()
        } else {
            "tool mismatch".to_owned()
        },
        cost_cents: actual.cost_cents,
        latency_ms: actual.latency_ms,
    }
}

pub fn score_customer_service_grounded_resolution_case(
    case_id: impl Into<String>,
    expected: &EvalCaseExpected,
    actual: &EvalCaseActual,
) -> EvalCaseScore {
    let answer_ok = expected.answer_contains.iter().all(|needle| {
        actual
            .answer
            .as_deref()
            .is_some_and(|answer| contains_case_insensitive(answer, needle))
    });
    let expects_insufficient_evidence = expected
        .answer_contains
        .iter()
        .any(|needle| contains_case_insensitive(needle, "insufficient evidence"));
    let citation_ok = if expected.citations.is_empty() {
        !expects_insufficient_evidence || actual.citations.is_empty()
    } else {
        expected
            .citations
            .iter()
            .all(|citation| actual.citations.iter().any(|actual| actual == citation))
    };
    let score = match (answer_ok, citation_ok) {
        (true, true) => 1.0,
        (true, false) => 0.5,
        (false, _) => 0.0,
    };

    EvalCaseScore {
        case_id: case_id.into(),
        target_kind: EvalTargetKind::CustomerService,
        metric: EvalMetricKind::GroundedResolution,
        score,
        passed: score >= 1.0,
        reason: if score >= 1.0 {
            if expects_insufficient_evidence {
                "insufficient evidence handled".to_owned()
            } else {
                "customer service answer grounded".to_owned()
            }
        } else if !citation_ok {
            "missing evidence or citation".to_owned()
        } else {
            "customer service answer mismatch".to_owned()
        },
        cost_cents: actual.cost_cents,
        latency_ms: actual.latency_ms,
    }
}

pub fn score_customer_service_handoff_accuracy_case(
    case_id: impl Into<String>,
    expected: &EvalCaseExpected,
    actual: &EvalCaseActual,
) -> EvalCaseScore {
    let expected_tool_ok = expected
        .tool_code
        .as_deref()
        .is_none_or(|expected| actual.tool_code.as_deref() == Some(expected));
    let expected_intent_ok = expected
        .intent
        .as_deref()
        .is_none_or(|expected| actual.intent.as_deref() == Some(expected));
    let passed = expected_tool_ok && expected_intent_ok;

    EvalCaseScore {
        case_id: case_id.into(),
        target_kind: EvalTargetKind::CustomerService,
        metric: EvalMetricKind::HandoffAccuracy,
        score: if passed { 1.0 } else { 0.0 },
        passed,
        reason: if passed {
            "handoff matched".to_owned()
        } else {
            "handoff mismatch".to_owned()
        },
        cost_cents: actual.cost_cents,
        latency_ms: actual.latency_ms,
    }
}

pub fn score_latency_case(
    case_id: impl Into<String>,
    target_kind: EvalTargetKind,
    actual: &EvalCaseActual,
    max_latency_ms: u32,
) -> EvalCaseScore {
    let passed = actual.latency_ms <= max_latency_ms;
    EvalCaseScore {
        case_id: case_id.into(),
        target_kind,
        metric: EvalMetricKind::Latency,
        score: if passed { 1.0 } else { 0.0 },
        passed,
        reason: if passed {
            format!(
                "latency {}ms within {}ms",
                actual.latency_ms, max_latency_ms
            )
        } else {
            format!(
                "latency {}ms exceeded {}ms",
                actual.latency_ms, max_latency_ms
            )
        },
        cost_cents: actual.cost_cents,
        latency_ms: actual.latency_ms,
    }
}

pub fn score_cost_case(
    case_id: impl Into<String>,
    target_kind: EvalTargetKind,
    actual: &EvalCaseActual,
    max_cost_cents: u32,
) -> EvalCaseScore {
    let passed = actual.cost_cents <= max_cost_cents;
    EvalCaseScore {
        case_id: case_id.into(),
        target_kind,
        metric: EvalMetricKind::Cost,
        score: if passed { 1.0 } else { 0.0 },
        passed,
        reason: if passed {
            format!("cost {}c within {}c", actual.cost_cents, max_cost_cents)
        } else {
            format!("cost {}c exceeded {}c", actual.cost_cents, max_cost_cents)
        },
        cost_cents: actual.cost_cents,
        latency_ms: actual.latency_ms,
    }
}

pub fn score_retrieval_recall_case(
    case_id: impl Into<String>,
    target_kind: EvalTargetKind,
    expected: &EvalCaseExpected,
    actual: &EvalCaseActual,
) -> EvalCaseScore {
    let passed = expected
        .citations
        .iter()
        .all(|citation| actual.citations.iter().any(|actual| actual == citation));
    EvalCaseScore {
        case_id: case_id.into(),
        target_kind,
        metric: EvalMetricKind::RetrievalRecall,
        score: if passed { 1.0 } else { 0.0 },
        passed,
        reason: if passed {
            "retrieval references matched".to_owned()
        } else {
            "retrieval references missing".to_owned()
        },
        cost_cents: actual.cost_cents,
        latency_ms: actual.latency_ms,
    }
}

pub fn build_regression_report(scores: &[EvalCaseScore]) -> RegressionReport {
    let total_cases = scores.len();
    let passed_cases = scores.iter().filter(|score| score.passed).count();
    let failed_cases = total_cases.saturating_sub(passed_cases);
    let average_score = if total_cases == 0 {
        0.0
    } else {
        round_score(scores.iter().map(|score| score.score).sum::<f64>() / total_cases as f64)
    };
    let mut metric_totals = BTreeMap::<EvalMetricKind, (f64, usize)>::new();
    for score in scores {
        let entry = metric_totals.entry(score.metric).or_insert((0.0, 0));
        entry.0 += score.score;
        entry.1 += 1;
    }
    let metric_breakdown = metric_totals
        .into_iter()
        .map(|(metric, (total, count))| (metric, round_score(total / count as f64)))
        .collect();

    RegressionReport {
        total_cases,
        passed_cases,
        failed_cases,
        average_score,
        metric_breakdown,
        total_cost_cents: scores.iter().map(|score| score.cost_cents).sum(),
        total_latency_ms: scores.iter().map(|score| score.latency_ms).sum(),
    }
}

fn score_exact_answer_case(
    target_kind: EvalTargetKind,
    expected: &EvalCaseExpected,
    actual: &EvalCaseActual,
) -> EvalCaseScore {
    let passed = expected.answer_contains.iter().all(|needle| {
        actual
            .answer
            .as_deref()
            .is_some_and(|answer| contains_case_insensitive(answer, needle))
    });
    EvalCaseScore {
        case_id: String::new(),
        target_kind,
        metric: EvalMetricKind::Faithfulness,
        score: if passed { 1.0 } else { 0.0 },
        passed,
        reason: if passed {
            "answer matched".to_owned()
        } else {
            "answer mismatch".to_owned()
        },
        cost_cents: actual.cost_cents,
        latency_ms: actual.latency_ms,
    }
}

fn contains_case_insensitive(value: &str, needle: &str) -> bool {
    value.to_lowercase().contains(&needle.to_lowercase())
}

fn round_score(score: f64) -> f64 {
    (score * 10_000.0).round() / 10_000.0
}

fn trace_event_payload_text(
    bundle: &TraceBundle,
    kind: TraceEventKind,
    key: &str,
) -> Option<String> {
    bundle
        .events
        .iter()
        .find(|event| event.kind == kind)
        .and_then(|event| trace_value_text(event.payload.get(key)))
}

fn trace_last_event_payload_text(
    bundle: &TraceBundle,
    kind: TraceEventKind,
    key: &str,
) -> Option<String> {
    bundle
        .events
        .iter()
        .rev()
        .find(|event| event.kind == kind)
        .and_then(|event| trace_value_text(event.payload.get(key)))
}

fn trace_event_count(bundle: &TraceBundle, kind: TraceEventKind) -> usize {
    bundle
        .events
        .iter()
        .filter(|event| event.kind == kind)
        .count()
}

fn trace_first_cancellation_reason(bundle: &TraceBundle) -> Option<String> {
    bundle
        .events
        .iter()
        .find(|event| event.kind == TraceEventKind::Cancellation)
        .and_then(|event| event.payload.get("cancelReason"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

#[derive(Debug, Default)]
struct TraceInferenceSummary {
    count: usize,
    route_id: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    latency_ms: i64,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
    cost_cents: Option<f64>,
}

fn trace_inference_summary(bundle: &TraceBundle) -> TraceInferenceSummary {
    let mut summary = TraceInferenceSummary::default();
    for event in bundle
        .events
        .iter()
        .filter(|event| event.kind == TraceEventKind::Inference)
    {
        let payload = trace_inference_payload(&event.payload);
        summary.count += 1;
        if summary.route_id.is_none() {
            summary.route_id = trace_value_text(payload.get("routeId"));
        }
        if summary.provider.is_none() {
            summary.provider = trace_value_text(payload.get("provider"));
        }
        if summary.model.is_none() {
            summary.model = trace_value_text(payload.get("model"));
        }
        summary.latency_ms += trace_value_i64(payload.get("latencyMs")).unwrap_or_default();
        if let Some(usage) = payload.get("usage") {
            summary.prompt_tokens += trace_value_i64(
                usage
                    .get("promptTokens")
                    .or_else(|| usage.get("prompt_tokens")),
            )
            .unwrap_or_default();
            summary.completion_tokens += trace_value_i64(
                usage
                    .get("completionTokens")
                    .or_else(|| usage.get("completion_tokens")),
            )
            .unwrap_or_default();
            summary.total_tokens += trace_value_i64(
                usage
                    .get("totalTokens")
                    .or_else(|| usage.get("total_tokens")),
            )
            .unwrap_or_default();
        }
        if let Some(cost_cents) = trace_value_f64(payload.get("costCents")) {
            summary.cost_cents = Some(summary.cost_cents.unwrap_or_default() + cost_cents);
        }
    }
    summary
}

fn trace_inference_payload(payload: &Value) -> &Value {
    payload.get("item").unwrap_or(payload)
}

fn trace_value_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| {
                number
                    .as_u64()
                    .map(|value| value.min(i64::MAX as u64) as i64)
            })
            .or_else(|| number.as_f64().map(|value| value.round() as i64)),
        _ => None,
    }
}

fn trace_value_f64(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64().filter(|value| value.is_finite()),
        _ => None,
    }
}

fn trace_value_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_owned())
        }
        Value::Null => None,
        value => Some(value.to_string()),
    }
}

fn trace_answer_snippet(answer: &str, max_chars: usize) -> String {
    answer.trim().chars().take(max_chars).collect()
}

fn trace_bundle_citations(bundle: &TraceBundle) -> Vec<String> {
    let mut citations = Vec::new();
    for event in &bundle.events {
        collect_citations_from_value(&event.payload, &mut citations);
    }
    citations.sort();
    citations.dedup();
    citations
}

fn collect_citations_from_value(value: &Value, citations: &mut Vec<String>) {
    let Some(values) = value.get("citations").and_then(Value::as_array) else {
        return;
    };
    citations.extend(values.iter().filter_map(|value| {
        match value {
            Value::String(citation) => Some(citation.trim().to_owned()),
            Value::Object(object) => object
                .get("chunkId")
                .or_else(|| object.get("chunk_id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .map(ToOwned::to_owned),
            _ => None,
        }
    }));
    citations.retain(|citation| !citation.is_empty());
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
    use novex_trace::{TraceBundle, TraceEvent};
    use serde_json::json;

    #[test]
    fn module_describes_eval_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-eval");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }

    #[test]
    fn eval_runtime_scores_rag_case_with_answer_and_citation_match() {
        let expected = EvalCaseExpected {
            answer_contains: vec!["Monday".to_owned()],
            citations: vec!["handbook:0".to_owned()],
            intent: None,
            tool_code: None,
        };
        let actual = EvalCaseActual {
            answer: Some("Training starts on Monday.".to_owned()),
            citations: vec!["handbook:0".to_owned()],
            intent: None,
            tool_code: None,
            cost_cents: 0,
            latency_ms: 12,
        };

        let score = score_rag_case(&expected, &actual);

        assert!(score.passed);
        assert_eq!(score.metric, EvalMetricKind::CitationAccuracy);
        assert_eq!(score.score, 1.0);
    }

    #[test]
    fn eval_runtime_scores_intent_case_by_exact_match() {
        let expected = EvalCaseExpected {
            answer_contains: vec![],
            citations: vec![],
            intent: Some("rag_question".to_owned()),
            tool_code: None,
        };
        let actual = EvalCaseActual {
            answer: None,
            citations: vec![],
            intent: Some("tool_task".to_owned()),
            tool_code: None,
            cost_cents: 0,
            latency_ms: 3,
        };

        let score = score_intent_case(&expected, &actual);

        assert!(!score.passed);
        assert_eq!(score.metric, EvalMetricKind::IntentAccuracy);
        assert_eq!(score.score, 0.0);
    }

    #[test]
    fn eval_runtime_expected_payload_defaults_fields_for_intent_and_tool_cases() {
        let intent_expected = serde_json::from_value::<EvalCaseExpected>(json!({
            "intent": "rag_question"
        }))
        .unwrap();
        let tool_expected = serde_json::from_value::<EvalCaseExpected>(json!({
            "toolCode": "feishu.message.send"
        }))
        .unwrap();

        assert_eq!(intent_expected.answer_contains, Vec::<String>::new());
        assert_eq!(intent_expected.citations, Vec::<String>::new());
        assert_eq!(intent_expected.intent.as_deref(), Some("rag_question"));
        assert_eq!(
            tool_expected.tool_code.as_deref(),
            Some("feishu.message.send")
        );
    }

    #[test]
    fn eval_runtime_scores_tool_case_by_selected_tool() {
        let expected = EvalCaseExpected {
            answer_contains: vec![],
            citations: vec![],
            intent: None,
            tool_code: Some("feishu.message.send".to_owned()),
        };
        let actual = EvalCaseActual {
            answer: None,
            citations: vec![],
            intent: None,
            tool_code: Some("feishu.message.send".to_owned()),
            cost_cents: 0,
            latency_ms: 8,
        };

        let score = score_tool_case(&expected, &actual);

        assert!(score.passed);
        assert_eq!(score.metric, EvalMetricKind::ToolAccuracy);
        assert_eq!(score.score, 1.0);
    }

    #[test]
    fn trace_eval_candidate_extracts_tool_and_final_answer() {
        let candidate = EvalCaseCandidate::from_trace_bundle(&bundle_with_tool_and_final());

        assert_eq!(candidate.target_kind, EvalTargetKind::ReAct);
        assert_eq!(candidate.expected.tool_code.as_deref(), Some("rag.search"));
        assert!(candidate.prompt.contains("customer data"));
        assert!(candidate
            .expected
            .answer_contains
            .iter()
            .any(|snippet| snippet.contains("approved systems")));
    }

    #[test]
    fn trace_eval_candidate_tags_runtime_spans() {
        let bundle = TraceBundle::new("agent-1")
            .with_event(TraceEvent::retrieval(1, json!({"hitCount":2})))
            .with_event(TraceEvent::context_compaction(
                2,
                json!({"compactedItemCount":4}),
            ))
            .with_event(TraceEvent::cancellation(
                3,
                json!({"cancelReason":"external_cancel"}),
            ));

        let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

        assert_eq!(candidate.tags["retrievalCount"], 1);
        assert_eq!(candidate.tags["compactionCount"], 1);
        assert_eq!(candidate.tags["cancelled"], true);
        assert_eq!(candidate.tags["cancelReason"], "external_cancel");
    }

    #[test]
    fn trace_eval_candidate_tags_inference_spans() {
        let bundle = TraceBundle::new("agent-1")
            .with_event(TraceEvent::inference(
                1,
                json!({
                    "item": {
                        "type": "model_inference",
                        "routeId": "runtime.llm.code_agent",
                        "provider": "deep-seek",
                        "model": "deepseek-v4-flash",
                        "latencyMs": 42,
                        "usage": {
                            "promptTokens": 11,
                            "completionTokens": 7,
                            "totalTokens": 18
                        },
                        "costCents": 0.65
                    }
                }),
            ))
            .with_event(TraceEvent::inference(
                2,
                json!({
                    "item": {
                        "type": "model_inference",
                        "latencyMs": 8,
                        "usage": {
                            "promptTokens": 3,
                            "completionTokens": 2,
                            "totalTokens": 5
                        }
                    }
                }),
            ));

        let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

        assert_eq!(candidate.tags["inferenceCount"], 2);
        assert_eq!(candidate.tags["modelProvider"], "deep-seek");
        assert_eq!(candidate.tags["modelRouteId"], "runtime.llm.code_agent");
        assert_eq!(candidate.tags["modelName"], "deepseek-v4-flash");
        assert_eq!(candidate.tags["latencyMs"], 50);
        assert_eq!(candidate.tags["promptTokens"], 14);
        assert_eq!(candidate.tags["completionTokens"], 9);
        assert_eq!(candidate.tags["totalTokens"], 23);
        assert_eq!(candidate.tags["costCents"], 0.65);
    }

    #[test]
    fn eval_runtime_scores_latency_case_with_max_threshold() {
        let actual = EvalCaseActual {
            answer: None,
            citations: vec![],
            intent: None,
            tool_code: None,
            cost_cents: 0,
            latency_ms: 51,
        };

        let score = score_latency_case("latency-1", EvalTargetKind::Safety, &actual, 50);

        assert!(!score.passed);
        assert_eq!(score.metric, EvalMetricKind::Latency);
        assert_eq!(score.score, 0.0);
        assert_eq!(score.reason, "latency 51ms exceeded 50ms");
    }

    #[test]
    fn eval_runtime_scores_cost_case_with_max_threshold() {
        let actual = EvalCaseActual {
            answer: None,
            citations: vec![],
            intent: None,
            tool_code: None,
            cost_cents: 0,
            latency_ms: 12,
        };

        let score = score_cost_case("cost-1", EvalTargetKind::Safety, &actual, 0);

        assert!(score.passed);
        assert_eq!(score.metric, EvalMetricKind::Cost);
        assert_eq!(score.score, 1.0);
        assert_eq!(score.reason, "cost 0c within 0c");
    }

    #[test]
    fn eval_runtime_scores_retrieval_recall_by_expected_citations() {
        let expected = EvalCaseExpected {
            answer_contains: vec![],
            citations: vec!["handbook:0".to_owned(), "policy:2".to_owned()],
            intent: None,
            tool_code: None,
        };
        let actual = EvalCaseActual {
            answer: None,
            citations: vec!["handbook:0".to_owned()],
            intent: None,
            tool_code: None,
            cost_cents: 0,
            latency_ms: 8,
        };

        let score =
            score_retrieval_recall_case("recall-1", EvalTargetKind::Rag, &expected, &actual);

        assert!(!score.passed);
        assert_eq!(score.metric, EvalMetricKind::RetrievalRecall);
        assert_eq!(score.score, 0.0);
        assert_eq!(score.reason, "retrieval references missing");
    }

    #[test]
    fn eval_runtime_builds_regression_report_with_metric_breakdown() {
        let scores = vec![
            EvalCaseScore {
                case_id: "rag-1".to_owned(),
                target_kind: EvalTargetKind::Rag,
                metric: EvalMetricKind::CitationAccuracy,
                score: 1.0,
                passed: true,
                reason: "ok".to_owned(),
                cost_cents: 0,
                latency_ms: 10,
            },
            EvalCaseScore {
                case_id: "intent-1".to_owned(),
                target_kind: EvalTargetKind::Intent,
                metric: EvalMetricKind::IntentAccuracy,
                score: 0.0,
                passed: false,
                reason: "mismatch".to_owned(),
                cost_cents: 0,
                latency_ms: 5,
            },
        ];

        let report = build_regression_report(&scores);

        assert_eq!(report.total_cases, 2);
        assert_eq!(report.passed_cases, 1);
        assert_eq!(report.failed_cases, 1);
        assert_eq!(report.average_score, 0.5);
        assert_eq!(
            report
                .metric_breakdown
                .get(&EvalMetricKind::CitationAccuracy),
            Some(&1.0)
        );
        assert_eq!(
            report.metric_breakdown.get(&EvalMetricKind::IntentAccuracy),
            Some(&0.0)
        );
    }

    #[test]
    fn customer_service_eval_scores_citation_and_handoff_accuracy() {
        let grounded_score = score_customer_service_grounded_resolution_case(
            "cs-refund",
            &EvalCaseExpected {
                answer_contains: vec!["30 days".to_owned()],
                citations: vec!["cs-faq:refunds".to_owned()],
                intent: None,
                tool_code: None,
            },
            &EvalCaseActual {
                answer: Some("Refunds are available within 30 days.".to_owned()),
                citations: vec!["cs-faq:refunds".to_owned()],
                intent: None,
                tool_code: None,
                cost_cents: 0,
                latency_ms: 18,
            },
        );
        let handoff_score = score_customer_service_handoff_accuracy_case(
            "cs-handoff",
            &EvalCaseExpected {
                answer_contains: vec![],
                citations: vec![],
                intent: Some("human_handoff".to_owned()),
                tool_code: Some("handoff.request".to_owned()),
            },
            &EvalCaseActual {
                answer: Some("I will request a human handoff.".to_owned()),
                citations: vec![],
                intent: Some("human_handoff".to_owned()),
                tool_code: Some("handoff.request".to_owned()),
                cost_cents: 0,
                latency_ms: 22,
            },
        );

        assert!(grounded_score.passed);
        assert_eq!(grounded_score.target_kind, EvalTargetKind::CustomerService);
        assert_eq!(grounded_score.metric, EvalMetricKind::GroundedResolution);
        assert!(handoff_score.passed);
        assert_eq!(handoff_score.metric, EvalMetricKind::HandoffAccuracy);
    }

    #[test]
    fn customer_service_eval_report_flags_missing_evidence() {
        let score = score_customer_service_grounded_resolution_case(
            "cs-missing-citation",
            &EvalCaseExpected {
                answer_contains: vec!["refund".to_owned()],
                citations: vec!["cs-faq:refunds".to_owned()],
                intent: None,
                tool_code: None,
            },
            &EvalCaseActual {
                answer: Some("Refunds are available.".to_owned()),
                citations: vec![],
                intent: None,
                tool_code: None,
                cost_cents: 0,
                latency_ms: 19,
            },
        );
        let report = build_regression_report(&[score.clone()]);

        assert!(!score.passed);
        assert!(score.reason.contains("missing evidence"));
        assert_eq!(report.failed_cases, 1);
        assert_eq!(
            report
                .metric_breakdown
                .get(&EvalMetricKind::GroundedResolution),
            Some(&0.5)
        );
    }

    #[test]
    fn trace_eval_actual_extracts_tool_and_final_answer() {
        let actual = actual_from_trace_bundle(&bundle_with_tool_and_final());

        assert_eq!(actual.tool_code.as_deref(), Some("rag.search"));
        assert_eq!(
            actual.answer.as_deref(),
            Some("Customer data must stay in approved systems.")
        );
    }

    fn bundle_with_tool_and_final() -> TraceBundle {
        TraceBundle::new("agent-1")
            .with_event(TraceEvent::user_message(
                1,
                "How should we handle customer data?",
            ))
            .with_event(TraceEvent::tool_call(2, "call-1", "rag.search"))
            .with_event(TraceEvent::final_answer(
                3,
                "Customer data must stay in approved systems.",
            ))
    }
}
