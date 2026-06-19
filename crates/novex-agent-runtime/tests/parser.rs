use novex_agent_protocol::{AgentTurnItem, TurnOutcome};
use novex_agent_runtime::*;
use serde_json::json;

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
fn streaming_parser_waits_for_complete_tool_call_json() {
    let mut parser = StreamingModelTurnParser::new();

    assert_eq!(
        parser.push_delta(r#"{"type":"tool_"#).unwrap(),
        StreamingModelTurnParseStatus::Pending
    );
    let status = parser
        .push_delta(
            r#"call","callId":"call-1","toolCode":"rag.search","arguments":{"query":"policy"}}"#,
        )
        .unwrap();

    match status {
        StreamingModelTurnParseStatus::Ready(parsed) => {
            assert_eq!(parsed.outcome, TurnOutcome::NeedsFollowUp);
            assert_eq!(parsed.items.len(), 1);
            assert_eq!(
                parsed.item,
                AgentTurnItem::tool_call("call-1", "rag.search", json!({"query":"policy"}))
            );
        }
        StreamingModelTurnParseStatus::Pending => panic!("expected complete tool call"),
    }
}

#[test]
fn streaming_parser_reads_tool_call_batch_across_chunks() {
    let mut parser = StreamingModelTurnParser::new();

    assert_eq!(
        parser
            .push_delta(r#"{"type":"tool_calls","calls":[{"callId":"call-1","#)
            .unwrap(),
        StreamingModelTurnParseStatus::Pending
    );
    let status = parser
        .push_delta(
            r#""toolCode":"rag.search","arguments":{"query":"policy"}},{"callId":"call-2","toolCode":"github.repo.read","arguments":{"repository":"org/repo","path":"README.md"}}]}"#,
        )
        .unwrap();

    match status {
        StreamingModelTurnParseStatus::Ready(parsed) => {
            assert_eq!(parsed.outcome, TurnOutcome::NeedsFollowUp);
            assert_eq!(parsed.items.len(), 2);
            assert_eq!(
                parsed.items[0],
                AgentTurnItem::tool_call("call-1", "rag.search", json!({"query":"policy"}))
            );
            assert_eq!(
                parsed.items[1],
                AgentTurnItem::tool_call(
                    "call-2",
                    "github.repo.read",
                    json!({"repository":"org/repo","path":"README.md"})
                )
            );
        }
        StreamingModelTurnParseStatus::Pending => panic!("expected complete tool-call batch"),
    }
}

#[test]
fn streaming_parser_keeps_natural_language_pending() {
    let mut parser = StreamingModelTurnParser::new();

    assert_eq!(
        parser.push_delta("Here is the answer").unwrap(),
        StreamingModelTurnParseStatus::Pending
    );
    assert_eq!(
        parser.push_delta(" with more text.").unwrap(),
        StreamingModelTurnParseStatus::Pending
    );
}

#[test]
fn streaming_parser_rejects_oversized_buffer() {
    let mut parser = StreamingModelTurnParser::with_max_chars(8);

    let err = parser.push_delta(r#"{"type":"tool_call"}"#).unwrap_err();

    assert_eq!(err.message, "streaming model turn exceeded buffer limit");
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
