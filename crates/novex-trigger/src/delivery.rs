use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const ACCEPTED_DELIVERY_STATUS: &str = "accepted";
pub const DEAD_LETTER_DELIVERY_STATUS: &str = "dead_letter";

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
