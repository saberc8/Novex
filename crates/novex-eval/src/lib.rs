use std::collections::BTreeMap;

use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-eval";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalTargetKind {
    Rag,
    Intent,
    Tool,
    ReAct,
    Safety,
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
}
