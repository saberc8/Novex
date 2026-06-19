use novex_eval::*;

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
