use std::fs;
use std::path::Path;

use novex_approval_review::{
    build_guardian_model_review_prompt, guardian_review_failure_decision,
    parse_guardian_model_assessment, review_tool_approval,
    review_tool_approval_with_model_assessment, GuardianApprovalPolicy, GuardianModelReviewRequest,
    GuardianRejectionCircuitBreaker, GuardianReviewFailureReason, GuardianReviewInput,
    GuardianReviewOutcome, GuardianReviewedAction, GuardianRiskLevel, GuardianTranscriptEntry,
    GuardianTranscriptRole, GuardianUserAuthorization, GUARDIAN_REVIEWER_NAME,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn review_input() -> GuardianReviewInput {
    GuardianReviewInput {
        tool_code: "github.issue.write".to_owned(),
        risk_level: GuardianRiskLevel::Medium,
        approval_policy: GuardianApprovalPolicy::OnRisk,
        user_authorization: GuardianUserAuthorization::Missing,
        auto_approved: true,
        reviewer_enabled: false,
    }
}

#[test]
fn lib_rs_is_facade_for_approval_review_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["breaker", "model_review", "policy", "types"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum GuardianRiskLevel",
        "pub struct GuardianReviewInput",
        "pub fn review_tool_approval",
        "pub fn build_guardian_model_review_prompt",
        "pub fn parse_guardian_model_assessment",
        "pub struct GuardianRejectionCircuitBreaker",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn approval_review_domain_modules_exist() {
    for module in [
        "src/breaker.rs",
        "src/model_review.rs",
        "src/policy.rs",
        "src/types.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_policy_model_review_and_breaker_contracts() {
    let decision = review_tool_approval(review_input());
    assert_eq!(decision.outcome, GuardianReviewOutcome::Approved);

    let request = GuardianModelReviewRequest {
        transcript: vec![GuardianTranscriptEntry {
            role: GuardianTranscriptRole::User,
            content: "Please create the GitHub issue".to_owned(),
        }],
        reviewed_action: GuardianReviewedAction {
            tool_code: "github.issue.write".to_owned(),
            arguments: serde_json::json!({"title":"Bug"}),
            permission_code: Some("ai:agent:run".to_owned()),
        },
        retry_reason: None,
    };
    let prompt = build_guardian_model_review_prompt(&request).unwrap();
    assert!(prompt[0].content.contains("Novex Guardian"));

    let assessment = parse_guardian_model_assessment(
        r#"{"risk_level":"medium","user_authorization":"explicit","outcome":"approved","rationale":"User asked for this."}"#,
    )
    .unwrap();
    let model_decision = review_tool_approval_with_model_assessment(review_input(), assessment);
    assert_eq!(
        model_decision.reviewer_name.as_deref(),
        Some(GUARDIAN_REVIEWER_NAME)
    );

    let failure = guardian_review_failure_decision(
        review_input(),
        GuardianReviewFailureReason::Timeout,
        "review timed out",
    );
    assert!(!failure.can_execute);

    let mut breaker = GuardianRejectionCircuitBreaker::default();
    assert!(!breaker.record_denial());
    assert!(!breaker.record_denial());
    assert!(breaker.record_denial());
}
