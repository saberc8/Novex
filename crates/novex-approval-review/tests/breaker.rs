use novex_approval_review::GuardianRejectionCircuitBreaker;

#[test]
fn guardian_denial_breaker_interrupts_after_three_consecutive_denials() {
    let mut breaker = GuardianRejectionCircuitBreaker::default();

    assert!(!breaker.record_denial());
    assert!(!breaker.record_denial());
    assert!(breaker.record_denial());
}

#[test]
fn guardian_denial_breaker_counts_recent_denials_in_window() {
    let mut breaker = GuardianRejectionCircuitBreaker::default();

    for _ in 0..9 {
        assert!(!breaker.record_denial());
        breaker.record_non_denial();
    }

    assert!(breaker.record_denial());
}

#[test]
fn guardian_denial_breaker_non_denial_resets_consecutive_denials() {
    let mut breaker = GuardianRejectionCircuitBreaker::default();

    assert!(!breaker.record_denial());
    assert!(!breaker.record_denial());
    breaker.record_non_denial();

    assert!(!breaker.record_denial());
    assert!(!breaker.record_denial());
}
