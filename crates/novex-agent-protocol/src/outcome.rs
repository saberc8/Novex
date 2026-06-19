use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnOutcome {
    NeedsFollowUp,
    Final,
    Paused,
    Cancelled,
    Failed,
    BudgetExceeded,
}

impl TurnOutcome {
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::NeedsFollowUp)
    }
}
