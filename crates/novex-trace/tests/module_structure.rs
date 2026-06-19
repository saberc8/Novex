use std::fs;
use std::path::Path;

use novex_ai_core::FoundationStatus;
use novex_trace::{module, TraceBundle, TraceEvent, TraceEventKind, TraceReplaySummary};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_trace_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["bundle", "event", "module", "summary"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum TraceEventKind",
        "pub struct TraceEvent",
        "pub struct TraceBundle",
        "pub struct TraceReplaySummary",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn trace_domain_modules_exist() {
    for module in [
        "src/bundle.rs",
        "src/event.rs",
        "src/module.rs",
        "src/summary.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_trace_contracts() {
    let module = module();
    assert_eq!(module.id, "novex-trace");
    assert_eq!(module.status, FoundationStatus::Skeleton);

    let bundle = TraceBundle::new("agent-1")
        .with_event(TraceEvent::tool_call(2, "call-1", "rag.search"))
        .with_event(TraceEvent::final_answer(3, "done"));
    assert_eq!(bundle.events[0].kind, TraceEventKind::ToolCall);
    assert_eq!(bundle.tool_call_count(), 1);

    let summary: TraceReplaySummary = bundle.replay_summary();
    assert_eq!(summary.trace_id, "agent-1");
    assert_eq!(summary.final_status, "succeeded");
}
