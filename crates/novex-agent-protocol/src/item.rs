use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTurnItemType {
    UserMessage,
    AssistantMessage,
    Reasoning,
    ToolCall,
    ToolObservation,
    FinalAnswer,
    ContextCompaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolObservationStatus {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentTurnItem {
    UserMessage {
        content: String,
    },
    AssistantMessage {
        content: String,
    },
    Reasoning {
        summary: String,
    },
    ToolCall {
        #[serde(rename = "callId")]
        call_id: String,
        #[serde(rename = "toolCode")]
        tool_code: String,
        arguments: Value,
    },
    ToolObservation {
        #[serde(rename = "callId")]
        call_id: String,
        status: ToolObservationStatus,
        output: Value,
    },
    FinalAnswer {
        content: String,
    },
    ContextCompaction {
        summary: String,
    },
}

impl AgentTurnItem {
    pub fn user_message(content: impl Into<String>) -> Self {
        Self::UserMessage {
            content: content.into(),
        }
    }

    pub fn assistant_message(content: impl Into<String>) -> Self {
        Self::AssistantMessage {
            content: content.into(),
        }
    }

    pub fn tool_call(
        call_id: impl Into<String>,
        tool_code: impl Into<String>,
        arguments: Value,
    ) -> Self {
        Self::ToolCall {
            call_id: call_id.into(),
            tool_code: tool_code.into(),
            arguments,
        }
    }

    pub fn tool_observation(
        call_id: impl Into<String>,
        status: ToolObservationStatus,
        output: Value,
    ) -> Self {
        Self::ToolObservation {
            call_id: call_id.into(),
            status,
            output,
        }
    }

    pub fn call_id(&self) -> Option<&str> {
        match self {
            Self::ToolCall { call_id, .. } | Self::ToolObservation { call_id, .. } => Some(call_id),
            _ => None,
        }
    }

    pub fn requires_follow_up(&self) -> bool {
        matches!(
            self,
            Self::ToolObservation { .. } | Self::ContextCompaction { .. }
        )
    }
}
