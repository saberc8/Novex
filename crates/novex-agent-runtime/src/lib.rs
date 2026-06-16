use novex_agent_protocol::{AgentTurnItem, TurnOutcome};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CRATE_ID: &str = "novex-agent-runtime";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeBudget {
    pub max_turns: usize,
    pub max_tool_calls: usize,
    #[serde(default)]
    pub compact_after_observations: Option<usize>,
}

impl Default for AgentRuntimeBudget {
    fn default() -> Self {
        Self {
            max_turns: 8,
            max_tool_calls: 4,
            compact_after_observations: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContextCompaction {
    pub window_id: u64,
    pub summary: String,
    pub retained_item_count: usize,
    pub compacted_item_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeState {
    pub run_ref: String,
    pub budget: AgentRuntimeBudget,
    pub items: Vec<AgentTurnItem>,
    #[serde(default)]
    pub compaction_window_id: u64,
}

impl AgentRuntimeState {
    pub fn new(run_ref: impl Into<String>) -> Self {
        Self::with_budget(run_ref, AgentRuntimeBudget::default())
    }

    pub fn with_budget(run_ref: impl Into<String>, budget: AgentRuntimeBudget) -> Self {
        Self {
            run_ref: run_ref.into(),
            budget,
            items: Vec::new(),
            compaction_window_id: 0,
        }
    }

    pub fn push_item(&mut self, item: AgentTurnItem) {
        self.items.push(item);
    }

    pub fn tool_call_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item, AgentTurnItem::ToolCall { .. }))
            .count()
    }

    pub fn can_execute_tool_call(&self) -> bool {
        !self.is_tool_call_budget_exhausted()
    }

    pub fn can_execute_tool_calls(&self, requested: usize) -> bool {
        requested <= self.remaining_tool_call_budget()
    }

    pub fn remaining_tool_call_budget(&self) -> usize {
        self.budget
            .max_tool_calls
            .saturating_sub(self.tool_call_count())
    }

    pub fn is_tool_call_budget_exhausted(&self) -> bool {
        self.tool_call_count() >= self.budget.max_tool_calls
    }

    pub fn turn_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    AgentTurnItem::UserMessage { .. } | AgentTurnItem::ToolObservation { .. }
                )
            })
            .count()
    }

    pub fn next_outcome(&self) -> TurnOutcome {
        if self.tool_call_count() > self.budget.max_tool_calls
            || self.turn_count() > self.budget.max_turns
        {
            return TurnOutcome::BudgetExceeded;
        }
        if self
            .items
            .last()
            .is_some_and(AgentTurnItem::requires_follow_up)
        {
            return TurnOutcome::NeedsFollowUp;
        }
        TurnOutcome::Final
    }

    pub fn should_compact_context(&self) -> bool {
        let Some(threshold) = self.budget.compact_after_observations else {
            return false;
        };
        if threshold == 0 {
            return false;
        }
        self.observation_count_since_last_compaction() >= threshold
    }

    pub fn compact_context(&mut self) -> Option<AgentContextCompaction> {
        let summary = self.compaction_candidate_summary()?;
        self.compact_context_with_summary(summary)
    }

    pub fn compaction_candidate_summary(&self) -> Option<String> {
        self.should_compact_context()
            .then(|| build_compaction_summary(&self.items_since_last_compaction()))
    }

    pub fn compact_context_with_summary(
        &mut self,
        summary: impl Into<String>,
    ) -> Option<AgentContextCompaction> {
        if !self.should_compact_context() {
            return None;
        }

        let compacted_item_count = self.items_since_last_compaction().len();
        let summary = summary.into();
        self.compaction_window_id = self.compaction_window_id.saturating_add(1);
        self.items.push(AgentTurnItem::ContextCompaction {
            summary: summary.clone(),
        });

        Some(AgentContextCompaction {
            window_id: self.compaction_window_id,
            summary,
            retained_item_count: 1,
            compacted_item_count,
        })
    }

    fn observation_count_since_last_compaction(&self) -> usize {
        self.items_since_last_compaction()
            .iter()
            .filter(|item| matches!(item, AgentTurnItem::ToolObservation { .. }))
            .count()
    }

    fn items_since_last_compaction(&self) -> Vec<AgentTurnItem> {
        self.items
            .iter()
            .rev()
            .take_while(|item| !matches!(item, AgentTurnItem::ContextCompaction { .. }))
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}

fn build_compaction_summary(items: &[AgentTurnItem]) -> String {
    let mut lines = vec!["Compacted prior agent context:".to_owned()];
    for item in items {
        match item {
            AgentTurnItem::UserMessage { content } => {
                lines.push(format!("User: {}", compact_text(content, 500)));
            }
            AgentTurnItem::AssistantMessage { content } => {
                lines.push(format!("Assistant: {}", compact_text(content, 500)));
            }
            AgentTurnItem::Reasoning { summary } => {
                lines.push(format!("Reasoning: {}", compact_text(summary, 500)));
            }
            AgentTurnItem::ToolCall {
                call_id,
                tool_code,
                arguments,
            } => {
                lines.push(format!(
                    "Tool call {call_id} `{tool_code}` args: {}",
                    compact_text(&arguments.to_string(), 500)
                ));
            }
            AgentTurnItem::ToolObservation {
                call_id,
                status,
                output,
            } => {
                lines.push(format!(
                    "Observation for {call_id} ({status:?}): {}",
                    compact_text(&output.to_string(), 1000)
                ));
            }
            AgentTurnItem::FinalAnswer { content } => {
                lines.push(format!("Final answer: {}", compact_text(content, 500)));
            }
            AgentTurnItem::ContextCompaction { summary } => {
                lines.push(format!(
                    "Previous compaction: {}",
                    compact_text(summary, 500)
                ));
            }
        }
    }
    lines.join("\n")
}

fn compact_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let mut compacted = text.chars().take(max_chars).collect::<String>();
    compacted.push_str("...");
    compacted
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedModelTurnOutput {
    pub item: AgentTurnItem,
    #[serde(default)]
    pub items: Vec<AgentTurnItem>,
    pub outcome: TurnOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelTurnParseError {
    pub message: String,
}

pub fn parse_model_turn_output(output: &str) -> Result<ParsedModelTurnOutput, ModelTurnParseError> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err(ModelTurnParseError {
            message: "model output is empty".to_owned(),
        });
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        match value.get("type").and_then(Value::as_str) {
            Some("tool_call") => {
                let item = parse_tool_call_value(&value, 0)?;
                return Ok(ParsedModelTurnOutput {
                    item: item.clone(),
                    items: vec![item],
                    outcome: TurnOutcome::NeedsFollowUp,
                });
            }
            Some("tool_calls") => {
                let calls = value
                    .get("calls")
                    .and_then(Value::as_array)
                    .ok_or_else(|| ModelTurnParseError {
                        message: "tool_calls.calls is required".to_owned(),
                    })?;
                if calls.is_empty() {
                    return Err(ModelTurnParseError {
                        message: "tool_calls requires at least one call".to_owned(),
                    });
                }

                let items = calls
                    .iter()
                    .enumerate()
                    .map(|(index, call)| parse_tool_call_value(call, index))
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(ParsedModelTurnOutput {
                    item: items[0].clone(),
                    items,
                    outcome: TurnOutcome::NeedsFollowUp,
                });
            }
            _ => {}
        }
    }

    let item = AgentTurnItem::FinalAnswer {
        content: trimmed.to_owned(),
    };
    Ok(ParsedModelTurnOutput {
        item: item.clone(),
        items: vec![item],
        outcome: TurnOutcome::Final,
    })
}

fn parse_tool_call_value(
    value: &Value,
    index: usize,
) -> Result<AgentTurnItem, ModelTurnParseError> {
    let call_id = value
        .get("callId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("call-{}", index + 1));
    let tool_code = value
        .get("toolCode")
        .and_then(Value::as_str)
        .ok_or_else(|| ModelTurnParseError {
            message: "toolCode is required".to_owned(),
        })?
        .to_owned();
    let arguments = value.get("arguments").cloned().unwrap_or(Value::Null);

    Ok(AgentTurnItem::tool_call(call_id, tool_code, arguments))
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_agent_protocol::{AgentTurnItem, ToolObservationStatus, TurnOutcome};
    use serde_json::json;

    #[test]
    fn runtime_state_continues_after_observation() {
        let mut state = AgentRuntimeState::new("run-1");
        state.push_item(AgentTurnItem::user_message("search policy"));
        state.push_item(AgentTurnItem::tool_call(
            "call-1",
            "rag.search",
            json!({"query":"policy"}),
        ));
        state.push_item(AgentTurnItem::tool_observation(
            "call-1",
            ToolObservationStatus::Succeeded,
            json!({"hits": []}),
        ));

        assert_eq!(state.next_outcome(), TurnOutcome::NeedsFollowUp);
        assert_eq!(state.tool_call_count(), 1);
    }

    #[test]
    fn runtime_budget_stops_excessive_tool_calls() {
        let budget = AgentRuntimeBudget {
            max_turns: 4,
            max_tool_calls: 1,
            compact_after_observations: None,
        };
        let mut state = AgentRuntimeState::with_budget("run-1", budget);
        state.push_item(AgentTurnItem::tool_call("call-1", "rag.search", json!({})));
        state.push_item(AgentTurnItem::tool_call("call-2", "rag.search", json!({})));

        assert_eq!(state.next_outcome(), TurnOutcome::BudgetExceeded);
    }

    #[test]
    fn runtime_budget_allows_tool_calls_up_to_limit() {
        let budget = AgentRuntimeBudget {
            max_turns: 4,
            max_tool_calls: 1,
            compact_after_observations: None,
        };
        let state = AgentRuntimeState::with_budget("run-1", budget);

        assert!(state.can_execute_tool_call());
        assert!(!state.is_tool_call_budget_exhausted());
    }

    #[test]
    fn runtime_budget_exceeds_when_tool_calls_reach_limit_before_next_call() {
        let budget = AgentRuntimeBudget {
            max_turns: 4,
            max_tool_calls: 1,
            compact_after_observations: None,
        };
        let mut state = AgentRuntimeState::with_budget("run-1", budget);
        state.push_item(AgentTurnItem::tool_call("call-1", "rag.search", json!({})));

        assert!(!state.can_execute_tool_call());
        assert!(state.is_tool_call_budget_exhausted());
    }

    #[test]
    fn runtime_budget_reports_remaining_tool_call_capacity() {
        let budget = AgentRuntimeBudget {
            max_turns: 4,
            max_tool_calls: 3,
            compact_after_observations: None,
        };
        let mut state = AgentRuntimeState::with_budget("run-1", budget);
        state.push_item(AgentTurnItem::tool_call("call-1", "rag.search", json!({})));

        assert_eq!(state.remaining_tool_call_budget(), 2);
        assert!(state.can_execute_tool_calls(2));
        assert!(!state.can_execute_tool_calls(3));
    }

    #[test]
    fn runtime_compaction_is_needed_after_observation_threshold() {
        let budget = AgentRuntimeBudget {
            max_turns: 8,
            max_tool_calls: 4,
            compact_after_observations: Some(2),
        };
        let mut state = AgentRuntimeState::with_budget("run-1", budget);
        state.push_item(AgentTurnItem::user_message("find policy"));
        state.push_item(AgentTurnItem::tool_observation(
            "call-1",
            ToolObservationStatus::Succeeded,
            json!({"hits":[{"title":"A"}]}),
        ));
        assert!(!state.should_compact_context());
        state.push_item(AgentTurnItem::tool_observation(
            "call-2",
            ToolObservationStatus::Succeeded,
            json!({"hits":[{"title":"B"}]}),
        ));
        assert!(state.should_compact_context());
    }

    #[test]
    fn runtime_compaction_pushes_summary_and_advances_window() {
        let budget = AgentRuntimeBudget {
            max_turns: 8,
            max_tool_calls: 4,
            compact_after_observations: Some(1),
        };
        let mut state = AgentRuntimeState::with_budget("run-1", budget);
        state.push_item(AgentTurnItem::user_message("find policy"));
        state.push_item(AgentTurnItem::tool_call(
            "call-1",
            "rag.search",
            json!({"query":"policy"}),
        ));
        state.push_item(AgentTurnItem::tool_observation(
            "call-1",
            ToolObservationStatus::Succeeded,
            json!({"hits":[{"citation":"doc#1","text":"refund within 7 days"}]}),
        ));

        let compaction = state.compact_context().unwrap();

        assert_eq!(compaction.window_id, 1);
        assert!(compaction.summary.contains("refund within 7 days"));
        assert!(!state.should_compact_context());
        assert!(matches!(
            state.items.last(),
            Some(AgentTurnItem::ContextCompaction { .. })
        ));
    }

    #[test]
    fn runtime_compaction_can_install_model_generated_summary() {
        let budget = AgentRuntimeBudget {
            max_turns: 8,
            max_tool_calls: 4,
            compact_after_observations: Some(1),
        };
        let mut state = AgentRuntimeState::with_budget("run-1", budget);
        state.push_item(AgentTurnItem::user_message("summarize refund policy"));
        state.push_item(AgentTurnItem::tool_observation(
            "call-1",
            ToolObservationStatus::Succeeded,
            json!({"hits":[{"text":"refund within 7 days"}]}),
        ));

        let candidate = state.compaction_candidate_summary().unwrap();
        assert!(candidate.contains("refund within 7 days"));

        let compaction = state
            .compact_context_with_summary("Model summary: refunds are allowed within 7 days.")
            .unwrap();

        assert_eq!(compaction.window_id, 1);
        assert_eq!(
            compaction.summary,
            "Model summary: refunds are allowed within 7 days."
        );
        assert!(!state.should_compact_context());
        assert!(matches!(
            state.items.last(),
            Some(AgentTurnItem::ContextCompaction { summary })
                if summary == "Model summary: refunds are allowed within 7 days."
        ));
    }

    #[test]
    fn parser_reads_json_tool_call_from_model_answer() {
        let parsed = parse_model_turn_output(
            r#"{"type":"tool_call","callId":"call-1","toolCode":"rag.search","arguments":{"query":"policy"}}"#,
        )
        .unwrap();

        assert_eq!(
            parsed.item,
            AgentTurnItem::tool_call(
                "call-1",
                "rag.search",
                serde_json::json!({"query":"policy"})
            )
        );
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.outcome, TurnOutcome::NeedsFollowUp);
    }

    #[test]
    fn parser_reads_json_tool_call_batch_from_model_answer() {
        let parsed = parse_model_turn_output(
            r#"{"type":"tool_calls","calls":[{"callId":"call-1","toolCode":"rag.search","arguments":{"query":"policy"}},{"callId":"call-2","toolCode":"github.repo.read","arguments":{"repository":"org/repo","path":"README.md"}}]}"#,
        )
        .unwrap();

        assert_eq!(parsed.outcome, TurnOutcome::NeedsFollowUp);
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(
            parsed.items[0],
            AgentTurnItem::tool_call(
                "call-1",
                "rag.search",
                serde_json::json!({"query":"policy"})
            )
        );
        assert_eq!(
            parsed.items[1],
            AgentTurnItem::tool_call(
                "call-2",
                "github.repo.read",
                serde_json::json!({"repository":"org/repo","path":"README.md"})
            )
        );
    }

    #[test]
    fn parser_rejects_empty_tool_call_batch() {
        let err = parse_model_turn_output(r#"{"type":"tool_calls","calls":[]}"#).unwrap_err();

        assert_eq!(err.message, "tool_calls requires at least one call");
    }

    #[test]
    fn parser_treats_plain_text_as_final_answer() {
        let parsed = parse_model_turn_output("Here is the answer.").unwrap();

        assert_eq!(
            parsed.item,
            AgentTurnItem::FinalAnswer {
                content: "Here is the answer.".to_owned()
            }
        );
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.outcome, TurnOutcome::Final);
    }
}
