use std::collections::VecDeque;

use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-approval-review";
pub const GUARDIAN_REVIEWER_NAME: &str = "guardian";
pub const MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN: usize = 3;
pub const MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN: usize = 10;
pub const AUTO_REVIEW_DENIAL_WINDOW_SIZE: usize = 50;

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
}

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
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardianRejectionCircuitBreaker {
    consecutive_denials: usize,
    recent_outcomes: VecDeque<bool>,
}

impl Default for GuardianRejectionCircuitBreaker {
    fn default() -> Self {
        Self {
            consecutive_denials: 0,
            recent_outcomes: VecDeque::with_capacity(AUTO_REVIEW_DENIAL_WINDOW_SIZE),
        }
    }
}

impl GuardianRejectionCircuitBreaker {
    pub fn record_denial(&mut self) -> bool {
        self.consecutive_denials += 1;
        self.push_recent_outcome(true);
        self.should_interrupt()
    }

    pub fn record_non_denial(&mut self) -> bool {
        self.consecutive_denials = 0;
        self.push_recent_outcome(false);
        self.should_interrupt()
    }

    pub fn should_interrupt(&self) -> bool {
        self.consecutive_denials >= MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN
            || self.recent_denial_count() >= MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN
    }

    pub fn consecutive_denial_count(&self) -> usize {
        self.consecutive_denials
    }

    pub fn recent_denial_count(&self) -> usize {
        self.recent_outcomes
            .iter()
            .filter(|outcome| **outcome)
            .count()
    }

    fn push_recent_outcome(&mut self, denied: bool) {
        if self.recent_outcomes.len() == AUTO_REVIEW_DENIAL_WINDOW_SIZE {
            self.recent_outcomes.pop_front();
        }
        self.recent_outcomes.push_back(denied);
    }
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Approval Review",
        "ai-foundation",
        "Guardian approval review contracts, fail-closed policy decisions, and denial breakers.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn guardian_denial_breaker_interrupts_after_three_consecutive_denials() {
        let mut breaker = GuardianRejectionCircuitBreaker::default();

        assert!(!breaker.record_denial());
        assert!(!breaker.record_denial());
        assert!(breaker.record_denial());
    }

    #[test]
    fn guardian_denial_breaker_counts_recent_denials_in_window() {
        let mut breaker = GuardianRejectionCircuitBreaker::default();

        for _ in 0..9 {
            assert!(!breaker.record_denial());
            breaker.record_non_denial();
        }

        assert!(breaker.record_denial());
    }

    #[test]
    fn guardian_denial_breaker_non_denial_resets_consecutive_denials() {
        let mut breaker = GuardianRejectionCircuitBreaker::default();

        assert!(!breaker.record_denial());
        assert!(!breaker.record_denial());
        breaker.record_non_denial();

        assert!(!breaker.record_denial());
        assert!(!breaker.record_denial());
    }
}
