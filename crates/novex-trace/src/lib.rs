use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const CRATE_ID: &str = "novex-trace";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceEventKind {
    UserMessage,
    AssistantMessage,
    ToolCall,
    Observation,
    ApprovalRequested,
    FinalAnswer,
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

    pub fn error(sequence_no: i32, message: impl Into<String>) -> Self {
        Self {
            sequence_no,
            kind: TraceEventKind::Error,
            payload: json!({ "message": message.into() }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceBundle {
    pub trace_id: String,
    pub events: Vec<TraceEvent>,
}

impl TraceBundle {
    pub fn new(trace_id: impl Into<String>) -> Self {
        Self {
            trace_id: trace_id.into(),
            events: Vec::new(),
        }
    }

    pub fn with_event(mut self, event: TraceEvent) -> Self {
        self.events.push(event);
        self.events.sort_by_key(|event| event.sequence_no);
        self
    }

    pub fn tool_call_count(&self) -> usize {
        self.events
            .iter()
            .filter(|event| matches!(event.kind, TraceEventKind::ToolCall))
            .count()
    }

    pub fn replay_summary(&self) -> TraceReplaySummary {
        let has_error = self
            .events
            .iter()
            .any(|event| matches!(event.kind, TraceEventKind::Error));
        let has_final_answer = self
            .events
            .iter()
            .any(|event| matches!(event.kind, TraceEventKind::FinalAnswer));
        let has_approval_pause = self
            .events
            .iter()
            .any(|event| matches!(event.kind, TraceEventKind::ApprovalRequested));
        let final_status = if has_error {
            "failed"
        } else if has_approval_pause && !has_final_answer {
            "waiting_approval"
        } else if has_final_answer {
            "succeeded"
        } else {
            "running"
        };

        TraceReplaySummary {
            trace_id: self.trace_id.clone(),
            total_events: self.events.len(),
            tool_call_count: self.tool_call_count(),
            final_status: final_status.to_owned(),
            has_approval_pause,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceReplaySummary {
    pub trace_id: String,
    pub total_events: usize,
    pub tool_call_count: usize,
    pub final_status: String,
    pub has_approval_pause: bool,
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Trace Rollout",
        "ai-foundation",
        "Agent trace bundles, replay summaries, rollout snapshots, and eval capture boundaries.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_bundle_orders_events_and_counts_tool_calls() {
        let bundle = TraceBundle::new("agent-1")
            .with_event(TraceEvent::user_message(2, "hi"))
            .with_event(TraceEvent::tool_call(3, "call-1", "rag.search"))
            .with_event(TraceEvent::final_answer(4, "done"));

        assert_eq!(bundle.trace_id, "agent-1");
        assert_eq!(bundle.tool_call_count(), 1);
        assert_eq!(bundle.events[0].sequence_no, 2);
    }
}
