use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const CRATE_ID: &str = "novex-trigger";
pub const WEBHOOK_SIGNATURE_PREFIX: &str = "sha256=";
pub const ACCEPTED_DELIVERY_STATUS: &str = "accepted";
pub const DEAD_LETTER_DELIVERY_STATUS: &str = "dead_letter";
const MAX_IDEMPOTENCY_KEY_CHARS: usize = 128;

type HmacSha256 = hmac::Hmac<sha2::Sha256>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSourceKind {
    Webhook,
    Schedule,
    PluginEvent,
    ConnectorEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerTargetKind {
    RunGraph,
    AgentRun,
    Job,
    Notification,
}

pub fn is_supported_target_kind(target_kind: &str) -> bool {
    matches!(
        target_kind,
        "run_graph" | "agent_run" | "job" | "notification"
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerRetryPolicy {
    pub max_attempts: u8,
    pub backoff_seconds: Vec<u32>,
}

impl TriggerRetryPolicy {
    pub fn disabled() -> Self {
        Self {
            max_attempts: 0,
            backoff_seconds: Vec::new(),
        }
    }
}

impl Default for TriggerRetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff_seconds: vec![30, 300],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerDeliveryInput {
    pub trigger_id: i64,
    pub trigger_code: String,
    pub target_kind: String,
    pub route_config: Value,
    pub event_id: i64,
    pub retry_policy: TriggerRetryPolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerDeliveryPlan {
    pub status: String,
    pub trace_id: Option<i64>,
    pub error_message: Option<String>,
    pub retry_policy: TriggerRetryPolicy,
    pub route_snapshot: Value,
}

pub fn plan_trigger_delivery(input: TriggerDeliveryInput) -> TriggerDeliveryPlan {
    let supported = is_supported_target_kind(&input.target_kind);
    let status = if supported {
        ACCEPTED_DELIVERY_STATUS
    } else {
        DEAD_LETTER_DELIVERY_STATUS
    }
    .to_owned();
    let retry_policy = if supported {
        input.retry_policy
    } else {
        TriggerRetryPolicy::disabled()
    };
    let error_message = if supported {
        None
    } else {
        Some(format!(
            "unsupported trigger target kind: {}",
            input.target_kind
        ))
    };
    let route_snapshot = json!({
        "triggerId": input.trigger_id,
        "triggerCode": input.trigger_code,
        "targetKind": input.target_kind,
        "routeConfig": input.route_config,
        "deliveryStatus": status,
        "traceId": input.event_id,
        "retryPolicy": retry_policy,
        "deadLetter": !supported
    });

    TriggerDeliveryPlan {
        status,
        trace_id: Some(input.event_id),
        error_message,
        retry_policy,
        route_snapshot,
    }
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Trigger Router",
        "ai-foundation",
        "Webhook, schedule, plugin event, connector event, idempotency, retry, and routing boundaries.",
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerValidationError {
    MissingIdempotencyKey,
    IdempotencyKeyTooLong,
}

impl std::fmt::Display for TriggerValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingIdempotencyKey => formatter.write_str("idempotency key is required"),
            Self::IdempotencyKeyTooLong => formatter.write_str("idempotency key is too long"),
        }
    }
}

impl std::error::Error for TriggerValidationError {}

pub fn webhook_signature(secret: &str, body: &[u8]) -> String {
    let mut mac = <HmacSha256 as hmac::Mac>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts keys of any size");
    hmac::Mac::update(&mut mac, body);
    format!(
        "{WEBHOOK_SIGNATURE_PREFIX}{}",
        hex_encode(&hmac::Mac::finalize(mac).into_bytes())
    )
}

pub fn verify_webhook_signature(secret: &str, body: &[u8], provided: &str) -> bool {
    let provided = provided.trim();
    let digest = provided
        .strip_prefix(WEBHOOK_SIGNATURE_PREFIX)
        .unwrap_or(provided);
    let Some(bytes) = hex_decode(digest) else {
        return false;
    };
    let Ok(mut mac) = <HmacSha256 as hmac::Mac>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    hmac::Mac::update(&mut mac, body);
    hmac::Mac::verify_slice(mac, &bytes).is_ok()
}

pub fn normalize_idempotency_key(raw: &str) -> Result<String, TriggerValidationError> {
    let key = raw.trim();
    if key.is_empty() {
        return Err(TriggerValidationError::MissingIdempotencyKey);
    }
    if key.chars().count() > MAX_IDEMPOTENCY_KEY_CHARS {
        return Err(TriggerValidationError::IdempotencyKeyTooLong);
    }
    Ok(key.to_owned())
}

fn hex_encode(bytes: &[u8]) -> String {
    const CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(CHARS[(byte >> 4) as usize] as char);
        encoded.push(CHARS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0])?;
            let low = hex_nibble(pair[1])?;
            Some((high << 4) | low)
        })
        .collect()
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::FoundationStatus;

    #[test]
    fn module_describes_trigger_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-trigger");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }

    #[test]
    fn webhook_signature_uses_sha256_hmac_prefix() {
        let signature = webhook_signature("top-secret", br#"{"event":"training.completed"}"#);

        assert!(signature.starts_with("sha256="));
        assert!(verify_webhook_signature(
            "top-secret",
            br#"{"event":"training.completed"}"#,
            &signature
        ));
        assert!(!verify_webhook_signature(
            "top-secret",
            br#"{"event":"training.changed"}"#,
            &signature
        ));
    }

    #[test]
    fn idempotency_key_is_required_and_bounded() {
        assert_eq!(
            normalize_idempotency_key("  tenant-1:event-1  ").unwrap(),
            "tenant-1:event-1"
        );
        assert!(matches!(
            normalize_idempotency_key("   "),
            Err(TriggerValidationError::MissingIdempotencyKey)
        ));
        assert!(matches!(
            normalize_idempotency_key(&"x".repeat(129)),
            Err(TriggerValidationError::IdempotencyKeyTooLong)
        ));
    }

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
}
