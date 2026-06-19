use novex_agent_protocol::{AgentTurnItem, ToolObservationStatus};
use serde_json::json;

#[test]
fn turn_item_serializes_with_snake_case_type_tags() {
    let item = AgentTurnItem::tool_call("call-1", "rag.search", json!({"query":"policy"}));
    let value = serde_json::to_value(item).unwrap();

    assert_eq!(value["type"], "tool_call");
    assert_eq!(value["callId"], "call-1");
    assert_eq!(value["toolCode"], "rag.search");
}

#[test]
fn tool_observation_links_to_call_id() {
    let item = AgentTurnItem::tool_observation(
        "call-1",
        ToolObservationStatus::Succeeded,
        json!({"hits": 2}),
    );

    assert_eq!(item.call_id(), Some("call-1"));
    assert!(item.requires_follow_up());
}
