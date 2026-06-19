use novex_trace::{TraceBundle, TraceEvent, TraceEventKind};

#[test]
fn trace_bundle_preserves_inference_span_events() {
    let bundle = TraceBundle::new("agent-1").with_event(TraceEvent::inference(
        1,
        serde_json::json!({
            "routeId": "runtime.llm.code_agent",
            "provider": "deep-seek",
            "latencyMs": 42
        }),
    ));

    assert_eq!(bundle.events[0].kind, TraceEventKind::Inference);
    assert_eq!(bundle.events[0].payload["latencyMs"], 42);
}
