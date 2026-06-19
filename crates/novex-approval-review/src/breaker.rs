use std::collections::VecDeque;

pub const MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN: usize = 3;
pub const MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN: usize = 10;
pub const AUTO_REVIEW_DENIAL_WINDOW_SIZE: usize = 50;

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
