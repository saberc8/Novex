use novex_ai_core::{
    build_integration_usage_subject, enforce_integration_usage_limits, integration_usage_windows,
    IntegrationPrincipalType, IntegrationUsageLimitError, IntegrationUsageSubject,
    INTEGRATION_QPS_RESOURCE, INTEGRATION_QUOTA_RESOURCE, INTEGRATION_USAGE_UNIT,
};

#[test]
fn integration_usage_subject_binds_principal_to_meter_scope() {
    assert_eq!(
        build_integration_usage_subject(IntegrationPrincipalType::ApiKey, 11, "42", 2, 5).unwrap(),
        IntegrationUsageSubject {
            tenant_id: 11,
            scope_type: "api_key".to_owned(),
            scope_id: "42".to_owned(),
            qps_limit: 2,
            quota_limit: 5,
        }
    );
    assert_eq!(
        build_integration_usage_subject(IntegrationPrincipalType::PublicLink, 11, "43", 3, 8)
            .unwrap()
            .scope_type,
        "public_link"
    );
}

#[test]
fn integration_usage_windows_cover_second_qps_and_monthly_quota() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-06-06T08:09:10Z")
        .unwrap()
        .naive_utc();
    let windows = integration_usage_windows(now);

    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0].resource_type, INTEGRATION_QPS_RESOURCE);
    assert_eq!(windows[0].usage_unit, INTEGRATION_USAGE_UNIT);
    assert_eq!(windows[0].window_start, now);
    assert_eq!(
        windows[0].window_end,
        chrono::DateTime::parse_from_rfc3339("2026-06-06T08:09:11Z")
            .unwrap()
            .naive_utc()
    );
    assert_eq!(windows[1].resource_type, INTEGRATION_QUOTA_RESOURCE);
    assert_eq!(
        windows[1].window_start,
        chrono::DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z")
            .unwrap()
            .naive_utc()
    );
    assert_eq!(
        windows[1].window_end,
        chrono::DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
            .unwrap()
            .naive_utc()
    );
}

#[test]
fn integration_usage_limits_allow_boundary_and_reject_excess() {
    let subject = IntegrationUsageSubject {
        tenant_id: 11,
        scope_type: "api_key".to_owned(),
        scope_id: "42".to_owned(),
        qps_limit: 2,
        quota_limit: 5,
    };

    assert!(enforce_integration_usage_limits(&subject, 2, 5).is_ok());
    assert_eq!(
        enforce_integration_usage_limits(&subject, 3, 5).unwrap_err(),
        IntegrationUsageLimitError::QpsExceeded
    );
    assert_eq!(
        enforce_integration_usage_limits(&subject, 2, 6).unwrap_err(),
        IntegrationUsageLimitError::QuotaExceeded
    );
}
