use novex_approval_review::*;

#[test]
fn guardian_model_review_prompt_includes_transcript_action_and_schema() {
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
        retry_reason: Some("previous parse failure".to_owned()),
    };

    let messages = build_guardian_model_review_prompt(&request).unwrap();

    assert_eq!(messages[0].role, "system");
    assert!(messages[0].content.contains("Novex Guardian"));
    assert!(messages[0].content.contains("risk_level"));
    assert!(messages[1].content.contains(">>> TRANSCRIPT START"));
    assert!(messages[1]
        .content
        .contains("Please create the GitHub issue"));
    assert!(messages[1].content.contains("previous parse failure"));
    assert!(messages[1]
        .content
        .contains("\"toolCode\": \"github.issue.write\""));
    assert!(messages[1].content.contains(">>> APPROVAL REQUEST END"));
}

#[test]
fn guardian_model_review_parser_accepts_plain_and_fenced_json() {
    let plain = parse_guardian_model_assessment(
        r#"{"risk_level":"medium","user_authorization":"explicit","outcome":"approved","rationale":"User requested the issue."}"#,
    )
    .unwrap();
    let fenced = parse_guardian_model_assessment(
        "```json\n{\"risk_level\":\"high\",\"user_authorization\":\"missing\",\"outcome\":\"rejected\",\"rationale\":\"No user authorization.\"}\n```",
    )
    .unwrap();

    assert_eq!(plain.outcome, GuardianReviewOutcome::Approved);
    assert_eq!(
        plain.user_authorization,
        GuardianUserAuthorization::Explicit
    );
    assert_eq!(fenced.outcome, GuardianReviewOutcome::Rejected);
    assert_eq!(fenced.risk_level, GuardianRiskLevel::High);
}

#[test]
fn guardian_model_review_parser_rejects_missing_rationale() {
    let err = parse_guardian_model_assessment(
        r#"{"risk_level":"medium","user_authorization":"explicit","outcome":"approved","rationale":""}"#,
    )
    .unwrap_err();

    assert_eq!(err.kind, GuardianReviewFailureReason::Parse);
}
