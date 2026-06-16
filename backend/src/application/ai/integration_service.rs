use std::collections::HashMap;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, NaiveDateTime, Utc};
use novex_ai_core::{
    build_integration_usage_subject, enforce_integration_usage_limits, integration_usage_windows,
    IntegrationPrincipalType, IntegrationUsageLimitError, IntegrationUsageSubject, TaskBudget,
    INTEGRATION_QPS_RESOURCE, INTEGRATION_QUOTA_RESOURCE,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::{
    application::{
        ai::agent_service::{
            normalize_agent_run_command, AgentRunCommand, AgentRunResp, AgentService,
        },
        ai::knowledge_service::{
            normalize_rag_ask_command, CitationResp, KnowledgeService, RagAskCommand, RagAskResp,
        },
        ai::model_service::{
            ModelChatCommand, ModelChatFileContext, ModelChatMessage, ModelChatResp,
            ModelChatUsage, ModelRuntimeService,
        },
        system::ensure_max_chars,
    },
    infrastructure::persistence::ai_integration_repository::{
        AiIntegrationRepository, ApiKeyRecord, ApiKeySaveRecord, IntegrationFilter,
        PublicLinkRecord, PublicLinkSaveRecord, RuntimeApiKeyRecord, RuntimePublicLinkRecord,
        UsageMeterIncrementRecord, UsageMeterSummaryFilter, UsageMeterSummaryRecord,
    },
    shared::{
        error::AppError,
        id::next_id,
        pagination::{PageQuery, PageResult, DEFAULT_PAGE},
    },
};

const DEFAULT_PAGE_SIZE: u64 = 20;
const DEFAULT_STATUS: i16 = 1;
const API_KEY_PREFIX: &str = "nxk_live";
const PUBLIC_LINK_PREFIX: &str = "nxl";
const DEFAULT_PUBLIC_ORIGIN: &str = "https://public.novex.local";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_size")]
    pub size: u64,
    #[serde(default)]
    pub app_id: Option<String>,
    #[serde(default = "default_enabled_status")]
    pub status: Option<i16>,
}

impl Default for IntegrationQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            size: DEFAULT_PAGE_SIZE,
            app_id: None,
            status: Some(DEFAULT_STATUS),
        }
    }
}

impl IntegrationQuery {
    fn page_query(&self) -> PageQuery {
        PageQuery {
            page: self.page,
            size: self.size,
        }
        .normalized()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyCommand {
    pub app_id: String,
    pub name: String,
    #[serde(default)]
    pub permission_scope: Vec<String>,
    pub qps_limit: i32,
    pub quota_limit: i64,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyResp {
    pub id: i64,
    pub app_id: String,
    pub name: String,
    pub key_prefix: String,
    pub masked_key: String,
    pub permission_scope: Vec<String>,
    pub qps_limit: i32,
    pub quota_limit: i64,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub usage_summary: IntegrationUsageSummaryResp,
    pub status: i16,
    pub create_time: String,
    pub update_time: Option<String>,
    pub plain_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicLinkCommand {
    pub app_id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub permission_scope: Vec<String>,
    pub qps_limit: i32,
    pub quota_limit: i64,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicLinkResp {
    pub id: i64,
    pub app_id: String,
    pub name: String,
    pub path: String,
    pub public_url: String,
    pub masked_token: String,
    pub permission_scope: Vec<String>,
    pub qps_limit: i32,
    pub quota_limit: i64,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub usage_summary: IntegrationUsageSummaryResp,
    pub status: i16,
    pub create_time: String,
    pub update_time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationUsageSummaryResp {
    pub qps_used: i64,
    pub qps_limit: i32,
    pub quota_used: i64,
    pub quota_limit: i64,
    pub qps_window_start: Option<String>,
    pub quota_window_start: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenApiInvokeCommand {
    pub app_id: String,
    pub operation: String,
    #[serde(default)]
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationRuntimeContext {
    pub principal_type: String,
    pub tenant_id: i64,
    pub app_id: String,
    pub name: String,
    pub path: Option<String>,
    pub masked_credential: String,
    pub permission_scope: Vec<String>,
    pub qps_limit: i32,
    pub quota_limit: i64,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenApiInvokeResp {
    pub accepted: bool,
    pub operation: String,
    pub required_permission: String,
    pub input: serde_json::Value,
    pub auth: IntegrationRuntimeContext,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<CitationResp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieval_hit_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer_strategy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ModelChatUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_tool_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pause_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_budget: Option<TaskBudget>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicShareResp {
    pub accepted: bool,
    pub target_path: String,
    pub auth: IntegrationRuntimeContext,
}

#[derive(Debug, Clone)]
pub struct IntegrationService {
    db: PgPool,
    repo: AiIntegrationRepository,
}

#[derive(Debug, Clone)]
struct AuthenticatedApiKey {
    user_id: i64,
    context: IntegrationRuntimeContext,
}

#[derive(Debug, Clone)]
struct TrainingAskInput {
    dataset_id: i64,
    command: RagAskCommand,
}

#[derive(Debug, Clone, Default)]
struct RuntimeUsageSnapshot {
    qps_used: i64,
    quota_used: i64,
    qps_window_start: Option<NaiveDateTime>,
    quota_window_start: Option<NaiveDateTime>,
}

impl IntegrationService {
    pub fn new(db: PgPool) -> Self {
        Self {
            repo: AiIntegrationRepository::new(db.clone()),
            db,
        }
    }

    pub async fn list_api_keys(
        &self,
        tenant_id: i64,
        query: IntegrationQuery,
    ) -> Result<PageResult<ApiKeyResp>, AppError> {
        let page = query.page_query();
        let filter = IntegrationFilter {
            tenant_id,
            app_id: query.app_id.as_deref(),
            status: query.status,
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_api_keys(&filter).await?;
        let records = self.repo.list_api_keys(&filter).await?;
        let usage_by_id = self
            .usage_snapshots_for_entries(
                tenant_id,
                IntegrationPrincipalType::ApiKey,
                records.iter().map(|record| record.id).collect(),
            )
            .await?;
        let list = records
            .into_iter()
            .map(|record| {
                let usage = usage_by_id.get(&record.id).cloned();
                api_key_response_with_usage(record, None, usage)
            })
            .collect();
        Ok(PageResult::new(list, total))
    }

    pub async fn create_api_key(
        &self,
        tenant_id: i64,
        user_id: i64,
        command: ApiKeyCommand,
    ) -> Result<ApiKeyResp, AppError> {
        let command = normalize_api_key_command(command)?;
        let plain_key = generate_secret(API_KEY_PREFIX);
        let now = Utc::now().naive_utc();
        let record = self
            .repo
            .create_api_key(&ApiKeySaveRecord {
                id: next_id(),
                tenant_id,
                app_id: command.app_id,
                name: command.name,
                key_prefix: API_KEY_PREFIX.to_owned(),
                key_hash: sha256_hex(&plain_key),
                masked_key: mask_secret(&plain_key),
                permission_scope: json!(command.permission_scope),
                qps_limit: command.qps_limit,
                quota_limit: command.quota_limit,
                expires_at: parse_optional_datetime(command.expires_at)?,
                metadata: json!({ "source": "admin-control-plane" }),
                user_id,
                now,
            })
            .await?;

        Ok(api_key_response(record, Some(plain_key)))
    }

    pub async fn revoke_api_key(
        &self,
        tenant_id: i64,
        user_id: i64,
        id: i64,
    ) -> Result<bool, AppError> {
        self.repo
            .revoke_api_key(tenant_id, id, user_id, Utc::now().naive_utc())
            .await
    }

    pub async fn list_public_links(
        &self,
        tenant_id: i64,
        query: IntegrationQuery,
    ) -> Result<PageResult<PublicLinkResp>, AppError> {
        let page = query.page_query();
        let filter = IntegrationFilter {
            tenant_id,
            app_id: query.app_id.as_deref(),
            status: query.status,
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_public_links(&filter).await?;
        let records = self.repo.list_public_links(&filter).await?;
        let usage_by_id = self
            .usage_snapshots_for_entries(
                tenant_id,
                IntegrationPrincipalType::PublicLink,
                records.iter().map(|record| record.id).collect(),
            )
            .await?;
        let list = records
            .into_iter()
            .map(|record| {
                let usage = usage_by_id.get(&record.id).cloned();
                public_link_response_with_usage(record, usage)
            })
            .collect();
        Ok(PageResult::new(list, total))
    }

    pub async fn create_public_link(
        &self,
        tenant_id: i64,
        user_id: i64,
        command: PublicLinkCommand,
    ) -> Result<PublicLinkResp, AppError> {
        let command = normalize_public_link_command(command)?;
        let token = generate_secret(PUBLIC_LINK_PREFIX);
        let public_url = public_url_for_token(&token);
        let now = Utc::now().naive_utc();
        let record = self
            .repo
            .create_public_link(&PublicLinkSaveRecord {
                id: next_id(),
                tenant_id,
                app_id: command.app_id,
                name: command.name,
                path: command.path,
                token_hash: sha256_hex(&token),
                masked_token: mask_secret(&token),
                public_url,
                permission_scope: json!(command.permission_scope),
                qps_limit: command.qps_limit,
                quota_limit: command.quota_limit,
                expires_at: parse_optional_datetime(command.expires_at)?,
                metadata: json!({ "source": "admin-control-plane" }),
                user_id,
                now,
            })
            .await?;

        Ok(public_link_response(record))
    }

    pub async fn revoke_public_link(
        &self,
        tenant_id: i64,
        user_id: i64,
        id: i64,
    ) -> Result<bool, AppError> {
        self.repo
            .revoke_public_link(tenant_id, id, user_id, Utc::now().naive_utc())
            .await
    }

    pub async fn invoke_openapi(
        &self,
        plain_key: &str,
        command: OpenApiInvokeCommand,
    ) -> Result<OpenApiInvokeResp, AppError> {
        let command = normalize_openapi_invoke_command(command)?;
        let required_permission = required_permission_for_operation(&command.operation)?;
        let authenticated = self
            .authenticate_api_key_record(plain_key, &command.app_id, &required_permission)
            .await?;
        let auth = authenticated.context;

        if command.operation == "training.ask" {
            let ask = normalize_training_ask_input(&command.input)?;
            let answer = KnowledgeService::new(self.db.clone())
                .ask_dataset_for_tenant(
                    auth.tenant_id,
                    authenticated.user_id,
                    ask.dataset_id,
                    ask.command,
                )
                .await?;

            return Ok(openapi_training_ask_response(
                command.operation,
                required_permission,
                command.input,
                auth,
                answer,
            ));
        }

        if command.operation == "chat.use" {
            let chat = normalize_openapi_chat_input(&command.input)?;
            let answer = ModelRuntimeService::for_tenant(self.db.clone(), auth.tenant_id)
                .chat_completion_for_source(authenticated.user_id, chat, "ai.openapi.chat")
                .await?;

            return Ok(openapi_chat_response(
                command.operation,
                required_permission,
                command.input,
                auth,
                answer,
            ));
        }

        if command.operation == "agent.run" {
            let agent = normalize_openapi_agent_run_input(&command.input)?;
            let run = AgentService::for_tenant(self.db.clone(), auth.tenant_id)
                .create_run(authenticated.user_id, agent)
                .await?;

            return Ok(openapi_agent_run_response(
                command.operation,
                required_permission,
                command.input,
                auth,
                run,
            ));
        }

        Ok(OpenApiInvokeResp {
            accepted: true,
            operation: command.operation,
            required_permission,
            input: command.input,
            auth,
            answer: None,
            citations: Vec::new(),
            trace_id: None,
            retrieval_hit_count: None,
            answer_strategy: None,
            route_id: None,
            model: None,
            usage: None,
            latency_ms: None,
            run_id: None,
            agent_trace_id: None,
            status: None,
            intent: None,
            loop_kind: None,
            selected_tool_code: None,
            pause_reason: None,
            final_output: None,
            task_budget: None,
        })
    }

    pub async fn authenticate_api_key(
        &self,
        plain_key: &str,
        app_id: &str,
        required_permission: &str,
    ) -> Result<IntegrationRuntimeContext, AppError> {
        Ok(self
            .authenticate_api_key_record(plain_key, app_id, required_permission)
            .await?
            .context)
    }

    async fn authenticate_api_key_record(
        &self,
        plain_key: &str,
        app_id: &str,
        required_permission: &str,
    ) -> Result<AuthenticatedApiKey, AppError> {
        let plain_key = normalize_runtime_secret(plain_key, API_KEY_PREFIX)?;
        let app_id = app_id.trim();
        if app_id.is_empty() {
            return Err(AppError::bad_request("应用标识不能为空"));
        }
        let now = Utc::now().naive_utc();
        let record = self
            .repo
            .find_runtime_api_key_by_hash(&sha256_hex(&plain_key), now)
            .await?
            .ok_or(AppError::Unauthorized)?;
        let id = record.id;
        let user_id = record.create_user;
        let context = runtime_context_from_api_key_record(record, app_id, required_permission)?;
        self.record_runtime_usage(&context, id, user_id, now)
            .await?;
        self.repo.touch_api_key_last_used(id, now).await?;
        Ok(AuthenticatedApiKey { user_id, context })
    }

    pub async fn resolve_public_share(&self, token: &str) -> Result<PublicShareResp, AppError> {
        let token = normalize_runtime_secret(token, PUBLIC_LINK_PREFIX)?;
        let now = Utc::now().naive_utc();
        let record = self
            .repo
            .find_runtime_public_link_by_token_hash(&sha256_hex(&token), now)
            .await?
            .ok_or(AppError::Unauthorized)?;
        let id = record.id;
        let user_id = record.create_user;
        let context = runtime_context_from_public_link_record(record)?;
        self.record_runtime_usage(&context, id, user_id, now)
            .await?;
        self.repo.touch_public_link_last_used(id, now).await?;
        let target_path = context.path.clone().unwrap_or_else(|| "/".to_owned());

        Ok(PublicShareResp {
            accepted: true,
            target_path,
            auth: context,
        })
    }

    async fn record_runtime_usage(
        &self,
        context: &IntegrationRuntimeContext,
        credential_id: i64,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        let subject = integration_usage_subject(context, credential_id)?;
        let windows = integration_usage_windows(now);
        let mut qps_usage = 0;
        let mut quota_usage = 0;

        for window in windows {
            let usage_value = self
                .repo
                .increment_usage_meter(&UsageMeterIncrementRecord {
                    id: next_id(),
                    tenant_id: subject.tenant_id,
                    scope_type: subject.scope_type.clone(),
                    scope_id: subject.scope_id.clone(),
                    resource_type: window.resource_type.clone(),
                    usage_unit: window.usage_unit.clone(),
                    window_start: window.window_start,
                    window_end: window.window_end,
                    metadata: json!({
                        "principalType": context.principal_type,
                        "appId": context.app_id,
                        "entryName": context.name,
                    }),
                    user_id,
                    now,
                })
                .await?;

            match window.resource_type.as_str() {
                INTEGRATION_QPS_RESOURCE => qps_usage = usage_value,
                INTEGRATION_QUOTA_RESOURCE => quota_usage = usage_value,
                _ => {}
            }
        }

        enforce_runtime_usage_limits(&subject, qps_usage, quota_usage)
    }

    async fn usage_snapshots_for_entries(
        &self,
        tenant_id: i64,
        principal_type: IntegrationPrincipalType,
        entry_ids: Vec<i64>,
    ) -> Result<HashMap<i64, RuntimeUsageSnapshot>, AppError> {
        if entry_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let windows = integration_usage_windows(Utc::now().naive_utc());
        let qps_window = windows
            .iter()
            .find(|window| window.resource_type == INTEGRATION_QPS_RESOURCE)
            .expect("integration usage windows include qps");
        let quota_window = windows
            .iter()
            .find(|window| window.resource_type == INTEGRATION_QUOTA_RESOURCE)
            .expect("integration usage windows include quota");
        let rows = self
            .repo
            .list_usage_meter_summaries(&UsageMeterSummaryFilter {
                tenant_id,
                scope_type: principal_type.scope_type().to_owned(),
                scope_ids: entry_ids.iter().map(ToString::to_string).collect(),
                qps_resource_type: qps_window.resource_type.clone(),
                qps_window_start: qps_window.window_start,
                qps_window_end: qps_window.window_end,
                quota_resource_type: quota_window.resource_type.clone(),
                quota_window_start: quota_window.window_start,
                quota_window_end: quota_window.window_end,
            })
            .await?;

        Ok(usage_snapshots_from_rows(rows))
    }
}

pub fn normalize_api_key_command(mut command: ApiKeyCommand) -> Result<ApiKeyCommand, AppError> {
    command.app_id = command.app_id.trim().to_owned();
    command.name = command.name.trim().to_owned();
    command.permission_scope = normalize_permission_scope(command.permission_scope)?;
    validate_common_fields(
        &command.app_id,
        &command.name,
        command.qps_limit,
        command.quota_limit,
        command.expires_at.as_deref(),
    )?;
    Ok(command)
}

pub fn normalize_public_link_command(
    mut command: PublicLinkCommand,
) -> Result<PublicLinkCommand, AppError> {
    command.app_id = command.app_id.trim().to_owned();
    command.name = command.name.trim().to_owned();
    command.path = normalize_public_path(&command.path)?;
    command.permission_scope = normalize_permission_scope(command.permission_scope)?;
    validate_common_fields(
        &command.app_id,
        &command.name,
        command.qps_limit,
        command.quota_limit,
        command.expires_at.as_deref(),
    )?;
    Ok(command)
}

pub fn normalize_openapi_invoke_command(
    mut command: OpenApiInvokeCommand,
) -> Result<OpenApiInvokeCommand, AppError> {
    command.app_id = command.app_id.trim().to_owned();
    command.operation = command.operation.trim().to_owned();
    if command.app_id.is_empty() {
        return Err(AppError::bad_request("应用标识不能为空"));
    }
    if command.operation.is_empty() {
        return Err(AppError::bad_request("OpenAPI operation不能为空"));
    }
    ensure_max_chars("应用标识", &command.app_id, 128)?;
    ensure_max_chars("OpenAPI operation", &command.operation, 128)?;
    Ok(command)
}

pub fn required_permission_for_operation(operation: &str) -> Result<String, AppError> {
    match operation.trim() {
        "training.ask" => Ok("app:training:ask".to_owned()),
        "chat.use" => Ok("app:chat:use".to_owned()),
        "agent.run" => Ok("ai:agent:run".to_owned()),
        _ => Err(AppError::bad_request("OpenAPI operation不支持")),
    }
}

fn normalize_training_ask_input(input: &serde_json::Value) -> Result<TrainingAskInput, AppError> {
    let object = input
        .as_object()
        .ok_or_else(|| AppError::bad_request("training.ask input 必须是对象"))?;
    let dataset_id = object
        .get("datasetId")
        .or_else(|| object.get("dataset_id"))
        .and_then(json_i64)
        .ok_or_else(|| AppError::bad_request("知识库 ID 不合法"))?;
    if dataset_id <= 0 {
        return Err(AppError::bad_request("知识库 ID 不合法"));
    }
    let question = object
        .get("question")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_owned();
    let limit = object.get("limit").and_then(json_usize).unwrap_or_default();
    let command = normalize_rag_ask_command(RagAskCommand {
        question,
        limit,
        ..RagAskCommand::default()
    })?;

    Ok(TrainingAskInput {
        dataset_id,
        command,
    })
}

fn normalize_openapi_chat_input(input: &Value) -> Result<ModelChatCommand, AppError> {
    let object = input
        .as_object()
        .ok_or_else(|| AppError::bad_request("chat.use input 必须是对象"))?;
    let messages = if let Some(messages) = object.get("messages") {
        openapi_chat_messages(messages)?
    } else {
        let content = object
            .get("question")
            .or_else(|| object.get("prompt"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned();
        if content.is_empty() {
            return Err(AppError::bad_request("chat.use question 不能为空"));
        }
        vec![ModelChatMessage {
            role: "user".to_owned(),
            content,
        }]
    };
    if messages.is_empty() {
        return Err(AppError::bad_request("chat.use messages 不能为空"));
    }

    Ok(ModelChatCommand {
        messages,
        file_contexts: openapi_chat_file_contexts(
            object
                .get("fileContexts")
                .or_else(|| object.get("file_contexts")),
        )?,
        temperature: object.get("temperature").and_then(json_f64),
        max_tokens: object
            .get("maxTokens")
            .or_else(|| object.get("max_tokens"))
            .and_then(json_u32),
        ..ModelChatCommand::default()
    })
}

fn openapi_chat_messages(value: &Value) -> Result<Vec<ModelChatMessage>, AppError> {
    let items = value
        .as_array()
        .ok_or_else(|| AppError::bad_request("chat.use messages 必须是数组"))?;
    if items.is_empty() {
        return Err(AppError::bad_request("chat.use messages 不能为空"));
    }

    items
        .iter()
        .map(|item| {
            let object = item
                .as_object()
                .ok_or_else(|| AppError::bad_request("chat.use message 必须是对象"))?;
            let role = object
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("user")
                .trim()
                .to_owned();
            let content = object
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_owned();
            if content.is_empty() {
                return Err(AppError::bad_request("chat.use message content 不能为空"));
            }
            Ok(ModelChatMessage {
                role: if role.is_empty() {
                    "user".to_owned()
                } else {
                    role
                },
                content,
            })
        })
        .collect()
}

fn openapi_chat_file_contexts(
    value: Option<&Value>,
) -> Result<Vec<ModelChatFileContext>, AppError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .ok_or_else(|| AppError::bad_request("chat.use fileContexts 必须是数组"))?;

    items
        .iter()
        .map(|item| {
            let object = item
                .as_object()
                .ok_or_else(|| AppError::bad_request("chat.use fileContext 必须是对象"))?;
            Ok(ModelChatFileContext {
                name: object
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned(),
                content_type: object
                    .get("contentType")
                    .or_else(|| object.get("content_type"))
                    .and_then(Value::as_str)
                    .unwrap_or("text/plain")
                    .trim()
                    .to_owned(),
                content: object
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned(),
            })
        })
        .collect()
}

fn normalize_openapi_agent_run_input(input: &Value) -> Result<AgentRunCommand, AppError> {
    let object = input
        .as_object()
        .ok_or_else(|| AppError::bad_request("agent.run input 必须是对象"))?;
    let mut command_value = Value::Object(object.clone());

    if let Some(fields) = command_value.as_object_mut() {
        if !fields.contains_key("input") {
            if let Some(alias) = fields
                .get("task")
                .or_else(|| fields.get("prompt"))
                .or_else(|| fields.get("question"))
                .cloned()
            {
                fields.insert("input".to_owned(), alias);
            }
        }
    }

    let command = serde_json::from_value::<AgentRunCommand>(command_value)
        .map_err(|_| AppError::bad_request("agent.run input 格式不合法"))?;
    normalize_agent_run_command(command)
}

fn json_i64(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_str()?.trim().parse::<i64>().ok())
}

fn json_u32(value: &serde_json::Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .or_else(|| value.as_i64().and_then(|value| u32::try_from(value).ok()))
        .or_else(|| value.as_str()?.trim().parse::<u32>().ok())
}

fn json_usize(value: &serde_json::Value) -> Option<usize> {
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .or_else(|| value.as_str()?.trim().parse::<usize>().ok())
}

fn json_f64(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite())
}

fn openapi_training_ask_response(
    operation: String,
    required_permission: String,
    input: serde_json::Value,
    auth: IntegrationRuntimeContext,
    answer: RagAskResp,
) -> OpenApiInvokeResp {
    OpenApiInvokeResp {
        accepted: true,
        operation,
        required_permission,
        input,
        auth,
        answer: Some(answer.answer),
        citations: answer.citations,
        trace_id: Some(answer.trace_id),
        retrieval_hit_count: Some(answer.retrieval_hit_count),
        answer_strategy: Some(answer.answer_strategy),
        route_id: Some(answer.answer_model_route),
        model: answer.answer_model,
        usage: None,
        latency_ms: None,
        run_id: None,
        agent_trace_id: None,
        status: None,
        intent: None,
        loop_kind: None,
        selected_tool_code: None,
        pause_reason: None,
        final_output: None,
        task_budget: None,
    }
}

fn openapi_chat_response(
    operation: String,
    required_permission: String,
    input: serde_json::Value,
    auth: IntegrationRuntimeContext,
    answer: ModelChatResp,
) -> OpenApiInvokeResp {
    OpenApiInvokeResp {
        accepted: true,
        operation,
        required_permission,
        input,
        auth,
        answer: Some(answer.answer),
        citations: Vec::new(),
        trace_id: None,
        retrieval_hit_count: None,
        answer_strategy: Some("model".to_owned()),
        route_id: Some(answer.route_id),
        model: answer.model,
        usage: Some(answer.usage),
        latency_ms: Some(answer.latency_ms),
        run_id: None,
        agent_trace_id: None,
        status: None,
        intent: None,
        loop_kind: None,
        selected_tool_code: None,
        pause_reason: None,
        final_output: None,
        task_budget: None,
    }
}

fn openapi_agent_run_response(
    operation: String,
    required_permission: String,
    input: serde_json::Value,
    auth: IntegrationRuntimeContext,
    run: AgentRunResp,
) -> OpenApiInvokeResp {
    OpenApiInvokeResp {
        accepted: true,
        operation,
        required_permission,
        input,
        auth,
        answer: run.final_output.clone(),
        citations: Vec::new(),
        trace_id: None,
        retrieval_hit_count: None,
        answer_strategy: Some("agent".to_owned()),
        route_id: None,
        model: None,
        usage: None,
        latency_ms: None,
        run_id: Some(run.run_id),
        agent_trace_id: Some(run.trace_id),
        status: Some(run.status),
        intent: Some(run.intent),
        loop_kind: Some(run.loop_kind),
        selected_tool_code: run.selected_tool_code,
        pause_reason: run.pause_reason,
        final_output: run.final_output,
        task_budget: Some(run.task_budget),
    }
}

fn validate_common_fields(
    app_id: &str,
    name: &str,
    qps_limit: i32,
    quota_limit: i64,
    expires_at: Option<&str>,
) -> Result<(), AppError> {
    if app_id.is_empty() {
        return Err(AppError::bad_request("应用标识不能为空"));
    }
    if name.is_empty() {
        return Err(AppError::bad_request("入口名称不能为空"));
    }
    ensure_max_chars("应用标识", app_id, 128)?;
    ensure_max_chars("入口名称", name, 100)?;
    if qps_limit <= 0 || qps_limit > 10_000 {
        return Err(AppError::bad_request("QPS 限制必须在 1 到 10000 之间"));
    }
    if quota_limit <= 0 {
        return Err(AppError::bad_request("用量限制必须大于 0"));
    }
    if expires_at.is_some() {
        parse_optional_datetime(expires_at.map(str::to_owned))?;
    }
    Ok(())
}

fn normalize_permission_scope(scope: Vec<String>) -> Result<Vec<String>, AppError> {
    let mut normalized = scope
        .into_iter()
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    if normalized.is_empty() {
        return Err(AppError::bad_request("权限范围不能为空"));
    }
    if normalized.iter().any(|item| item.len() > 128) {
        return Err(AppError::bad_request("权限范围过长"));
    }
    Ok(normalized)
}

fn normalize_public_path(path: &str) -> Result<String, AppError> {
    let path = path.trim();
    if !path.starts_with('/') || path.contains("..") || path.contains('\\') {
        return Err(AppError::bad_request("Public Link 路径无效"));
    }
    ensure_max_chars("Public Link 路径", path, 255)?;
    Ok(path.to_owned())
}

fn normalize_runtime_secret(value: &str, expected_prefix: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || !value.starts_with(expected_prefix) {
        return Err(AppError::Unauthorized);
    }
    ensure_max_chars("集成入口凭据", value, 255)?;
    Ok(value.to_owned())
}

fn runtime_context_from_api_key_record(
    record: RuntimeApiKeyRecord,
    requested_app_id: &str,
    required_permission: &str,
) -> Result<IntegrationRuntimeContext, AppError> {
    if record.app_id != requested_app_id {
        return Err(AppError::Forbidden);
    }
    let permission_scope = string_array(record.permission_scope);
    if !permission_scope
        .iter()
        .any(|item| item == required_permission)
    {
        return Err(AppError::Forbidden);
    }

    Ok(IntegrationRuntimeContext {
        principal_type: "apiKey".to_owned(),
        tenant_id: record.tenant_id,
        app_id: record.app_id,
        name: record.name,
        path: None,
        masked_credential: record.masked_key,
        permission_scope,
        qps_limit: record.qps_limit,
        quota_limit: record.quota_limit,
        expires_at: record.expires_at.map(format_datetime),
        last_used_at: record.last_used_at.map(format_datetime),
    })
}

fn runtime_context_from_public_link_record(
    record: RuntimePublicLinkRecord,
) -> Result<IntegrationRuntimeContext, AppError> {
    let permission_scope = string_array(record.permission_scope);
    if permission_scope.is_empty() {
        return Err(AppError::Forbidden);
    }

    Ok(IntegrationRuntimeContext {
        principal_type: "publicLink".to_owned(),
        tenant_id: record.tenant_id,
        app_id: record.app_id,
        name: record.name,
        path: Some(record.path),
        masked_credential: record.masked_token,
        permission_scope,
        qps_limit: record.qps_limit,
        quota_limit: record.quota_limit,
        expires_at: record.expires_at.map(format_datetime),
        last_used_at: record.last_used_at.map(format_datetime),
    })
}

fn integration_usage_subject(
    context: &IntegrationRuntimeContext,
    credential_id: i64,
) -> Result<IntegrationUsageSubject, AppError> {
    let principal_type = match context.principal_type.as_str() {
        "apiKey" => IntegrationPrincipalType::ApiKey,
        "publicLink" => IntegrationPrincipalType::PublicLink,
        _ => return Err(AppError::Forbidden),
    };

    build_integration_usage_subject(
        principal_type,
        context.tenant_id,
        credential_id.to_string(),
        context.qps_limit,
        context.quota_limit,
    )
    .map_err(integration_usage_error)
}

fn enforce_runtime_usage_limits(
    subject: &IntegrationUsageSubject,
    qps_usage: i64,
    quota_usage: i64,
) -> Result<(), AppError> {
    enforce_integration_usage_limits(subject, qps_usage, quota_usage)
        .map_err(integration_usage_error)
}

fn integration_usage_error(_err: IntegrationUsageLimitError) -> AppError {
    AppError::Forbidden
}

fn parse_optional_datetime(value: Option<String>) -> Result<Option<NaiveDateTime>, AppError> {
    let Some(value) = value
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
    else {
        return Ok(None);
    };
    if let Ok(parsed) = DateTime::parse_from_rfc3339(&value) {
        return Ok(Some(parsed.naive_utc()));
    }
    NaiveDateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S")
        .map(Some)
        .map_err(|_| AppError::bad_request("过期时间格式无效"))
}

fn api_key_response(record: ApiKeyRecord, plain_key: Option<String>) -> ApiKeyResp {
    api_key_response_with_usage(record, plain_key, None)
}

fn api_key_response_with_usage(
    record: ApiKeyRecord,
    plain_key: Option<String>,
    usage: Option<RuntimeUsageSnapshot>,
) -> ApiKeyResp {
    let usage_summary = integration_usage_summary(record.qps_limit, record.quota_limit, usage);
    ApiKeyResp {
        id: record.id,
        app_id: record.app_id,
        name: record.name,
        key_prefix: record.key_prefix,
        masked_key: record.masked_key,
        permission_scope: string_array(record.permission_scope),
        qps_limit: record.qps_limit,
        quota_limit: record.quota_limit,
        expires_at: record.expires_at.map(format_datetime),
        last_used_at: record.last_used_at.map(format_datetime),
        usage_summary,
        status: record.status,
        create_time: format_datetime(record.create_time),
        update_time: record.update_time.map(format_datetime),
        plain_key,
    }
}

fn public_link_response(record: PublicLinkRecord) -> PublicLinkResp {
    public_link_response_with_usage(record, None)
}

fn public_link_response_with_usage(
    record: PublicLinkRecord,
    usage: Option<RuntimeUsageSnapshot>,
) -> PublicLinkResp {
    let usage_summary = integration_usage_summary(record.qps_limit, record.quota_limit, usage);
    PublicLinkResp {
        id: record.id,
        app_id: record.app_id,
        name: record.name,
        path: record.path,
        public_url: record.public_url,
        masked_token: record.masked_token,
        permission_scope: string_array(record.permission_scope),
        qps_limit: record.qps_limit,
        quota_limit: record.quota_limit,
        expires_at: record.expires_at.map(format_datetime),
        last_used_at: record.last_used_at.map(format_datetime),
        usage_summary,
        status: record.status,
        create_time: format_datetime(record.create_time),
        update_time: record.update_time.map(format_datetime),
    }
}

fn integration_usage_summary(
    qps_limit: i32,
    quota_limit: i64,
    usage: Option<RuntimeUsageSnapshot>,
) -> IntegrationUsageSummaryResp {
    let usage = usage.unwrap_or_default();
    IntegrationUsageSummaryResp {
        qps_used: usage.qps_used,
        qps_limit,
        quota_used: usage.quota_used,
        quota_limit,
        qps_window_start: usage.qps_window_start.map(format_datetime),
        quota_window_start: usage.quota_window_start.map(format_datetime),
    }
}

fn usage_snapshots_from_rows(
    rows: Vec<UsageMeterSummaryRecord>,
) -> HashMap<i64, RuntimeUsageSnapshot> {
    let mut snapshots = HashMap::new();
    for row in rows {
        let Ok(entry_id) = row.scope_id.parse::<i64>() else {
            continue;
        };
        let snapshot = snapshots
            .entry(entry_id)
            .or_insert_with(RuntimeUsageSnapshot::default);
        match row.resource_type.as_str() {
            INTEGRATION_QPS_RESOURCE => {
                snapshot.qps_used = row.usage_value;
                snapshot.qps_window_start = Some(row.window_start);
            }
            INTEGRATION_QUOTA_RESOURCE => {
                snapshot.quota_used = row.usage_value;
                snapshot.quota_window_start = Some(row.window_start);
            }
            _ => {}
        }
    }
    snapshots
}

fn public_url_for_token(token: &str) -> String {
    format!("{DEFAULT_PUBLIC_ORIGIN}/share/{token}")
}

fn string_array(value: serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn generate_secret(prefix: &str) -> String {
    let mut bytes = [0_u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("{}_{}", prefix, URL_SAFE_NO_PAD.encode(bytes))
}

fn sha256_hex(value: &str) -> String {
    hex_encode(&Sha256::digest(value.as_bytes()))
}

fn mask_secret(value: &str) -> String {
    let suffix = value
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    let prefix = value.split('_').next().unwrap_or("secret");
    format!("{prefix}_****{suffix}")
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn format_datetime(value: NaiveDateTime) -> String {
    value.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_size() -> u64 {
    DEFAULT_PAGE_SIZE
}

fn default_enabled_status() -> Option<i16> {
    Some(DEFAULT_STATUS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_command_normalizes_scope_limits_and_expiry() {
        let command = normalize_api_key_command(ApiKeyCommand {
            app_id: " training_app ".to_owned(),
            name: " Training API ".to_owned(),
            permission_scope: vec![
                " app:training:ask ".to_owned(),
                "app:training:ask".to_owned(),
            ],
            qps_limit: 5,
            quota_limit: 1000,
            expires_at: Some("2026-12-31T00:00:00Z".to_owned()),
        })
        .unwrap();

        assert_eq!(command.app_id, "training_app");
        assert_eq!(command.name, "Training API");
        assert_eq!(command.permission_scope, vec!["app:training:ask"]);
        assert_eq!(
            parse_optional_datetime(command.expires_at)
                .unwrap()
                .unwrap(),
            DateTime::parse_from_rfc3339("2026-12-31T00:00:00Z")
                .unwrap()
                .naive_utc()
        );
    }

    #[test]
    fn public_link_command_rejects_unsafe_path_and_empty_scope() {
        let err = normalize_public_link_command(PublicLinkCommand {
            app_id: "training_app".to_owned(),
            name: "Training Preview".to_owned(),
            path: "../admin".to_owned(),
            permission_scope: vec![],
            qps_limit: 2,
            quota_limit: 200,
            expires_at: None,
        })
        .unwrap_err();

        assert!(err.to_string().contains("路径") || err.to_string().contains("权限范围"));
    }

    #[test]
    fn generated_secret_is_masked_and_hashed_without_plaintext_leak() {
        let secret = generate_secret(API_KEY_PREFIX);
        let hash = sha256_hex(&secret);
        let masked = mask_secret(&secret);

        assert!(secret.starts_with("nxk_live_"));
        assert_eq!(hash.len(), 64);
        assert!(masked.contains("****"));
        assert!(!hash.contains(&secret));
        assert_ne!(masked, secret);
    }

    #[test]
    fn public_link_url_contains_hashable_runtime_token() {
        let token = generate_secret(PUBLIC_LINK_PREFIX);
        let url = public_url_for_token(&token);
        let url_token = url.rsplit('/').next().unwrap();

        assert!(url.ends_with(&format!("/share/{token}")));
        assert_eq!(sha256_hex(url_token), sha256_hex(&token));
        assert!(normalize_runtime_secret(url_token, PUBLIC_LINK_PREFIX).is_ok());
    }

    #[test]
    fn runtime_api_key_context_requires_requested_app_and_scope() {
        let record = RuntimeApiKeyRecord {
            id: 1,
            tenant_id: 1,
            create_user: 99,
            app_id: "training_app".to_owned(),
            name: "Training API".to_owned(),
            masked_key: "nxk_live_****abcd".to_owned(),
            permission_scope: json!(["app:training:ask"]),
            qps_limit: 5,
            quota_limit: 1000,
            expires_at: None,
            last_used_at: None,
        };

        let context =
            runtime_context_from_api_key_record(record.clone(), "training_app", "app:training:ask")
                .unwrap();
        assert_eq!(context.principal_type, "apiKey");
        assert_eq!(context.tenant_id, 1);
        assert_eq!(context.permission_scope, vec!["app:training:ask"]);

        assert!(matches!(
            runtime_context_from_api_key_record(
                record.clone(),
                "agent_workspace",
                "app:training:ask"
            )
            .unwrap_err(),
            AppError::Forbidden
        ));
        assert!(matches!(
            runtime_context_from_api_key_record(record, "training_app", "ai:agent:run")
                .unwrap_err(),
            AppError::Forbidden
        ));
    }

    #[test]
    fn integration_usage_subject_binds_runtime_context_to_meter_scope() {
        let api_key = IntegrationRuntimeContext {
            principal_type: "apiKey".to_owned(),
            tenant_id: 11,
            app_id: "training_app".to_owned(),
            name: "Training API".to_owned(),
            path: None,
            masked_credential: "nxk_live_****abcd".to_owned(),
            permission_scope: vec!["app:training:ask".to_owned()],
            qps_limit: 2,
            quota_limit: 5,
            expires_at: None,
            last_used_at: None,
        };
        let public_link = IntegrationRuntimeContext {
            principal_type: "publicLink".to_owned(),
            tenant_id: 11,
            app_id: "training_app".to_owned(),
            name: "Training Share".to_owned(),
            path: Some("/ask".to_owned()),
            masked_credential: "nxl_****wxyz".to_owned(),
            permission_scope: vec!["app:training:ask".to_owned()],
            qps_limit: 3,
            quota_limit: 8,
            expires_at: None,
            last_used_at: None,
        };

        assert_eq!(
            integration_usage_subject(&api_key, 42).unwrap(),
            IntegrationUsageSubject {
                tenant_id: 11,
                scope_type: "api_key".to_owned(),
                scope_id: "42".to_owned(),
                qps_limit: 2,
                quota_limit: 5,
            }
        );
        assert_eq!(
            integration_usage_subject(&public_link, 43).unwrap(),
            IntegrationUsageSubject {
                tenant_id: 11,
                scope_type: "public_link".to_owned(),
                scope_id: "43".to_owned(),
                qps_limit: 3,
                quota_limit: 8,
            }
        );
    }

    #[test]
    fn integration_usage_windows_cover_second_qps_and_monthly_quota() {
        let now = DateTime::parse_from_rfc3339("2026-06-06T08:09:10Z")
            .unwrap()
            .naive_utc();
        let windows = integration_usage_windows(now);

        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].resource_type, "external_integration.qps");
        assert_eq!(windows[0].usage_unit, "request");
        assert_eq!(windows[0].window_start, now);
        assert_eq!(
            windows[0].window_end,
            DateTime::parse_from_rfc3339("2026-06-06T08:09:11Z")
                .unwrap()
                .naive_utc()
        );
        assert_eq!(windows[1].resource_type, "external_integration.quota");
        assert_eq!(windows[1].usage_unit, "request");
        assert_eq!(
            windows[1].window_start,
            DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z")
                .unwrap()
                .naive_utc()
        );
        assert_eq!(
            windows[1].window_end,
            DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
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

        assert!(enforce_runtime_usage_limits(&subject, 2, 5).is_ok());
        assert!(matches!(
            enforce_runtime_usage_limits(&subject, 3, 5).unwrap_err(),
            AppError::Forbidden
        ));
        assert!(matches!(
            enforce_runtime_usage_limits(&subject, 2, 6).unwrap_err(),
            AppError::Forbidden
        ));
    }

    #[test]
    fn integration_entry_responses_include_usage_summary_defaults() {
        let api_key = api_key_response(
            ApiKeyRecord {
                id: 42,
                app_id: "training_app".to_owned(),
                name: "Training API".to_owned(),
                key_prefix: "nxk_live".to_owned(),
                masked_key: "nxk_live_****abcd".to_owned(),
                permission_scope: json!(["app:training:ask"]),
                qps_limit: 5,
                quota_limit: 1000,
                expires_at: None,
                last_used_at: None,
                status: 1,
                create_time: DateTime::parse_from_rfc3339("2026-06-06T10:00:00Z")
                    .unwrap()
                    .naive_utc(),
                update_time: None,
            },
            None,
        );

        assert_eq!(api_key.usage_summary.qps_used, 0);
        assert_eq!(api_key.usage_summary.qps_limit, 5);
        assert_eq!(api_key.usage_summary.quota_used, 0);
        assert_eq!(api_key.usage_summary.quota_limit, 1000);

        let value = serde_json::to_value(api_key).unwrap();
        assert_eq!(value["usageSummary"]["qpsUsed"], 0);
        assert_eq!(value["usageSummary"]["quotaLimit"], 1000);
    }

    #[test]
    fn training_ask_input_normalizes_dataset_question_and_limit() {
        let ask = normalize_training_ask_input(&json!({
            "datasetId": "42",
            "question": "  Novex training policy?  ",
            "limit": 99
        }))
        .unwrap();

        assert_eq!(ask.dataset_id, 42);
        assert_eq!(ask.command.question, "Novex training policy?");
        assert_eq!(ask.command.limit, 24);

        let ask = normalize_training_ask_input(&json!({
            "dataset_id": 43,
            "question": "How to pass M5 smoke?",
            "limit": "2"
        }))
        .unwrap();

        assert_eq!(ask.dataset_id, 43);
        assert_eq!(ask.command.limit, 2);
    }

    #[test]
    fn training_ask_input_rejects_invalid_payloads() {
        assert!(normalize_training_ask_input(&json!("not an object")).is_err());
        assert!(normalize_training_ask_input(&json!({
            "datasetId": 0,
            "question": "hello"
        }))
        .is_err());
        assert!(normalize_training_ask_input(&json!({
            "datasetId": 42,
            "question": "   "
        }))
        .is_err());
    }

    #[test]
    fn openapi_chat_input_accepts_question_payload() {
        let command = normalize_openapi_chat_input(&json!({
            "question": "  Draft a training reminder.  ",
            "temperature": "0.7",
            "maxTokens": "256"
        }))
        .unwrap();

        assert_eq!(command.messages.len(), 1);
        assert_eq!(command.messages[0].role, "user");
        assert_eq!(command.messages[0].content, "Draft a training reminder.");
        assert_eq!(command.temperature, Some(0.7));
        assert_eq!(command.max_tokens, Some(256));
    }

    #[test]
    fn openapi_chat_input_accepts_messages_payload() {
        let command = normalize_openapi_chat_input(&json!({
            "messages": [
                {"role": " system ", "content": "  You are Novex.  "},
                {"role": "user", "content": "  Explain M5 templates.  "}
            ]
        }))
        .unwrap();

        assert_eq!(command.messages.len(), 2);
        assert_eq!(command.messages[0].role, "system");
        assert_eq!(command.messages[0].content, "You are Novex.");
        assert_eq!(command.messages[1].role, "user");
        assert_eq!(command.messages[1].content, "Explain M5 templates.");
    }

    #[test]
    fn openapi_chat_input_rejects_empty_payloads() {
        assert!(normalize_openapi_chat_input(&json!({})).is_err());
        assert!(normalize_openapi_chat_input(&json!({"question": "  "})).is_err());
        assert!(normalize_openapi_chat_input(&json!({"messages": []})).is_err());
    }

    #[test]
    fn openapi_agent_run_input_accepts_task_payload() {
        let command = normalize_openapi_agent_run_input(&json!({
            "task": "  Search the repository and summarize recent agent changes.  ",
            "autoApprove": true,
            "budget": {
                "maxSteps": 6,
                "maxToolCalls": 1,
                "maxSeconds": 30
            }
        }))
        .unwrap();

        assert_eq!(
            command.input,
            "Search the repository and summarize recent agent changes."
        );
        assert!(command.auto_approve);
        assert_eq!(command.budget.max_steps, Some(6));
        assert_eq!(command.budget.max_tool_calls, Some(1));
        assert_eq!(command.budget.max_seconds, Some(30));
    }

    #[test]
    fn openapi_agent_run_input_rejects_empty_payloads() {
        assert!(normalize_openapi_agent_run_input(&json!("not an object")).is_err());
        assert!(normalize_openapi_agent_run_input(&json!({})).is_err());
        assert!(normalize_openapi_agent_run_input(&json!({"task": "  "})).is_err());
    }

    #[test]
    fn training_ask_response_serializes_rag_contract() {
        let auth = IntegrationRuntimeContext {
            principal_type: "apiKey".to_owned(),
            tenant_id: 1,
            app_id: "training_app".to_owned(),
            name: "Training API".to_owned(),
            path: None,
            masked_credential: "nxk_live_****abcd".to_owned(),
            permission_scope: vec!["app:training:ask".to_owned()],
            qps_limit: 5,
            quota_limit: 1000,
            expires_at: None,
            last_used_at: None,
        };
        let resp = openapi_training_ask_response(
            "training.ask".to_owned(),
            "app:training:ask".to_owned(),
            json!({"datasetId": 42, "question": "Novex?"}),
            auth,
            RagAskResp {
                trace_id: 7,
                answer: "Novex uses the indexed training corpus.".to_owned(),
                citations: vec![CitationResp {
                    document_id: "doc-1".to_owned(),
                    chunk_id: "chunk-1".to_owned(),
                    page_no: Some(3),
                    section_path: vec!["Training".to_owned()],
                }],
                retrieval_hit_count: 1,
                answer_strategy: "extractive".to_owned(),
                embedding_model_route: "runtime.embedding".to_owned(),
                rerank_model_route: "runtime.reranker".to_owned(),
                answer_model_route: "runtime.llm".to_owned(),
                answer_model: Some("deepseek-v4-flash".to_owned()),
            },
        );

        let value = serde_json::to_value(resp).unwrap();

        assert_eq!(value["accepted"], true);
        assert_eq!(value["answer"], "Novex uses the indexed training corpus.");
        assert_eq!(value["traceId"], 7);
        assert_eq!(value["retrievalHitCount"], 1);
        assert_eq!(value["answerStrategy"], "extractive");
        assert_eq!(value["citations"][0]["documentId"], "doc-1");
        assert_eq!(value["citations"][0]["chunkId"], "chunk-1");
        assert_eq!(value["auth"]["principalType"], "apiKey");
    }

    #[test]
    fn openapi_chat_response_serializes_model_contract() {
        let auth = IntegrationRuntimeContext {
            principal_type: "apiKey".to_owned(),
            tenant_id: 1,
            app_id: "llm_chat".to_owned(),
            name: "LLM Chat API".to_owned(),
            path: None,
            masked_credential: "nxk_live_****abcd".to_owned(),
            permission_scope: vec!["app:chat:use".to_owned()],
            qps_limit: 5,
            quota_limit: 1000,
            expires_at: None,
            last_used_at: None,
        };
        let resp = openapi_chat_response(
            "chat.use".to_owned(),
            "app:chat:use".to_owned(),
            json!({"question": "Novex?"}),
            auth,
            ModelChatResp {
                conversation_id: None,
                answer: "Novex can answer with the configured chat model.".to_owned(),
                route_id: "runtime.llm".to_owned(),
                provider: "deep-seek".to_owned(),
                model: Some("deepseek-v4-flash".to_owned()),
                latency_ms: 42,
                usage: ModelChatUsage {
                    prompt_tokens: Some(9),
                    completion_tokens: Some(8),
                    total_tokens: Some(17),
                },
                cost_cents: None,
                provider_attempts: vec![],
            },
        );

        let value = serde_json::to_value(resp).unwrap();

        assert_eq!(
            value["answer"],
            "Novex can answer with the configured chat model."
        );
        assert_eq!(value["answerStrategy"], "model");
        assert_eq!(value["routeId"], "runtime.llm");
        assert_eq!(value["model"], "deepseek-v4-flash");
        assert_eq!(value["usage"]["totalTokens"], 17);
        assert_eq!(value["latencyMs"], 42);
        assert!(value.get("citations").is_none());
    }

    #[test]
    fn openapi_agent_run_response_serializes_run_contract() {
        let auth = IntegrationRuntimeContext {
            principal_type: "apiKey".to_owned(),
            tenant_id: 1,
            app_id: "agent_workspace".to_owned(),
            name: "Agent API".to_owned(),
            path: None,
            masked_credential: "nxk_live_****abcd".to_owned(),
            permission_scope: vec!["ai:agent:run".to_owned()],
            qps_limit: 5,
            quota_limit: 1000,
            expires_at: None,
            last_used_at: None,
        };
        let resp = openapi_agent_run_response(
            "agent.run".to_owned(),
            "ai:agent:run".to_owned(),
            json!({"task": "Summarize M5 status."}),
            auth,
            AgentRunResp {
                run_id: 42,
                trace_id: "agent-42".to_owned(),
                status: "succeeded".to_owned(),
                intent: "task_planning".to_owned(),
                loop_kind: "react".to_owned(),
                selected_tool_code: Some("github.repo.search".to_owned()),
                pause_reason: None,
                final_output: Some("M5 status summarized.".to_owned()),
                task_budget: TaskBudget {
                    max_steps: Some(6),
                    max_tool_calls: Some(1),
                    max_seconds: Some(30),
                    max_cost_cents: Some(0),
                },
                create_time: "2026-06-06 10:00:00".to_owned(),
                update_time: Some("2026-06-06 10:00:01".to_owned()),
            },
        );

        let value = serde_json::to_value(resp).unwrap();

        assert_eq!(value["accepted"], true);
        assert_eq!(value["runId"], 42);
        assert_eq!(value["agentTraceId"], "agent-42");
        assert_eq!(value["status"], "succeeded");
        assert_eq!(value["intent"], "task_planning");
        assert_eq!(value["loopKind"], "react");
        assert_eq!(value["selectedToolCode"], "github.repo.search");
        assert_eq!(value["finalOutput"], "M5 status summarized.");
        assert_eq!(value["taskBudget"]["maxSteps"], 6);
        assert_eq!(value["auth"]["principalType"], "apiKey");
    }

    #[test]
    fn openapi_chat_use_invocation_uses_metered_model_runtime() {
        let source = include_str!("integration_service.rs");
        let branch = ["command.operation == ", "\"chat.use\""].concat();
        let tenant_runtime = [
            "ModelRuntimeService::",
            "for_tenant(self.db.clone(), auth.tenant_id)",
        ]
        .concat();
        let metered_call = [".", "chat_completion_for_source("].concat();
        let source_tag = ["\"", "ai.openapi.chat", "\""].concat();

        assert!(
            source.contains(&branch),
            "chat.use must have a real OpenAPI branch"
        );
        assert!(
            source.contains(&tenant_runtime),
            "chat.use must bind model runtime to the authenticated tenant"
        );
        assert!(
            source.contains(&metered_call),
            "chat.use must record model usage with a source tag"
        );
        assert!(
            source.contains(&source_tag),
            "chat.use model usage source must be ai.openapi.chat"
        );
    }

    #[test]
    fn openapi_agent_run_invocation_uses_tenant_bound_agent_service() {
        let source = include_str!("integration_service.rs");
        let branch = ["command.operation == ", "\"agent.run\""].concat();
        let tenant_runtime = [
            "AgentService::",
            "for_tenant(self.db.clone(), auth.tenant_id)",
        ]
        .concat();
        let create_run_call = [".", "create_run(authenticated.user_id, agent)"].concat();

        assert!(
            source.contains(&branch),
            "agent.run must have a real OpenAPI branch"
        );
        assert!(
            source.contains(&tenant_runtime),
            "agent.run must bind AgentService to the authenticated tenant"
        );
        assert!(
            source.contains(&create_run_call),
            "agent.run must create a real Agent run for the API key user"
        );
    }

    #[test]
    fn openapi_operation_maps_to_template_permissions() {
        assert_eq!(
            required_permission_for_operation("training.ask").unwrap(),
            "app:training:ask"
        );
        assert_eq!(
            required_permission_for_operation("chat.use").unwrap(),
            "app:chat:use"
        );
        assert_eq!(
            required_permission_for_operation("agent.run").unwrap(),
            "ai:agent:run"
        );
        assert!(required_permission_for_operation("unknown").is_err());
    }
}
