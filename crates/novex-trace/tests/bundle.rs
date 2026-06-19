use novex_trace::{TraceBundle, TraceEvent, TraceEventKind};

#[test]
fn trace_bundle_orders_events_and_counts_tool_calls() {
    let bundle = TraceBundle::new("agent-1")
        .with_event(TraceEvent::user_message(2, "hi"))
        .with_event(TraceEvent::tool_call(3, "call-1", "rag.search"))
        .with_event(TraceEvent::final_answer(4, "done"));

    assert_eq!(bundle.trace_id, "agent-1");
    assert_eq!(bundle.tool_call_count(), 1);
    assert_eq!(bundle.events[0].sequence_no, 2);
}

#[test]
fn trace_bundle_preserves_runtime_span_events() {
    let bundle = TraceBundle::new("agent-1")
        .with_event(TraceEvent::retrieval(1, serde_json::json!({"hitCount":2})))
        .with_event(TraceEvent::action_selected(
            2,
            serde_json::json!({"toolCallBatch":[{"toolCode":"rag.search"}]}),
        ))
        .with_event(TraceEvent::context_compaction(
            3,
            serde_json::json!({"compactedItemCount":4}),
        ))
        .with_event(TraceEvent::cancellation(
            4,
            serde_json::json!({"cancelReason":"external_cancel"}),
        ));

    assert_eq!(bundle.events[0].kind, TraceEventKind::Retrieval);
    assert_eq!(bundle.events[1].kind, TraceEventKind::ActionSelected);
    assert_eq!(bundle.events[2].kind, TraceEventKind::ContextCompaction);
    assert_eq!(bundle.events[3].kind, TraceEventKind::Cancellation);
    assert_eq!(bundle.replay_summary().final_status, "cancelled");
}
