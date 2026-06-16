use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CRATE_ID: &str = "novex-agent-protocol";

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn turn_item_serializes_with_snake_case_type_tags() {
        let item = AgentTurnItem::tool_call("call-1", "rag.search", json!({"query":"policy"}));
        let value = serde_json::to_value(item).unwrap();

        assert_eq!(value["type"], "tool_call");
        assert_eq!(value["callId"], "call-1");
        assert_eq!(value["toolCode"], "rag.search");
    }

    #[test]
    fn tool_observation_links_to_call_id() {
        let item = AgentTurnItem::tool_observation(
            "call-1",
            ToolObservationStatus::Succeeded,
            json!({"hits": 2}),
        );

        assert_eq!(item.call_id(), Some("call-1"));
        assert!(item.requires_follow_up());
    }

    #[test]
    fn turn_outcome_identifies_terminal_states() {
        assert!(TurnOutcome::Final.is_terminal());
        assert!(TurnOutcome::Paused.is_terminal());
        assert!(!TurnOutcome::NeedsFollowUp.is_terminal());
    }
}
