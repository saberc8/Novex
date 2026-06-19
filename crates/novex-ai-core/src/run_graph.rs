use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    WaitingApproval,
    Paused,
    Resuming,
    Cancelling,
    Cancelled,
    Failed,
    Succeeded,
}

impl RunStatus {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Cancelled | Self::Failed | Self::Succeeded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunTransitionError {
    pub from: RunStatus,
    pub to: RunStatus,
}

pub fn can_transition_run_status(from: RunStatus, to: RunStatus) -> bool {
    use RunStatus::*;

    matches!(
        (from, to),
        (Queued, Running)
            | (Queued, Cancelling)
            | (Queued, Cancelled)
            | (Running, WaitingApproval)
            | (Running, Paused)
            | (Running, Cancelling)
            | (Running, Failed)
            | (Running, Succeeded)
            | (WaitingApproval, Resuming)
            | (WaitingApproval, Cancelling)
            | (WaitingApproval, Failed)
            | (Paused, Resuming)
            | (Paused, Cancelling)
            | (Paused, Failed)
            | (Resuming, Running)
            | (Resuming, Cancelling)
            | (Resuming, Failed)
            | (Cancelling, Cancelled)
    )
}

pub fn validate_run_transition(from: RunStatus, to: RunStatus) -> Result<(), RunTransitionError> {
    if can_transition_run_status(from, to) {
        Ok(())
    } else {
        Err(RunTransitionError { from, to })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStepType {
    ModelCall,
    Retrieval,
    Rerank,
    ToolCall,
    Approval,
    HumanInput,
    ConnectorSync,
    MediaJob,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PauseReason {
    Approval,
    HumanInput,
    ExternalCallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunEventKind {
    InputReceived,
    StatusChanged,
    IntentRouted,
    Thought,
    Retrieval,
    ActionSelected,
    ApprovalRequested,
    Paused,
    Resumed,
    ToolCalled,
    Observation,
    FinalOutput,
    CancelRequested,
    Cancelled,
    Error,
}
