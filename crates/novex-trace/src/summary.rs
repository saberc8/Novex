use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceReplaySummary {
    pub trace_id: String,
    pub total_events: usize,
    pub tool_call_count: usize,
    pub final_status: String,
    pub has_approval_pause: bool,
}
