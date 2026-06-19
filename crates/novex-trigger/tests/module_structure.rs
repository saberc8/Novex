use std::fs;
use std::path::Path;

use novex_ai_core::FoundationStatus;
use novex_trigger::{
    is_supported_target_kind, module, normalize_idempotency_key, plan_trigger_delivery,
    verify_webhook_signature, webhook_signature, TriggerDeliveryInput, TriggerRetryPolicy,
    TriggerSourceKind, ACCEPTED_DELIVERY_STATUS, WEBHOOK_SIGNATURE_PREFIX,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_trigger_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["delivery", "module", "types", "webhook"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum TriggerSourceKind",
        "pub struct TriggerRetryPolicy",
        "pub fn plan_trigger_delivery",
        "pub fn webhook_signature",
        "pub fn normalize_idempotency_key",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn trigger_domain_modules_exist() {
    for module in [
        "src/delivery.rs",
        "src/module.rs",
        "src/types.rs",
        "src/webhook.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_trigger_contracts() {
    let module = module();
    assert_eq!(module.id, "novex-trigger");
    assert_eq!(module.status, FoundationStatus::Skeleton);
    assert_eq!(
        serde_json::to_value(TriggerSourceKind::PluginEvent).unwrap(),
        serde_json::json!("plugin_event")
    );

    assert!(is_supported_target_kind("agent_run"));
    let plan = plan_trigger_delivery(TriggerDeliveryInput {
        trigger_id: 7,
        trigger_code: "webhook.training.event".to_owned(),
        target_kind: "agent_run".to_owned(),
        route_config: serde_json::json!({"agentCode":"training-assistant"}),
        event_id: 11,
        retry_policy: TriggerRetryPolicy::default(),
    });
    assert_eq!(plan.status, ACCEPTED_DELIVERY_STATUS);
    assert_eq!(plan.trace_id, Some(11));

    let signature = webhook_signature("top-secret", br#"{"event":"x"}"#);
    assert!(signature.starts_with(WEBHOOK_SIGNATURE_PREFIX));
    assert!(verify_webhook_signature(
        "top-secret",
        br#"{"event":"x"}"#,
        &signature
    ));
    assert_eq!(
        normalize_idempotency_key(" tenant:event ").unwrap(),
        "tenant:event"
    );
}
