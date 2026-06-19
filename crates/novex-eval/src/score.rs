use crate::case::{EvalCaseActual, EvalCaseExpected, EvalMetricKind, EvalTargetKind};
use crate::text::contains_case_insensitive;
use serde::{Deserialize, Serialize};

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
