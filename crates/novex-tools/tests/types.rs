use novex_tools::*;

#[test]
fn agent_tool_execution_envelope_builds_success_failure_and_cancelled_statuses() {
    let succeeded = AgentToolExecution::succeeded(
        serde_json::json!({"status": "succeeded", "answer": "ok"}),
        true,
        "Agent dry-run executed.".to_owned(),
    );
    assert_eq!(succeeded.status, "succeeded");
    assert!(succeeded.dry_run);
    assert_eq!(succeeded.error_message, None);
    assert_eq!(succeeded.final_output, "Agent dry-run executed.");
    assert_eq!(succeeded.response_payload["answer"], "ok");
    assert!(succeeded.succeeded_status());
    assert!(!succeeded.cancelled_status());

    let failed = AgentToolExecution::failed(
        serde_json::json!({"status": "failed", "error": "boom"}),
        "boom".to_owned(),
        "Agent failed.".to_owned(),
    );
    assert_eq!(failed.status, "failed");
    assert!(!failed.dry_run);
    assert_eq!(failed.error_message.as_deref(), Some("boom"));
    assert_eq!(failed.final_output, "Agent failed.");
    assert!(!failed.succeeded_status());
    assert!(!failed.cancelled_status());

    let cancelled = AgentToolExecution::cancelled(
        serde_json::json!({"status": "cancelled"}),
        "Tool was cancelled.".to_owned(),
    );
    assert_eq!(cancelled.status, "cancelled");
    assert!(!cancelled.dry_run);
    assert_eq!(
        cancelled.error_message.as_deref(),
        Some("Tool was cancelled.")
    );
    assert_eq!(cancelled.final_output, "Tool was cancelled.");
    assert!(!cancelled.succeeded_status());
    assert!(cancelled.cancelled_status());
}
