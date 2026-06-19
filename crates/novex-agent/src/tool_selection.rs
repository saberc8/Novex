use novex_tools::{
    evaluate_tool_execution_policy, ApprovalPolicy, ToolExecutionPolicyDecision,
    ToolExecutionPolicyInput, ToolRiskLevel,
};
use serde::{Deserialize, Serialize};

use crate::text::contains_any;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedTool {
    pub code: String,
    pub risk_level: u8,
    pub requires_approval: bool,
    pub policy_decision: ToolExecutionPolicyDecision,
}

pub fn select_tool(input: &str) -> Option<SelectedTool> {
    let normalized = input.to_lowercase();
    if contains_any(
        &normalized,
        &[
            "read github",
            "github file",
            "read repo file",
            "read file",
            "读取 github",
            "读取仓库文件",
        ],
    ) {
        return Some(selected_tool(
            "github.repo.read",
            ToolRiskLevel::Low,
            ApprovalPolicy::OnRisk,
            "ai:tool:dryRun",
        ));
    }
    if contains_any(
        &normalized,
        &[
            "github",
            "repo search",
            "search repo",
            "repository search",
            "代码仓库",
            "仓库搜索",
        ],
    ) {
        return Some(selected_tool(
            "github.repo.search",
            ToolRiskLevel::Low,
            ApprovalPolicy::OnRisk,
            "ai:tool:dryRun",
        ));
    }
    if contains_any(
        &normalized,
        &[
            "feishu",
            "飞书",
            "message",
            "notify",
            "notification",
            "reminder",
            "发送",
            "通知",
        ],
    ) {
        return Some(selected_tool(
            "feishu.message.send",
            ToolRiskLevel::Medium,
            ApprovalPolicy::OnRisk,
            "ai:agent:run",
        ));
    }
    if contains_any(
        &normalized,
        &[
            "image",
            "picture",
            "poster",
            "media",
            "generate image",
            "图片",
            "海报",
        ],
    ) {
        return Some(selected_tool(
            "media.image.generate",
            ToolRiskLevel::Medium,
            ApprovalPolicy::OnRisk,
            "ai:tool:dryRun",
        ));
    }
    if contains_any(
        &normalized,
        &[
            "search",
            "find",
            "knowledge",
            "handbook",
            "rag",
            "检索",
            "搜索",
            "知识库",
        ],
    ) {
        return Some(selected_tool(
            "rag.search",
            ToolRiskLevel::Low,
            ApprovalPolicy::OnRisk,
            "ai:knowledge:ask",
        ));
    }
    None
}

fn selected_tool(
    code: &str,
    risk_level: ToolRiskLevel,
    approval_policy: ApprovalPolicy,
    permission_code: &str,
) -> SelectedTool {
    let decision = evaluate_tool_execution_policy(ToolExecutionPolicyInput {
        tool_code: code.to_owned(),
        risk_level,
        approval_policy,
        permission_code: Some(permission_code.to_owned()),
        auto_approved: false,
    });
    SelectedTool {
        code: code.to_owned(),
        risk_level: risk_level_value(risk_level),
        requires_approval: decision.requires_approval,
        policy_decision: decision,
    }
}

fn risk_level_value(risk_level: ToolRiskLevel) -> u8 {
    match risk_level {
        ToolRiskLevel::Low => 1,
        ToolRiskLevel::Medium => 2,
        ToolRiskLevel::High => 3,
    }
}
