use novex_eval::*;

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

    let score = score_retrieval_recall_case("recall-1", EvalTargetKind::Rag, &expected, &actual);

    assert!(!score.passed);
    assert_eq!(score.metric, EvalMetricKind::RetrievalRecall);
    assert_eq!(score.score, 0.0);
    assert_eq!(score.reason, "retrieval references missing");
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
