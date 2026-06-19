use novex_agent_protocol::{AgentTurnItem, TurnOutcome};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRemoteCompactionImplementation {
    ResponsesCompactionV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCompactionTrigger {
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCompactionReason {
    ObservationThreshold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCompactionPhase {
    ModelLoopFollowUp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRemoteCompactionRequest {
    pub window_id: u64,
    pub implementation: AgentRemoteCompactionImplementation,
    pub trigger: AgentCompactionTrigger,
    pub reason: AgentCompactionReason,
    pub phase: AgentCompactionPhase,
    pub input_history: Vec<AgentTurnItem>,
    pub retained_history: Vec<AgentTurnItem>,
    pub tool_codes: Vec<String>,
    pub compacted_item_count: usize,
    pub retained_item_count: usize,
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

    pub fn remote_compaction_request(
        &self,
        tool_codes: Vec<String>,
    ) -> Option<AgentRemoteCompactionRequest> {
        if !self.should_compact_context() {
            return None;
        }

        let input_history = self.items_since_last_compaction();
        let retained_history = retained_remote_compaction_history(&input_history);
        Some(AgentRemoteCompactionRequest {
            window_id: self.compaction_window_id.saturating_add(1),
            implementation: AgentRemoteCompactionImplementation::ResponsesCompactionV2,
            trigger: AgentCompactionTrigger::Auto,
            reason: AgentCompactionReason::ObservationThreshold,
            phase: AgentCompactionPhase::ModelLoopFollowUp,
            compacted_item_count: input_history.len(),
            retained_item_count: retained_history.len(),
            input_history,
            retained_history,
            tool_codes,
        })
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

fn retained_remote_compaction_history(items: &[AgentTurnItem]) -> Vec<AgentTurnItem> {
    items
        .iter()
        .filter(|item| {
            matches!(
                item,
                AgentTurnItem::UserMessage { .. } | AgentTurnItem::ContextCompaction { .. }
            )
        })
        .cloned()
        .collect()
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
