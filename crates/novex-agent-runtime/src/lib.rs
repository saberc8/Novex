use novex_agent_protocol::{AgentTurnItem, TurnOutcome};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CRATE_ID: &str = "novex-agent-runtime";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeBudget {
    pub max_turns: usize,
    pub max_tool_calls: usize,
}

impl Default for AgentRuntimeBudget {
    fn default() -> Self {
        Self {
            max_turns: 8,
            max_tool_calls: 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeState {
    pub run_ref: String,
    pub budget: AgentRuntimeBudget,
    pub items: Vec<AgentTurnItem>,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedModelTurnOutput {
    pub item: AgentTurnItem,
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
        if value.get("type").and_then(Value::as_str) == Some("tool_call") {
            let call_id = value
                .get("callId")
                .and_then(Value::as_str)
                .unwrap_or("call-1")
                .to_owned();
            let tool_code = value
                .get("toolCode")
                .and_then(Value::as_str)
                .ok_or_else(|| ModelTurnParseError {
                    message: "toolCode is required".to_owned(),
                })?
                .to_owned();
            let arguments = value.get("arguments").cloned().unwrap_or(Value::Null);
            return Ok(ParsedModelTurnOutput {
                item: AgentTurnItem::tool_call(call_id, tool_code, arguments),
                outcome: TurnOutcome::NeedsFollowUp,
            });
        }
    }

    Ok(ParsedModelTurnOutput {
        item: AgentTurnItem::FinalAnswer {
            content: trimmed.to_owned(),
        },
        outcome: TurnOutcome::Final,
    })
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
        };
        let mut state = AgentRuntimeState::with_budget("run-1", budget);
        state.push_item(AgentTurnItem::tool_call("call-1", "rag.search", json!({})));
        state.push_item(AgentTurnItem::tool_call("call-2", "rag.search", json!({})));

        assert_eq!(state.next_outcome(), TurnOutcome::BudgetExceeded);
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
        assert_eq!(parsed.outcome, TurnOutcome::NeedsFollowUp);
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
        assert_eq!(parsed.outcome, TurnOutcome::Final);
    }
}
