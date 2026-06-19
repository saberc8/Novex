use novex_approval_review::*;

fn review_input(
    risk_level: GuardianRiskLevel,
    approval_policy: GuardianApprovalPolicy,
    auto_approved: bool,
) -> GuardianReviewInput {
    GuardianReviewInput {
        tool_code: "github.issue.write".to_owned(),
        risk_level,
        approval_policy,
        user_authorization: GuardianUserAuthorization::Missing,
        auto_approved,
        reviewer_enabled: false,
    }
}

#[test]
fn guardian_review_high_risk_requires_human_even_when_auto_approved() {
    let decision = review_tool_approval(review_input(
        GuardianRiskLevel::High,
        GuardianApprovalPolicy::OnRisk,
        true,
    ));

    assert_eq!(decision.outcome, GuardianReviewOutcome::NeedsHuman);
    assert!(decision.requires_human_approval);
    assert!(!decision.can_execute);
    assert_eq!(decision.rationale, "high_risk_requires_human_approval");
}

#[test]
fn guardian_review_medium_risk_auto_approved_can_execute() {
    let decision = review_tool_approval(review_input(
        GuardianRiskLevel::Medium,
        GuardianApprovalPolicy::OnRisk,
        true,
    ));

    assert_eq!(decision.outcome, GuardianReviewOutcome::Approved);
    assert!(!decision.requires_human_approval);
    assert!(decision.can_execute);
    assert_eq!(decision.rationale, "auto_approved_by_policy");
}

#[test]
fn guardian_review_approval_policy_always_requires_human() {
    let decision = review_tool_approval(review_input(
        GuardianRiskLevel::Low,
        GuardianApprovalPolicy::Always,
        true,
    ));

    assert_eq!(decision.outcome, GuardianReviewOutcome::NeedsHuman);
    assert!(decision.requires_human_approval);
    assert_eq!(decision.rationale, "approval_policy_always_requires_human");
}

#[test]
fn guardian_model_review_assessment_maps_to_guardian_decision() {
    let assessment = GuardianModelAssessment {
        risk_level: GuardianRiskLevel::Medium,
        user_authorization: GuardianUserAuthorization::Explicit,
        outcome: GuardianReviewOutcome::Approved,
        rationale: "User explicitly asked for the action.".to_owned(),
    };

    let decision = review_tool_approval_with_model_assessment(
        review_input(
            GuardianRiskLevel::Medium,
            GuardianApprovalPolicy::OnRisk,
            true,
        ),
        assessment,
    );

    assert_eq!(decision.source, GuardianDecisionSource::Guardian);
    assert_eq!(decision.review_status, GuardianReviewStatus::Reviewed);
    assert_eq!(decision.outcome, GuardianReviewOutcome::Approved);
    assert!(decision.can_execute);
    assert_eq!(
        decision.reviewer_name.as_deref(),
        Some(GUARDIAN_REVIEWER_NAME)
    );
}

#[test]
fn guardian_model_review_failure_decision_fails_closed() {
    let decision = guardian_review_failure_decision(
        review_input(
            GuardianRiskLevel::Medium,
            GuardianApprovalPolicy::OnRisk,
            true,
        ),
        GuardianReviewFailureReason::Timeout,
        "review timed out",
    );

    assert_eq!(decision.source, GuardianDecisionSource::Guardian);
    assert_eq!(decision.review_status, GuardianReviewStatus::FailedClosed);
    assert_eq!(
        decision.failure_reason,
        Some(GuardianReviewFailureReason::Timeout)
    );
    assert_eq!(decision.outcome, GuardianReviewOutcome::NeedsHuman);
    assert!(decision.requires_human_approval);
    assert!(!decision.can_execute);
}
