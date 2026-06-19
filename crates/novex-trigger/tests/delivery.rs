use novex_trigger::{
    is_supported_target_kind, plan_trigger_delivery, TriggerDeliveryInput, TriggerRetryPolicy,
    ACCEPTED_DELIVERY_STATUS, DEAD_LETTER_DELIVERY_STATUS,
};

#[test]
fn delivery_target_kind_matches_runtime_route_contract() {
    for target_kind in ["run_graph", "agent_run", "job", "notification"] {
        assert!(is_supported_target_kind(target_kind));
    }

    assert!(!is_supported_target_kind("flow_builder"));
}

#[test]
fn delivery_plan_tracks_trace_and_retry_policy_for_supported_targets() {
    let plan = plan_trigger_delivery(TriggerDeliveryInput {
        trigger_id: 42,
        trigger_code: "webhook.training.event".to_owned(),
        target_kind: "agent_run".to_owned(),
        route_config: serde_json::json!({"agentCode":"training-assistant"}),
        event_id: 9001,
        retry_policy: TriggerRetryPolicy {
            max_attempts: 3,
            backoff_seconds: vec![30, 300],
        },
    });

    assert_eq!(plan.status, ACCEPTED_DELIVERY_STATUS);
    assert_eq!(plan.trace_id, Some(9001));
    assert_eq!(plan.retry_policy.max_attempts, 3);
    assert_eq!(
        plan.route_snapshot["deliveryStatus"],
        ACCEPTED_DELIVERY_STATUS
    );
    assert_eq!(plan.route_snapshot["retryPolicy"]["maxAttempts"], 3);
    assert_eq!(plan.route_snapshot["deadLetter"], false);
}

#[test]
fn delivery_plan_dead_letters_unsupported_targets_without_retry() {
    let plan = plan_trigger_delivery(TriggerDeliveryInput {
        trigger_id: 42,
        trigger_code: "webhook.training.event".to_owned(),
        target_kind: "flow_builder".to_owned(),
        route_config: serde_json::json!({"flowId":"later"}),
        event_id: 9002,
        retry_policy: TriggerRetryPolicy {
            max_attempts: 3,
            backoff_seconds: vec![30, 300],
        },
    });

    assert_eq!(plan.status, DEAD_LETTER_DELIVERY_STATUS);
    assert_eq!(plan.retry_policy.max_attempts, 0);
    assert!(plan
        .error_message
        .as_deref()
        .unwrap()
        .contains("unsupported trigger target kind"));
    assert_eq!(plan.route_snapshot["deadLetter"], true);
    assert_eq!(plan.route_snapshot["retryPolicy"]["maxAttempts"], 0);
}
