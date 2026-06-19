use novex_agent_protocol::{AgentTurnItem, ToolObservationStatus, TurnOutcome};
use novex_agent_runtime::*;
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
fn remote_compaction_request_exposes_endpoint_metadata() {
    let budget = AgentRuntimeBudget {
        max_turns: 8,
        max_tool_calls: 4,
        compact_after_observations: Some(1),
    };
    let mut state = AgentRuntimeState::with_budget("run-1", budget);
    state.push_item(AgentTurnItem::user_message("find refund policy"));
    state.push_item(AgentTurnItem::tool_call(
        "call-1",
        "rag.search",
        json!({"query":"refund"}),
    ));
    state.push_item(AgentTurnItem::tool_observation(
        "call-1",
        ToolObservationStatus::Succeeded,
        json!({"hits":[{"text":"refund within 7 days"}]}),
    ));

    let request = state
        .remote_compaction_request(vec!["rag.search".to_owned(), "github.repo.read".to_owned()])
        .unwrap();

    assert_eq!(request.window_id, 1);
    assert_eq!(
        request.implementation,
        AgentRemoteCompactionImplementation::ResponsesCompactionV2
    );
    assert_eq!(request.trigger, AgentCompactionTrigger::Auto);
    assert_eq!(request.reason, AgentCompactionReason::ObservationThreshold);
    assert_eq!(request.phase, AgentCompactionPhase::ModelLoopFollowUp);
    assert_eq!(request.compacted_item_count, 3);
    assert_eq!(request.retained_item_count, 1);
    assert_eq!(
        request.tool_codes,
        vec!["rag.search".to_owned(), "github.repo.read".to_owned()]
    );
}

#[test]
fn remote_compaction_request_retains_user_and_previous_summary() {
    let budget = AgentRuntimeBudget {
        max_turns: 8,
        max_tool_calls: 4,
        compact_after_observations: Some(1),
    };
    let mut state = AgentRuntimeState::with_budget("run-1", budget);
    state.push_item(AgentTurnItem::ContextCompaction {
        summary: "previous compacted context".to_owned(),
    });
    state.push_item(AgentTurnItem::user_message("continue"));
    state.push_item(AgentTurnItem::tool_observation(
        "call-2",
        ToolObservationStatus::Succeeded,
        json!({"text":"new evidence"}),
    ));

    let request = state.remote_compaction_request(vec![]).unwrap();

    assert!(request
        .retained_history
        .iter()
        .any(|item| matches!(item, AgentTurnItem::UserMessage { .. })));
    assert!(!request
        .retained_history
        .iter()
        .any(|item| matches!(item, AgentTurnItem::ToolObservation { .. })));
}
