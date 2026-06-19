use std::fs;
use std::path::Path;

use novex_eval::{
    actual_from_trace_bundle, build_regression_report, score_case, score_cost_case,
    score_latency_case, EvalCaseActual, EvalCaseCandidate, EvalCaseExpected, EvalCaseInput,
    EvalMetricKind, EvalTargetKind, TraceEvalPolicy,
};
use novex_trace::{TraceBundle, TraceEvent};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_eval_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["case", "report", "score", "text", "trace_extract"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum EvalTargetKind",
        "pub struct EvalCaseCandidate",
        "pub fn actual_from_trace_bundle",
        "pub struct EvalCaseScore",
        "pub fn score_case",
        "pub struct RegressionReport",
        "pub fn build_regression_report",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn eval_domain_modules_exist() {
    for module in [
        "src/case.rs",
        "src/report.rs",
        "src/score.rs",
        "src/text.rs",
        "src/trace_extract.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_case_trace_and_score_contracts() {
    let input = EvalCaseInput {
        target_kind: EvalTargetKind::Rag,
        prompt: "When does training start?".to_owned(),
    };
    assert_eq!(input.target_kind, EvalTargetKind::Rag);
    assert_eq!(TraceEvalPolicy::default().answer_snippet_max_chars, 120);

    let bundle = TraceBundle::new("trace-1")
        .with_event(TraceEvent::user_message(1, "Find policy"))
        .with_event(TraceEvent::tool_call(2, "call-1", "rag.search"))
        .with_event(TraceEvent::final_answer(3, "Training starts Monday."));
    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);
    assert_eq!(candidate.expected.tool_code.as_deref(), Some("rag.search"));
    assert_eq!(
        actual_from_trace_bundle(&bundle).answer.as_deref(),
        Some("Training starts Monday.")
    );

    let expected = EvalCaseExpected {
        answer_contains: vec!["Monday".to_owned()],
        citations: vec!["handbook:0".to_owned()],
        intent: None,
        tool_code: None,
    };
    let actual = EvalCaseActual {
        answer: Some("Training starts Monday.".to_owned()),
        citations: vec!["handbook:0".to_owned()],
        intent: None,
        tool_code: None,
        cost_cents: 2,
        latency_ms: 10,
    };
    let score = score_case("case-1", EvalTargetKind::Rag, &expected, &actual);
    assert!(score.passed);
    assert_eq!(score.metric, EvalMetricKind::CitationAccuracy);

    let latency = score_latency_case("latency", EvalTargetKind::Rag, &actual, 20);
    assert!(latency.passed);
    let cost = score_cost_case("cost", EvalTargetKind::Rag, &actual, 2);
    assert!(cost.passed);

    let report = build_regression_report(&[score, latency, cost]);
    assert_eq!(report.total_cases, 3);
    assert_eq!(report.passed_cases, 3);
}
