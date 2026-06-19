use crate::types::{
    ApprovalPolicy, ToolExecutionPolicyDecision, ToolExecutionPolicyInput, ToolRiskLevel,
};

pub fn tool_risk_code(risk: ToolRiskLevel) -> &'static str {
    match risk {
        ToolRiskLevel::Low => "low",
        ToolRiskLevel::Medium => "medium",
        ToolRiskLevel::High => "high",
    }
}

pub fn approval_policy_code(policy: ApprovalPolicy) -> &'static str {
    match policy {
        ApprovalPolicy::Never => "never",
        ApprovalPolicy::OnRisk => "on_risk",
        ApprovalPolicy::Always => "always",
    }
}

pub fn evaluate_tool_execution_policy(
    input: ToolExecutionPolicyInput,
) -> ToolExecutionPolicyDecision {
    let requires_approval = match input.approval_policy {
        ApprovalPolicy::Always => true,
        ApprovalPolicy::Never => false,
        ApprovalPolicy::OnRisk => {
            matches!(input.risk_level, ToolRiskLevel::High)
                || (matches!(input.risk_level, ToolRiskLevel::Medium) && !input.auto_approved)
        }
    };
    let policy_reason = if matches!(input.risk_level, ToolRiskLevel::High) && requires_approval {
        "high_risk_requires_manual_approval"
    } else if matches!(input.approval_policy, ApprovalPolicy::Always) {
        "approval_policy_always"
    } else if requires_approval {
        "risk_requires_approval"
    } else if input.auto_approved {
        "auto_approved"
    } else {
        "low_risk_allowed"
    }
    .to_owned();

    ToolExecutionPolicyDecision {
        tool_code: input.tool_code,
        risk_level: input.risk_level,
        permission_code: input.permission_code,
        requires_approval,
        can_execute: !requires_approval,
        pause_reason: requires_approval.then(|| "approval".to_owned()),
        policy_reason,
    }
}
