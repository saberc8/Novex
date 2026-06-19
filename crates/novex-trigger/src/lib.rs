mod delivery;
mod module;
mod types;
mod webhook;

pub use delivery::{
    is_supported_target_kind, plan_trigger_delivery, TriggerDeliveryInput, TriggerDeliveryPlan,
    TriggerRetryPolicy, ACCEPTED_DELIVERY_STATUS, DEAD_LETTER_DELIVERY_STATUS,
};
pub use module::module;
pub use types::{TriggerSourceKind, TriggerTargetKind};
pub use webhook::{
    normalize_idempotency_key, verify_webhook_signature, webhook_signature, TriggerValidationError,
    WEBHOOK_SIGNATURE_PREFIX,
};

pub const CRATE_ID: &str = "novex-trigger";
