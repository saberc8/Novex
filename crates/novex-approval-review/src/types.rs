use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const GUARDIAN_REVIEWER_NAME: &str = "guardian";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianRiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianApprovalPolicy {
    Never,
    OnRisk,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianUserAuthorization {
    Explicit,
    Implicit,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianReviewOutcome {
    Approved,
    NeedsHuman,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianDecisionSource {
    Policy,
    Guardian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianReviewStatus {
    PolicyOnly,
    Reviewed,
    FailedClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianReviewFailureReason {
    Timeout,
    Session,
    Parse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianReviewInput {
    pub tool_code: String,
    pub risk_level: GuardianRiskLevel,
    pub approval_policy: GuardianApprovalPolicy,
    pub user_authorization: GuardianUserAuthorization,
    pub auto_approved: bool,
    pub reviewer_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianReviewDecision {
    pub tool_code: String,
    pub risk_level: GuardianRiskLevel,
    pub approval_policy: GuardianApprovalPolicy,
    pub user_authorization: GuardianUserAuthorization,
    pub outcome: GuardianReviewOutcome,
    pub source: GuardianDecisionSource,
    pub requires_human_approval: bool,
    pub can_execute: bool,
    pub rationale: String,
    pub reviewer_name: Option<String>,
    pub review_status: GuardianReviewStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<GuardianReviewFailureReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_route_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_latency_ms: Option<u128>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardianTranscriptRole {
    Developer,
    User,
    Assistant,
    Tool,
}

impl GuardianTranscriptRole {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Developer => "developer",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianTranscriptEntry {
    pub role: GuardianTranscriptRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianReviewedAction {
    #[serde(rename = "toolCode")]
    pub tool_code: String,
    pub arguments: Value,
    pub permission_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianModelReviewRequest {
    pub transcript: Vec<GuardianTranscriptEntry>,
    pub reviewed_action: GuardianReviewedAction,
    pub retry_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianPromptMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianModelAssessment {
    pub risk_level: GuardianRiskLevel,
    pub user_authorization: GuardianUserAuthorization,
    pub outcome: GuardianReviewOutcome,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardianModelReviewParseError {
    pub kind: GuardianReviewFailureReason,
    pub message: String,
}
