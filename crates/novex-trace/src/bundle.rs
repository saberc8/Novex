use serde::{Deserialize, Serialize};

use crate::event::{TraceEvent, TraceEventKind};
use crate::summary::TraceReplaySummary;

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
        let has_cancellation = self
            .events
            .iter()
            .any(|event| matches!(event.kind, TraceEventKind::Cancellation));
        let final_status = if has_error {
            "failed"
        } else if has_cancellation && !has_final_answer {
            "cancelled"
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
