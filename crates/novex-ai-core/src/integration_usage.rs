use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};

pub const INTEGRATION_QPS_RESOURCE: &str = "external_integration.qps";
pub const INTEGRATION_QUOTA_RESOURCE: &str = "external_integration.quota";
pub const INTEGRATION_USAGE_UNIT: &str = "request";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationPrincipalType {
    ApiKey,
    PublicLink,
}

impl IntegrationPrincipalType {
    pub const fn scope_type(self) -> &'static str {
        match self {
            Self::ApiKey => "api_key",
            Self::PublicLink => "public_link",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationUsageSubject {
    pub tenant_id: i64,
    pub scope_type: String,
    pub scope_id: String,
    pub qps_limit: i32,
    pub quota_limit: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationUsageWindow {
    pub resource_type: String,
    pub usage_unit: String,
    pub window_start: NaiveDateTime,
    pub window_end: NaiveDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationUsageLimitError {
    InvalidLimit,
    QpsExceeded,
    QuotaExceeded,
}

pub fn build_integration_usage_subject(
    principal_type: IntegrationPrincipalType,
    tenant_id: i64,
    credential_id: impl Into<String>,
    qps_limit: i32,
    quota_limit: i64,
) -> Result<IntegrationUsageSubject, IntegrationUsageLimitError> {
    if qps_limit <= 0 || quota_limit <= 0 {
        return Err(IntegrationUsageLimitError::InvalidLimit);
    }

    Ok(IntegrationUsageSubject {
        tenant_id,
        scope_type: principal_type.scope_type().to_owned(),
        scope_id: credential_id.into(),
        qps_limit,
        quota_limit,
    })
}

pub fn integration_usage_windows(now: NaiveDateTime) -> Vec<IntegrationUsageWindow> {
    let second_start = now
        .with_nanosecond(0)
        .expect("zero nanosecond is valid for NaiveDateTime");
    let month_start = NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
        .expect("current year and month form a valid date")
        .and_hms_opt(0, 0, 0)
        .expect("midnight is a valid time");
    let (next_year, next_month) = if now.month() == 12 {
        (now.year() + 1, 1)
    } else {
        (now.year(), now.month() + 1)
    };
    let month_end = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .expect("next year and month form a valid date")
        .and_hms_opt(0, 0, 0)
        .expect("midnight is a valid time");

    vec![
        IntegrationUsageWindow {
            resource_type: INTEGRATION_QPS_RESOURCE.to_owned(),
            usage_unit: INTEGRATION_USAGE_UNIT.to_owned(),
            window_start: second_start,
            window_end: second_start + Duration::seconds(1),
        },
        IntegrationUsageWindow {
            resource_type: INTEGRATION_QUOTA_RESOURCE.to_owned(),
            usage_unit: INTEGRATION_USAGE_UNIT.to_owned(),
            window_start: month_start,
            window_end: month_end,
        },
    ]
}

pub fn enforce_integration_usage_limits(
    subject: &IntegrationUsageSubject,
    qps_usage: i64,
    quota_usage: i64,
) -> Result<(), IntegrationUsageLimitError> {
    if subject.qps_limit <= 0 || subject.quota_limit <= 0 {
        return Err(IntegrationUsageLimitError::InvalidLimit);
    }
    if qps_usage > i64::from(subject.qps_limit) {
        return Err(IntegrationUsageLimitError::QpsExceeded);
    }
    if quota_usage > subject.quota_limit {
        return Err(IntegrationUsageLimitError::QuotaExceeded);
    }
    Ok(())
}
