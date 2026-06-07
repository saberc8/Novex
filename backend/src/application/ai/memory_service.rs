use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

use crate::{
    application::system::{ensure_max_chars, format_datetime},
    infrastructure::persistence::ai_memory_repository::{
        AiMemoryRepository, MemoryFilter, MemoryRecord, MemorySaveRecord,
    },
    shared::{
        error::AppError,
        id::next_id,
        pagination::{PageQuery, PageResult, DEFAULT_PAGE},
    },
};

const DEFAULT_TENANT_ID: i64 = 1;
const DEFAULT_MEMORY_PAGE_SIZE: u64 = 20;
const ENABLED_STATUS: i16 = 1;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_memory_size")]
    pub size: u64,
    #[serde(default)]
    pub scope_type: Option<String>,
    #[serde(default)]
    pub scope_id: Option<String>,
}

impl MemoryQuery {
    pub fn page_query(&self) -> PageQuery {
        PageQuery {
            page: self.page,
            size: self.size,
        }
        .normalized()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCommand {
    #[serde(default)]
    pub scope_type: String,
    #[serde(default)]
    pub scope_id: String,
    #[serde(default)]
    pub source_kind: String,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub sensitivity: String,
    #[serde(default)]
    pub write_policy: String,
    #[serde(default)]
    pub ttl_days: Option<i32>,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default = "default_enabled_status_i16")]
    pub status: i16,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryResp {
    pub id: i64,
    pub scope_type: String,
    pub scope_id: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub content: String,
    pub summary: String,
    pub sensitivity: String,
    pub write_policy: String,
    pub ttl_days: Option<i32>,
    pub expires_at: Option<String>,
    pub metadata: Value,
    pub status: i16,
    pub create_time: String,
    pub update_time: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MemoryService {
    tenant_id: i64,
    repo: AiMemoryRepository,
}

impl MemoryService {
    pub fn new(db: PgPool) -> Self {
        Self::for_tenant(db, DEFAULT_TENANT_ID)
    }

    pub fn for_tenant(db: PgPool, tenant_id: i64) -> Self {
        Self {
            tenant_id,
            repo: AiMemoryRepository::new(db),
        }
    }

    pub async fn list_memories(
        &self,
        query: MemoryQuery,
    ) -> Result<PageResult<MemoryResp>, AppError> {
        let page = query.page_query();
        let scope_type = query
            .scope_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let scope_id = query
            .scope_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let filter = MemoryFilter {
            tenant_id: self.tenant_id,
            scope_type,
            scope_id,
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_memories(&filter).await?;
        let list = self
            .repo
            .list_memories(&filter)
            .await?
            .into_iter()
            .map(MemoryResp::from)
            .collect();

        Ok(PageResult::new(list, total))
    }

    pub async fn upsert_memory(
        &self,
        user_id: i64,
        command: MemoryCommand,
    ) -> Result<MemoryResp, AppError> {
        let command = normalize_memory_command(command)?;
        let now = Utc::now().naive_utc();
        let expires_at = command
            .ttl_days
            .map(|days| now + Duration::days(i64::from(days)));
        let owner_user_id = if command.scope_type == "user" {
            command.scope_id.parse::<i64>().ok().or(Some(user_id))
        } else {
            None
        };
        let record = MemorySaveRecord {
            id: next_id(),
            tenant_id: self.tenant_id,
            scope_type: command.scope_type,
            scope_id: command.scope_id,
            owner_user_id,
            source_kind: command.source_kind,
            source_id: command.source_id,
            content: command.content,
            summary: command.summary,
            sensitivity: command.sensitivity,
            write_policy: command.write_policy,
            ttl_days: command.ttl_days,
            expires_at,
            metadata: command.metadata,
            status: command.status,
            user_id,
            now,
        };

        Ok(MemoryResp::from(self.repo.upsert_memory(&record).await?))
    }

    pub async fn delete_memory(&self, user_id: i64, memory_id: i64) -> Result<bool, AppError> {
        self.repo
            .soft_delete_memory(self.tenant_id, memory_id, user_id, Utc::now().naive_utc())
            .await
    }
}

pub fn normalize_memory_command(mut command: MemoryCommand) -> Result<MemoryCommand, AppError> {
    command.scope_type = command.scope_type.trim().to_ascii_lowercase();
    command.scope_id = command.scope_id.trim().to_owned();
    command.source_kind = command.source_kind.trim().to_ascii_lowercase();
    command.source_id = trim_optional(command.source_id);
    command.content = command.content.trim().to_owned();
    command.summary = command.summary.trim().to_owned();
    command.sensitivity = command.sensitivity.trim().to_ascii_lowercase();
    command.write_policy = command
        .write_policy
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_");
    if command.metadata.is_null() {
        command.metadata = Value::Object(Default::default());
    }
    if command.summary.is_empty() {
        command.summary = summarize_content(&command.content);
    }
    if command.sensitivity.is_empty() {
        command.sensitivity = "low".to_owned();
    }
    if command.write_policy.is_empty() {
        command.write_policy = "user_approved".to_owned();
    }
    if command.source_kind.is_empty() {
        command.source_kind = "manual".to_owned();
    }

    if !matches!(
        command.scope_type.as_str(),
        "session" | "user" | "org" | "project"
    ) {
        return Err(AppError::bad_request("Memory scope 无效"));
    }
    if command.scope_id.is_empty() {
        return Err(AppError::bad_request("Memory scopeId 不能为空"));
    }
    if !matches!(
        command.source_kind.as_str(),
        "manual" | "agent" | "rag" | "trigger" | "system"
    ) {
        return Err(AppError::bad_request("Memory sourceKind 无效"));
    }
    if command.content.is_empty() {
        return Err(AppError::bad_request("Memory content 不能为空"));
    }
    if !matches!(
        command.sensitivity.as_str(),
        "low" | "preference" | "confidential" | "regulated"
    ) {
        return Err(AppError::bad_request("Memory sensitivity 无效"));
    }
    if !matches!(
        command.write_policy.as_str(),
        "disabled" | "user_approved" | "automatic"
    ) {
        return Err(AppError::bad_request("Memory writePolicy 无效"));
    }
    if let Some(ttl_days) = command.ttl_days {
        if !(1..=3650).contains(&ttl_days) {
            return Err(AppError::bad_request(
                "Memory ttlDays 必须在 1 到 3650 之间",
            ));
        }
    }
    if !(0..=1).contains(&command.status) {
        return Err(AppError::bad_request("Memory status 无效"));
    }
    if !command.metadata.is_object() {
        return Err(AppError::bad_request("Memory metadata 必须是对象"));
    }
    if command.scope_type == "user"
        && command.write_policy == "automatic"
        && command.metadata["confirmedByUser"] != Value::Bool(true)
    {
        return Err(AppError::bad_request("User Memory 自动写入需要用户确认"));
    }

    ensure_max_chars("Memory scope", &command.scope_type, 32)?;
    ensure_max_chars("Memory scopeId", &command.scope_id, 128)?;
    ensure_max_chars("Memory sourceKind", &command.source_kind, 64)?;
    if let Some(source_id) = command.source_id.as_deref() {
        ensure_max_chars("Memory sourceId", source_id, 128)?;
    }
    ensure_max_chars("Memory content", &command.content, 4000)?;
    ensure_max_chars("Memory summary", &command.summary, 512)?;
    ensure_max_chars("Memory sensitivity", &command.sensitivity, 32)?;
    ensure_max_chars("Memory writePolicy", &command.write_policy, 32)?;
    Ok(command)
}

impl From<MemoryRecord> for MemoryResp {
    fn from(record: MemoryRecord) -> Self {
        Self {
            id: record.id,
            scope_type: record.scope_type,
            scope_id: record.scope_id,
            source_kind: record.source_kind,
            source_id: record.source_id,
            content: record.content,
            summary: record.summary,
            sensitivity: record.sensitivity,
            write_policy: record.write_policy,
            ttl_days: record.ttl_days,
            expires_at: record.expires_at.map(format_datetime),
            metadata: record.metadata,
            status: record.status,
            create_time: format_datetime(record.create_time),
            update_time: record.update_time.map(format_datetime),
        }
    }
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn summarize_content(content: &str) -> String {
    content.chars().take(160).collect()
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_memory_size() -> u64 {
    DEFAULT_MEMORY_PAGE_SIZE
}

fn default_enabled_status_i16() -> i16 {
    ENABLED_STATUS
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;

    #[tokio::test]
    async fn memory_service_can_be_bound_to_request_tenant() {
        let db = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();

        let service = MemoryService::for_tenant(db, 42);

        assert_eq!(service.tenant_id, 42);
    }

    #[test]
    fn memory_command_normalizes_scope_policy_ttl_and_metadata() {
        let command = normalize_memory_command(MemoryCommand {
            scope_type: " User ".to_owned(),
            scope_id: " 1 ".to_owned(),
            source_kind: " Manual ".to_owned(),
            source_id: Some(" note-7 ".to_owned()),
            content: " prefers concise delivery updates ".to_owned(),
            summary: " concise updates ".to_owned(),
            sensitivity: " Preference ".to_owned(),
            write_policy: " User_Approved ".to_owned(),
            ttl_days: Some(90),
            metadata: Value::Null,
            status: 1,
        })
        .expect("memory command should normalize");

        assert_eq!(command.scope_type, "user");
        assert_eq!(command.scope_id, "1");
        assert_eq!(command.source_kind, "manual");
        assert_eq!(command.source_id.as_deref(), Some("note-7"));
        assert_eq!(command.sensitivity, "preference");
        assert_eq!(command.write_policy, "user_approved");
        assert_eq!(command.metadata, json!({}));
    }

    #[test]
    fn automatic_user_memory_requires_user_confirmation_metadata() {
        let err = normalize_memory_command(MemoryCommand {
            scope_type: "user".to_owned(),
            scope_id: "1".to_owned(),
            source_kind: "agent".to_owned(),
            source_id: None,
            content: "remember a private preference".to_owned(),
            summary: "".to_owned(),
            sensitivity: "preference".to_owned(),
            write_policy: "automatic".to_owned(),
            ttl_days: Some(30),
            metadata: json!({ "confirmedByUser": false }),
            status: 1,
        })
        .unwrap_err();

        assert!(err.to_string().contains("确认"));
    }
}
