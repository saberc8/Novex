mod breaker;
mod model_review;
mod policy;
mod types;

use novex_ai_core::FoundationModule;

pub use breaker::{
    GuardianRejectionCircuitBreaker, AUTO_REVIEW_DENIAL_WINDOW_SIZE,
    MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN, MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN,
};
pub use model_review::{build_guardian_model_review_prompt, parse_guardian_model_assessment};
pub use policy::{
    guardian_review_failure_decision, review_tool_approval,
    review_tool_approval_with_model_assessment,
};
pub use types::{
    GuardianApprovalPolicy, GuardianDecisionSource, GuardianModelAssessment,
    GuardianModelReviewParseError, GuardianModelReviewRequest, GuardianPromptMessage,
    GuardianReviewDecision, GuardianReviewFailureReason, GuardianReviewInput,
    GuardianReviewOutcome, GuardianReviewStatus, GuardianReviewedAction, GuardianRiskLevel,
    GuardianTranscriptEntry, GuardianTranscriptRole, GuardianUserAuthorization,
    GUARDIAN_REVIEWER_NAME,
};

pub const CRATE_ID: &str = "novex-approval-review";

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Approval Review",
        "ai-foundation",
        "Guardian approval review contracts, fail-closed policy decisions, and denial breakers.",
    )
}
