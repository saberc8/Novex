use chrono::Utc;
use novex_trigger::{TriggerDeliveryInput, TriggerRetryPolicy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

use crate::{
    application::system::format_datetime,
    infrastructure::persistence::ai_capability_repository::{
        AiCapabilityRepository, TriggerEventFilter, TriggerEventListRecord, TriggerEventSaveRecord,
        TriggerLookupRecord,
    },
    shared::{
        error::AppError,
        id::next_id,
        pagination::{PageQuery, PageResult, DEFAULT_PAGE},
    },
};

const WEBHOOK_SOURCE_TYPE: &str = "webhook";
const DEFAULT_TENANT_ID: i64 = 1;
const DEFAULT_TRIGGER_EVENT_PAGE_SIZE: u64 = 10;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerEventQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_trigger_event_size")]
    pub size: u64,
    #[serde(default)]
    pub trigger_code: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

impl Default for TriggerEventQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            size: DEFAULT_TRIGGER_EVENT_PAGE_SIZE,
            trigger_code: None,
            status: None,
        }
    }
}

impl TriggerEventQuery {
    pub fn page_query(&self) -> PageQuery {
        PageQuery {
            page: self.page,
            size: self.size,
        }
        .normalized()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerWebhookCommand {
    pub trigger_code: String,
    pub signature: String,
    pub idempotency_key: String,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedTriggerWebhookCommand {
    pub trigger_code: String,
    pub signature: String,
    pub idempotency_key: String,
    pub body: Vec<u8>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TriggerWebhookOutcome {
    pub event_id: i64,
    pub trigger_code: String,
    pub target_kind: String,
    pub idempotency_key: String,
    pub status: String,
    pub trace_id: Option<i64>,
    pub duplicate: bool,
    pub route_snapshot: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerWebhookResp {
    pub event_id: i64,
    pub trigger_code: String,
    pub target_kind: String,
    pub idempotency_key: String,
    pub status: String,
    pub trace_id: Option<i64>,
    pub duplicate: bool,
    pub route_snapshot: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerEventResp {
    pub id: i64,
    pub trigger_code: String,
    pub source_type: String,
    pub target_kind: String,
    pub idempotency_key: String,
    pub event_payload: Value,
    pub route_snapshot: Value,
    pub status: String,
    pub trace_id: Option<i64>,
    pub error_message: Option<String>,
    pub create_time: String,
}

#[derive(Debug, Clone)]
pub struct TriggerService {
    tenant_id: i64,
    repo: AiCapabilityRepository,
}

impl TriggerService {
    pub fn new(db: PgPool) -> Self {
        Self::for_tenant(db, DEFAULT_TENANT_ID)
    }

    pub fn for_tenant(db: PgPool, tenant_id: i64) -> Self {
        Self {
            tenant_id,
            repo: AiCapabilityRepository::new(db),
        }
    }

    pub async fn receive_webhook(
        &self,
        command: TriggerWebhookCommand,
    ) -> Result<TriggerWebhookResp, AppError> {
        let command = normalize_trigger_webhook_command(command)?;
        let Some(trigger) = self
            .repo
            .find_webhook_trigger_by_public_key(&command.trigger_code)
            .await?
        else {
            return Err(AppError::NotFound);
        };
        let secret = resolve_signature_secret(&trigger)?;
        verify_trigger_webhook_signature(&secret, &command.body, &command.signature)?;

        let event_id = next_id();
        let delivery_plan = build_trigger_delivery_plan(&trigger, event_id);
        let outcome = self
            .repo
            .create_trigger_event(&TriggerEventSaveRecord {
                id: event_id,
                tenant_id: trigger.tenant_id,
                trigger_id: trigger.id,
                trigger_code: trigger.code.clone(),
                source_type: WEBHOOK_SOURCE_TYPE.to_owned(),
                target_kind: trigger.target_kind.clone(),
                idempotency_key: command.idempotency_key,
                signature_header: command.signature,
                event_payload: command.payload,
                route_snapshot: delivery_plan.route_snapshot,
                status: delivery_plan.status,
                trace_id: delivery_plan.trace_id,
                error_message: delivery_plan.error_message,
                user_id: 1,
                now: Utc::now().naive_utc(),
            })
            .await?;

        Ok(trigger_webhook_response(TriggerWebhookOutcome {
            event_id: outcome.record.id,
            trigger_code: outcome.record.trigger_code,
            target_kind: outcome.record.target_kind,
            idempotency_key: outcome.record.idempotency_key,
            status: if outcome.duplicate {
                "duplicate".to_owned()
            } else {
                outcome.record.status
            },
            trace_id: outcome.record.trace_id,
            duplicate: outcome.duplicate,
            route_snapshot: outcome.record.route_snapshot,
        }))
    }

    pub async fn list_events(
        &self,
        query: TriggerEventQuery,
    ) -> Result<PageResult<TriggerEventResp>, AppError> {
        let page = query.page_query();
        let trigger_code = query
            .trigger_code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let status = query
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let filter = TriggerEventFilter {
            tenant_id: self.tenant_id,
            trigger_code,
            status,
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_trigger_events(&filter).await?;
        let list = self
            .repo
            .list_trigger_events(&filter)
            .await?
            .into_iter()
            .map(TriggerEventResp::from)
            .collect();

        Ok(PageResult::new(list, total))
    }
}

impl From<TriggerWebhookResp> for TriggerWebhookOutcome {
    fn from(resp: TriggerWebhookResp) -> Self {
        Self {
            event_id: resp.event_id,
            trigger_code: resp.trigger_code,
            target_kind: resp.target_kind,
            idempotency_key: resp.idempotency_key,
            status: resp.status,
            trace_id: resp.trace_id,
            duplicate: resp.duplicate,
            route_snapshot: resp.route_snapshot,
        }
    }
}

pub fn normalize_trigger_webhook_command(
    command: TriggerWebhookCommand,
) -> Result<NormalizedTriggerWebhookCommand, AppError> {
    let trigger_code = command.trigger_code.trim().to_owned();
    if trigger_code.is_empty() {
        return Err(AppError::bad_request("触发器编码不能为空"));
    }
    let signature = command.signature.trim().to_owned();
    if signature.is_empty() {
        return Err(AppError::bad_request("Webhook 签名不能为空"));
    }
    let idempotency_key = novex_trigger::normalize_idempotency_key(&command.idempotency_key)
        .map_err(|_| AppError::bad_request("Webhook 幂等键不能为空"))?;
    if command.body.is_empty() {
        return Err(AppError::bad_request("Webhook 请求体不能为空"));
    }
    let payload = serde_json::from_slice::<Value>(&command.body)
        .map_err(|_| AppError::bad_request("Webhook 请求体必须是 JSON"))?;

    Ok(NormalizedTriggerWebhookCommand {
        trigger_code,
        signature,
        idempotency_key,
        body: command.body,
        payload,
    })
}

pub fn verify_trigger_webhook_signature(
    secret: &str,
    body: &[u8],
    signature: &str,
) -> Result<(), AppError> {
    if novex_trigger::verify_webhook_signature(secret, body, signature) {
        Ok(())
    } else {
        Err(AppError::bad_request("Webhook 签名无效"))
    }
}

pub fn trigger_webhook_response(outcome: TriggerWebhookOutcome) -> TriggerWebhookResp {
    TriggerWebhookResp {
        event_id: outcome.event_id,
        trigger_code: outcome.trigger_code,
        target_kind: outcome.target_kind,
        idempotency_key: outcome.idempotency_key,
        status: outcome.status,
        trace_id: outcome.trace_id,
        duplicate: outcome.duplicate,
        route_snapshot: outcome.route_snapshot,
    }
}

fn resolve_signature_secret(trigger: &TriggerLookupRecord) -> Result<String, AppError> {
    let column_secret_ref = trigger
        .signature_secret_ref
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let secret_ref = column_secret_ref
        .or_else(|| {
            trigger.route_config["signatureSecretRef"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .ok_or_else(|| AppError::bad_request("Webhook 签名密钥未配置"))?;

    if let Some(env_name) = secret_ref.strip_prefix("env:") {
        return std::env::var(env_name)
            .map_err(|_| AppError::bad_request("Webhook 签名密钥未配置"));
    }

    if let Some(value) = secret_ref.strip_prefix("literal:") {
        if value.is_empty() {
            return Err(AppError::bad_request("Webhook 签名密钥未配置"));
        }
        return Ok(value.to_owned());
    }

    Err(AppError::bad_request("Webhook 签名密钥引用不受支持"))
}

fn build_trigger_delivery_plan(
    trigger: &TriggerLookupRecord,
    event_id: i64,
) -> novex_trigger::TriggerDeliveryPlan {
    novex_trigger::plan_trigger_delivery(TriggerDeliveryInput {
        trigger_id: trigger.id,
        trigger_code: trigger.code.clone(),
        target_kind: trigger.target_kind.clone(),
        route_config: trigger.route_config.clone(),
        event_id,
        retry_policy: TriggerRetryPolicy::default(),
    })
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_trigger_event_size() -> u64 {
    DEFAULT_TRIGGER_EVENT_PAGE_SIZE
}

impl From<TriggerEventListRecord> for TriggerEventResp {
    fn from(record: TriggerEventListRecord) -> Self {
        Self {
            id: record.id,
            trigger_code: record.trigger_code,
            source_type: record.source_type,
            target_kind: record.target_kind,
            idempotency_key: record.idempotency_key,
            event_payload: record.event_payload,
            route_snapshot: record.route_snapshot,
            status: record.status,
            trace_id: record.trace_id,
            error_message: record.error_message,
            create_time: format_datetime(record.create_time),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn webhook_lookup_resolves_tenant_from_public_trigger_key() {
        let source = include_str!("trigger_service.rs");
        let public_lookup = ["find_webhook_trigger", "by_public_key"].join("_");
        let default_tenant = ["DEFAULT", "TENANT", "ID"].join("_");
        let default_lookup = format!("find_webhook_trigger({default_tenant}");

        assert!(source.contains(&format!(".{public_lookup}(&command.trigger_code)")));
        assert!(!source.contains(&default_lookup));
    }

    #[test]
    fn webhook_command_normalizes_headers_and_payload() {
        let body = br#"{"event":"training.completed","employeeId":7}"#.to_vec();
        let command = normalize_trigger_webhook_command(TriggerWebhookCommand {
            trigger_code: " training ".to_owned(),
            signature: " sha256=abc ".to_owned(),
            idempotency_key: " tenant-1:event-7 ".to_owned(),
            body,
        })
        .unwrap();

        assert_eq!(command.trigger_code, "training");
        assert_eq!(command.signature, "sha256=abc");
        assert_eq!(command.idempotency_key, "tenant-1:event-7");
        assert_eq!(command.payload["event"], "training.completed");
    }

    #[test]
    fn webhook_command_rejects_invalid_signature_and_idempotency_inputs() {
        let err = normalize_trigger_webhook_command(TriggerWebhookCommand {
            trigger_code: "training".to_owned(),
            signature: "   ".to_owned(),
            idempotency_key: "event-1".to_owned(),
            body: br#"{"event":"training.completed"}"#.to_vec(),
        })
        .unwrap_err();

        assert!(err.to_string().contains("签名"));

        let err = normalize_trigger_webhook_command(TriggerWebhookCommand {
            trigger_code: "training".to_owned(),
            signature: "sha256=abc".to_owned(),
            idempotency_key: " ".to_owned(),
            body: br#"{"event":"training.completed"}"#.to_vec(),
        })
        .unwrap_err();

        assert!(err.to_string().contains("幂等"));
    }

    #[test]
    fn webhook_signature_validation_uses_trigger_secret() {
        let body = br#"{"event":"training.completed"}"#;
        let signature = novex_trigger::webhook_signature("secret-1", body);

        assert!(verify_trigger_webhook_signature("secret-1", body, &signature).is_ok());
        assert!(verify_trigger_webhook_signature("secret-2", body, &signature).is_err());
    }

    #[test]
    fn webhook_secret_ref_falls_back_to_route_config() {
        let trigger =
            crate::infrastructure::persistence::ai_capability_repository::TriggerLookupRecord {
                id: 1,
                tenant_id: 1,
                code: "webhook.training.event".to_owned(),
                target_kind: "run_graph".to_owned(),
                signature_secret_ref: Some("   ".to_owned()),
                route_config: json!({"signatureSecretRef":"literal:test-secret"}),
            };

        assert_eq!(resolve_signature_secret(&trigger).unwrap(), "test-secret");
    }

    #[test]
    fn webhook_response_marks_duplicate_events() {
        let resp = trigger_webhook_response(TriggerWebhookOutcome {
            event_id: 10,
            trigger_code: "webhook.training.event".to_owned(),
            target_kind: "run_graph".to_owned(),
            idempotency_key: "tenant-1:event-7".to_owned(),
            status: "accepted".to_owned(),
            trace_id: Some(10),
            duplicate: false,
            route_snapshot: json!({"targetKind":"run_graph"}),
        });
        assert!(!resp.duplicate);

        let duplicate = trigger_webhook_response(TriggerWebhookOutcome {
            duplicate: true,
            status: "duplicate".to_owned(),
            ..TriggerWebhookOutcome::from(resp)
        });

        assert!(duplicate.duplicate);
        assert_eq!(duplicate.status, "duplicate");
    }

    #[test]
    fn webhook_delivery_plan_dead_letters_unsupported_target_kind() {
        let trigger =
            crate::infrastructure::persistence::ai_capability_repository::TriggerLookupRecord {
                id: 1,
                tenant_id: 1,
                code: "webhook.training.event".to_owned(),
                target_kind: "flow_builder".to_owned(),
                signature_secret_ref: Some("literal:test-secret".to_owned()),
                route_config: json!({"path":"/ai/triggers/webhook/training"}),
            };

        let plan = build_trigger_delivery_plan(&trigger, 9001);

        assert_eq!(plan.status, "dead_letter");
        assert_eq!(plan.trace_id, Some(9001));
        assert_eq!(plan.retry_policy.max_attempts, 0);
        assert!(plan
            .error_message
            .as_deref()
            .unwrap()
            .contains("unsupported trigger target kind"));
        assert_eq!(plan.route_snapshot["deliveryStatus"], "dead_letter");
        assert_eq!(plan.route_snapshot["deadLetter"], true);
    }
}
