use std::fs;
use std::path::Path;

use novex_agent_protocol::{AgentTurnItem, ToolObservationStatus, TurnOutcome};
use novex_agent_runtime::{
    parse_model_turn_output, AgentCompactionReason, AgentCompactionTrigger,
    AgentRemoteCompactionImplementation, AgentRuntimeBudget, AgentRuntimeState,
    StreamingModelTurnParseStatus, StreamingModelTurnParser, MAX_STREAMING_MODEL_TURN_BUFFER_CHARS,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_agent_runtime_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["parser", "state"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub struct AgentRuntimeBudget",
        "pub struct AgentRuntimeState",
        "pub struct ParsedModelTurnOutput",
        "pub struct StreamingModelTurnParser",
        "pub fn parse_model_turn_output",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn agent_runtime_domain_modules_exist() {
    for module in ["src/parser.rs", "src/state.rs"] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_runtime_state_and_parser_contracts() {
    let budget = AgentRuntimeBudget {
        max_turns: 8,
        max_tool_calls: 2,
        compact_after_observations: Some(1),
    };
    let mut state = AgentRuntimeState::with_budget("run-1", budget);
    state.push_item(AgentTurnItem::user_message("find policy"));
    state.push_item(AgentTurnItem::tool_call(
        "call-1",
        "rag.search",
        serde_json::json!({"query":"policy"}),
    ));
    state.push_item(AgentTurnItem::tool_observation(
        "call-1",
        ToolObservationStatus::Succeeded,
        serde_json::json!({"hits":[]}),
    ));

    assert_eq!(state.next_outcome(), TurnOutcome::NeedsFollowUp);
    assert!(state.should_compact_context());
    let request = state
        .remote_compaction_request(vec!["rag.search".to_owned()])
        .unwrap();
    assert_eq!(
        request.implementation,
        AgentRemoteCompactionImplementation::ResponsesCompactionV2
    );
    assert_eq!(request.trigger, AgentCompactionTrigger::Auto);
    assert_eq!(request.reason, AgentCompactionReason::ObservationThreshold);

    let parsed = parse_model_turn_output(
        r#"{"type":"tool_call","callId":"call-1","toolCode":"rag.search","arguments":{"query":"policy"}}"#,
    )
    .unwrap();
    assert_eq!(parsed.outcome, TurnOutcome::NeedsFollowUp);

    let mut streaming =
        StreamingModelTurnParser::with_max_chars(MAX_STREAMING_MODEL_TURN_BUFFER_CHARS);
    assert_eq!(
        streaming.push_delta("plain text").unwrap(),
        StreamingModelTurnParseStatus::Pending
    );
}
