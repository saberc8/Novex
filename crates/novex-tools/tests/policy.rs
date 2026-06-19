use novex_tools::*;

#[test]
fn tool_execution_policy_evaluates_risk_permission_and_auto_approval() {
    let low = evaluate_tool_execution_policy(ToolExecutionPolicyInput {
        tool_code: "github.repo.read".to_owned(),
        risk_level: ToolRiskLevel::Low,
        approval_policy: ApprovalPolicy::OnRisk,
        permission_code: Some("ai:tool:dryRun".to_owned()),
        auto_approved: false,
    });
    assert!(!low.requires_approval);
    assert!(low.can_execute);

    let medium = evaluate_tool_execution_policy(ToolExecutionPolicyInput {
        tool_code: "media.image.generate".to_owned(),
        risk_level: ToolRiskLevel::Medium,
        approval_policy: ApprovalPolicy::OnRisk,
        permission_code: Some("ai:agent:run".to_owned()),
        auto_approved: false,
    });
    assert!(medium.requires_approval);
    assert_eq!(medium.pause_reason.as_deref(), Some("approval"));

    let high = evaluate_tool_execution_policy(ToolExecutionPolicyInput {
        tool_code: "feishu.message.send".to_owned(),
        risk_level: ToolRiskLevel::High,
        approval_policy: ApprovalPolicy::OnRisk,
        permission_code: Some("ai:agent:run".to_owned()),
        auto_approved: true,
    });
    assert!(high.requires_approval);
    assert_eq!(high.policy_reason, "high_risk_requires_manual_approval");
}
