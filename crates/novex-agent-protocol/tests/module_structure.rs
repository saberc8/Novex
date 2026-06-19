use std::fs;
use std::path::Path;

use novex_agent_protocol::{AgentTurnItem, ToolObservationStatus, TurnOutcome};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_agent_protocol_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["item", "outcome"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum AgentTurnItem",
        "pub enum TurnOutcome",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn agent_protocol_modules_exist() {
    for module in ["src/item.rs", "src/outcome.rs"] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_agent_protocol_contracts() {
    let item = AgentTurnItem::tool_observation(
        "call-1",
        ToolObservationStatus::Succeeded,
        serde_json::json!({"hits": 2}),
    );

    assert_eq!(item.call_id(), Some("call-1"));
    assert!(item.requires_follow_up());
    assert!(TurnOutcome::Final.is_terminal());
    assert!(!TurnOutcome::NeedsFollowUp.is_terminal());
}
