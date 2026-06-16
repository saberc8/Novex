use novex_agent_protocol::{AgentTurnItem, TurnOutcome};
use serde::{Deserialize, Serialize};

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
}
