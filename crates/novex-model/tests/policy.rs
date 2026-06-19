use novex_model::*;
use serde_json::Value;

#[test]
fn model_route_policy_defaults_to_disabled_fallback() {
    let status = evaluate_model_route_policy(ModelRoutePolicyInput {
        network_zone: "public",
        fallback_network_zone: None,
        fallback_policy: &Value::Null,
        route_policy: &Value::Null,
    });

    assert_eq!(status.network_zone, "public");
    assert!(!status.fallback_enabled);
    assert!(!status.cross_zone_fallback_allowed);
    assert_eq!(status.max_retries, 0);
    assert_eq!(status.circuit_breaker_seconds, 0);
    assert!(status.violations.is_empty());
}

#[test]
fn model_route_policy_blocks_cross_zone_fallback_without_explicit_policy() {
    let policy = serde_json::json!({
        "enabled": true,
        "maxRetries": 2,
        "circuitBreakerSeconds": 45
    });

    let status = evaluate_model_route_policy(ModelRoutePolicyInput {
        network_zone: "private",
        fallback_network_zone: Some("public"),
        fallback_policy: &policy,
        route_policy: &Value::Null,
    });

    assert!(status.fallback_enabled);
    assert_eq!(status.max_retries, 2);
    assert_eq!(status.circuit_breaker_seconds, 45);
    assert!(!status.cross_zone_fallback_allowed);
    assert_eq!(
        status.violations,
        vec!["cross_zone_fallback_not_allowed".to_owned()]
    );
}

#[test]
fn model_route_policy_allows_cross_zone_fallback_when_policy_explicit() {
    let policy = serde_json::json!({
        "enabled": true,
        "allowCrossZone": true,
        "max_retries": 1,
        "circuit_breaker_seconds": 30
    });

    let status = evaluate_model_route_policy(ModelRoutePolicyInput {
        network_zone: "private",
        fallback_network_zone: Some("public"),
        fallback_policy: &policy,
        route_policy: &Value::Null,
    });

    assert!(status.fallback_enabled);
    assert!(status.cross_zone_fallback_allowed);
    assert_eq!(status.max_retries, 1);
    assert_eq!(status.circuit_breaker_seconds, 30);
    assert!(status.violations.is_empty());
}
