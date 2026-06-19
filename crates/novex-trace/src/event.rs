use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceEventKind {
    UserMessage,
    AssistantMessage,
    Inference,
    Retrieval,
    ActionSelected,
    ToolCall,
    Observation,
    ContextCompaction,
    ApprovalRequested,
    FinalAnswer,
    Cancellation,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceEvent {
    pub sequence_no: i32,
    pub kind: TraceEventKind,
    pub payload: Value,
}

impl TraceEvent {
    pub fn user_message(sequence_no: i32, content: impl Into<String>) -> Self {
        Self {
            sequence_no,
            kind: TraceEventKind::UserMessage,
            payload: json!({ "content": content.into() }),
        }
    }

    pub fn assistant_message(sequence_no: i32, content: impl Into<String>) -> Self {
        Self {
            sequence_no,
            kind: TraceEventKind::AssistantMessage,
            payload: json!({ "content": content.into() }),
        }
    }

    pub fn inference(sequence_no: i32, payload: Value) -> Self {
        Self {
            sequence_no,
            kind: TraceEventKind::Inference,
            payload,
        }
    }

    pub fn retrieval(sequence_no: i32, payload: Value) -> Self {
        Self {
            sequence_no,
            kind: TraceEventKind::Retrieval,
            payload,
        }
    }

    pub fn action_selected(sequence_no: i32, payload: Value) -> Self {
        Self {
            sequence_no,
            kind: TraceEventKind::ActionSelected,
            payload,
        }
    }

    pub fn tool_call(
        sequence_no: i32,
        call_id: impl Into<String>,
        tool_code: impl Into<String>,
    ) -> Self {
        Self {
            sequence_no,
            kind: TraceEventKind::ToolCall,
            payload: json!({
                "callId": call_id.into(),
                "toolCode": tool_code.into(),
            }),
        }
    }

    pub fn observation(sequence_no: i32, call_id: impl Into<String>, output: Value) -> Self {
        Self {
            sequence_no,
            kind: TraceEventKind::Observation,
            payload: json!({
                "callId": call_id.into(),
                "output": output,
            }),
        }
    }

    pub fn context_compaction(sequence_no: i32, payload: Value) -> Self {
        Self {
            sequence_no,
            kind: TraceEventKind::ContextCompaction,
            payload,
        }
    }

    pub fn approval_requested(sequence_no: i32, tool_code: impl Into<String>) -> Self {
        Self {
            sequence_no,
            kind: TraceEventKind::ApprovalRequested,
            payload: json!({ "toolCode": tool_code.into() }),
        }
    }

    pub fn final_answer(sequence_no: i32, answer: impl Into<String>) -> Self {
        Self {
            sequence_no,
            kind: TraceEventKind::FinalAnswer,
            payload: json!({ "answer": answer.into() }),
        }
    }

    pub fn cancellation(sequence_no: i32, payload: Value) -> Self {
        Self {
            sequence_no,
            kind: TraceEventKind::Cancellation,
            payload,
        }
    }

    pub fn error(sequence_no: i32, message: impl Into<String>) -> Self {
        Self {
            sequence_no,
            kind: TraceEventKind::Error,
            payload: json!({ "message": message.into() }),
        }
    }
}
