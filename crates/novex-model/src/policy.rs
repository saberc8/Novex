use crate::util::{json_bool_field, json_i64_field};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelRoutePolicyInput<'a> {
    pub network_zone: &'a str,
    pub fallback_network_zone: Option<&'a str>,
    pub fallback_policy: &'a Value,
    pub route_policy: &'a Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRoutePolicyStatus {
    pub network_zone: String,
    pub fallback_network_zone: Option<String>,
    pub fallback_enabled: bool,
    pub cross_zone_fallback_allowed: bool,
    pub max_retries: u32,
    pub circuit_breaker_seconds: u32,
    pub violations: Vec<String>,
}

pub fn evaluate_model_route_policy(input: ModelRoutePolicyInput<'_>) -> ModelRoutePolicyStatus {
    let network_zone = normalize_network_zone(input.network_zone);
    let fallback_network_zone = input.fallback_network_zone.map(normalize_network_zone);
    let fallback_enabled = policy_bool_field(
        input.route_policy,
        input.fallback_policy,
        &["fallbackEnabled", "fallback_enabled", "enabled"],
    )
    .unwrap_or(false);
    let cross_zone_fallback_allowed = policy_bool_field(
        input.route_policy,
        input.fallback_policy,
        &[
            "allowCrossZone",
            "allow_cross_zone",
            "allowCrossNetworkZone",
            "allow_cross_network_zone",
            "crossZoneFallback",
            "cross_zone_fallback",
        ],
    )
    .unwrap_or(false);
    let max_retries = policy_u32_field(
        input.route_policy,
        input.fallback_policy,
        &["maxRetries", "max_retries", "retryCount", "retry_count"],
    )
    .unwrap_or(0);
    let circuit_breaker_seconds = policy_u32_field(
        input.route_policy,
        input.fallback_policy,
        &[
            "circuitBreakerSeconds",
            "circuit_breaker_seconds",
            "circuitBreakerCooldownSeconds",
            "circuit_breaker_cooldown_seconds",
        ],
    )
    .unwrap_or(0);

    let mut violations = Vec::new();
    if fallback_enabled
        && fallback_network_zone
            .as_deref()
            .is_some_and(|fallback_zone| fallback_zone != network_zone)
        && !cross_zone_fallback_allowed
    {
        violations.push("cross_zone_fallback_not_allowed".to_owned());
    }

    ModelRoutePolicyStatus {
        network_zone,
        fallback_network_zone,
        fallback_enabled,
        cross_zone_fallback_allowed,
        max_retries,
        circuit_breaker_seconds,
        violations,
    }
}

fn policy_bool_field(route_policy: &Value, fallback_policy: &Value, keys: &[&str]) -> Option<bool> {
    json_bool_field(route_policy, keys).or_else(|| json_bool_field(fallback_policy, keys))
}

fn policy_u32_field(route_policy: &Value, fallback_policy: &Value, keys: &[&str]) -> Option<u32> {
    json_i64_field(route_policy, keys)
        .or_else(|| json_i64_field(fallback_policy, keys))
        .map(|value| value.min(u32::MAX as i64) as u32)
}

fn normalize_network_zone(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() {
        "unknown".to_owned()
    } else {
        value
    }
}
