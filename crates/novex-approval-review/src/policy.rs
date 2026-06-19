use crate::types::{
    GuardianApprovalPolicy, GuardianDecisionSource, GuardianModelAssessment,
    GuardianReviewDecision, GuardianReviewFailureReason, GuardianReviewInput,
    GuardianReviewOutcome, GuardianReviewStatus, GuardianRiskLevel, GuardianUserAuthorization,
    GUARDIAN_REVIEWER_NAME,
};

pub fn review_tool_approval(input: GuardianReviewInput) -> GuardianReviewDecision {
    let (outcome, rationale) = if matches!(input.approval_policy, GuardianApprovalPolicy::Always) {
        (
            GuardianReviewOutcome::NeedsHuman,
            "approval_policy_always_requires_human",
        )
    } else if matches!(input.risk_level, GuardianRiskLevel::High) {
        (
            GuardianReviewOutcome::NeedsHuman,
            "high_risk_requires_human_approval",
        )
    } else if matches!(input.risk_level, GuardianRiskLevel::Medium)
        && matches!(input.approval_policy, GuardianApprovalPolicy::OnRisk)
        && !input.auto_approved
        && !matches!(
            input.user_authorization,
            GuardianUserAuthorization::Explicit
        )
    {
        (
            GuardianReviewOutcome::NeedsHuman,
            "risk_requires_human_approval",
        )
    } else if input.auto_approved {
        (GuardianReviewOutcome::Approved, "auto_approved_by_policy")
    } else if matches!(
        input.user_authorization,
        GuardianUserAuthorization::Explicit
    ) {
        (
            GuardianReviewOutcome::Approved,
            "explicit_user_authorization",
        )
    } else {
        (GuardianReviewOutcome::Approved, "low_risk_allowed")
    };
    let requires_human_approval = matches!(outcome, GuardianReviewOutcome::NeedsHuman);
    let can_execute = matches!(outcome, GuardianReviewOutcome::Approved);
    let source = if input.reviewer_enabled {
        GuardianDecisionSource::Guardian
    } else {
        GuardianDecisionSource::Policy
    };

    GuardianReviewDecision {
        tool_code: input.tool_code,
        risk_level: input.risk_level,
        approval_policy: input.approval_policy,
        user_authorization: input.user_authorization,
        outcome,
        source,
        requires_human_approval,
        can_execute,
        rationale: rationale.to_owned(),
        reviewer_name: input
            .reviewer_enabled
            .then(|| GUARDIAN_REVIEWER_NAME.to_owned()),
        review_status: GuardianReviewStatus::PolicyOnly,
        failure_reason: None,
        model_route_id: None,
        model_provider: None,
        model_name: None,
        review_latency_ms: None,
    }
}

pub fn review_tool_approval_with_model_assessment(
    input: GuardianReviewInput,
    assessment: GuardianModelAssessment,
) -> GuardianReviewDecision {
    let mut outcome = assessment.outcome;
    let mut rationale = assessment.rationale;
    if matches!(input.approval_policy, GuardianApprovalPolicy::Always)
        && matches!(outcome, GuardianReviewOutcome::Approved)
    {
        outcome = GuardianReviewOutcome::NeedsHuman;
        rationale = "approval_policy_always_requires_human".to_owned();
    }
    let requires_human_approval = matches!(outcome, GuardianReviewOutcome::NeedsHuman);
    let can_execute = matches!(outcome, GuardianReviewOutcome::Approved);

    GuardianReviewDecision {
        tool_code: input.tool_code,
        risk_level: assessment.risk_level,
        approval_policy: input.approval_policy,
        user_authorization: assessment.user_authorization,
        outcome,
        source: GuardianDecisionSource::Guardian,
        requires_human_approval,
        can_execute,
        rationale,
        reviewer_name: Some(GUARDIAN_REVIEWER_NAME.to_owned()),
        review_status: GuardianReviewStatus::Reviewed,
        failure_reason: None,
        model_route_id: None,
        model_provider: None,
        model_name: None,
        review_latency_ms: None,
    }
}

pub fn guardian_review_failure_decision(
    input: GuardianReviewInput,
    failure_reason: GuardianReviewFailureReason,
    message: impl Into<String>,
) -> GuardianReviewDecision {
    GuardianReviewDecision {
        tool_code: input.tool_code,
        risk_level: input.risk_level,
        approval_policy: input.approval_policy,
        user_authorization: input.user_authorization,
        outcome: GuardianReviewOutcome::NeedsHuman,
        source: GuardianDecisionSource::Guardian,
        requires_human_approval: true,
        can_execute: false,
        rationale: message.into(),
        reviewer_name: Some(GUARDIAN_REVIEWER_NAME.to_owned()),
        review_status: GuardianReviewStatus::FailedClosed,
        failure_reason: Some(failure_reason),
        model_route_id: None,
        model_provider: None,
        model_name: None,
        review_latency_ms: None,
    }
}
