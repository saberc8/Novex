use novex_trigger::{
    normalize_idempotency_key, verify_webhook_signature, webhook_signature, TriggerValidationError,
};

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
