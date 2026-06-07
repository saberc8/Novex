use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::{
    application::system::{ensure_max_chars, format_datetime},
    infrastructure::persistence::system_secret_repository::{
        SecretFilter, SecretRecord, SecretSaveRecord, SystemSecretRepository,
    },
    shared::{
        error::AppError,
        id::next_id,
        pagination::{PageQuery, PageResult, DEFAULT_PAGE},
    },
};

const DEFAULT_TENANT_ID: i64 = 1;
const DEFAULT_SECRET_PAGE_SIZE: u64 = 20;
const ENABLED_STATUS: i16 = 1;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_secret_size")]
    pub size: u64,
    #[serde(default)]
    pub scope_type: Option<String>,
    #[serde(default)]
    pub scope_id: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
}

impl SecretQuery {
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
pub struct SecretCommand {
    #[serde(default)]
    pub scope_type: String,
    #[serde(default)]
    pub scope_id: String,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub plaintext: String,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default = "default_enabled_status")]
    pub status: i16,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretRecordPublicResp {
    pub id: i64,
    pub scope_type: String,
    pub scope_id: String,
    pub code: String,
    pub key_version: i32,
    pub masked_value: String,
    pub expires_at: Option<String>,
    pub rotated_at: Option<String>,
    pub last_used_at: Option<String>,
    pub metadata: Value,
    pub status: i16,
    pub create_time: String,
    pub update_time: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SecretService {
    repo: SystemSecretRepository,
}

impl SecretService {
    pub fn new(db: PgPool) -> Self {
        Self {
            repo: SystemSecretRepository::new(db),
        }
    }

    pub async fn list(
        &self,
        query: SecretQuery,
    ) -> Result<PageResult<SecretRecordPublicResp>, AppError> {
        let page = query.page_query();
        let filter = SecretFilter {
            tenant_id: DEFAULT_TENANT_ID,
            scope_type: query.scope_type.as_deref(),
            scope_id: query.scope_id.as_deref(),
            code: query.code.as_deref(),
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count(&filter).await?;
        let list = self
            .repo
            .list(&filter)
            .await?
            .into_iter()
            .map(SecretRecordPublicResp::from)
            .collect();

        Ok(PageResult::new(list, total))
    }

    pub async fn upsert(
        &self,
        user_id: i64,
        command: SecretCommand,
    ) -> Result<SecretRecordPublicResp, AppError> {
        let command = normalize_secret_command(command)?;
        let latest = self
            .repo
            .latest_key_version(
                DEFAULT_TENANT_ID,
                &command.scope_type,
                &command.scope_id,
                &command.code,
            )
            .await?;
        let now = Utc::now().naive_utc();
        let record = SecretSaveRecord {
            id: next_id(),
            tenant_id: DEFAULT_TENANT_ID,
            scope_type: command.scope_type,
            scope_id: command.scope_id,
            code: command.code,
            key_version: latest + 1,
            ciphertext: seal_secret_value(&command.plaintext),
            masked_value: mask_secret_value(&command.plaintext),
            metadata: command.metadata,
            status: command.status,
            user_id,
            now,
        };

        Ok(SecretRecordPublicResp::from(
            self.repo.create_version(&record).await?,
        ))
    }
}

pub fn normalize_secret_command(mut command: SecretCommand) -> Result<SecretCommand, AppError> {
    command.scope_type = command.scope_type.trim().to_ascii_lowercase();
    command.scope_id = command.scope_id.trim().to_owned();
    command.code = command.code.trim().to_owned();
    command.plaintext = command.plaintext.trim().to_owned();
    if command.metadata.is_null() {
        command.metadata = Value::Object(Default::default());
    }

    if !matches!(
        command.scope_type.as_str(),
        "platform" | "tenant" | "user" | "app"
    ) {
        return Err(AppError::bad_request("密钥作用域无效"));
    }
    if command.scope_id.is_empty() {
        return Err(AppError::bad_request("密钥作用域ID不能为空"));
    }
    if command.code.is_empty() {
        return Err(AppError::bad_request("密钥编码不能为空"));
    }
    if command.plaintext.is_empty() {
        return Err(AppError::bad_request("密钥明文不能为空"));
    }
    if !(0..=1).contains(&command.status) {
        return Err(AppError::bad_request("密钥状态无效"));
    }

    ensure_max_chars("密钥作用域", &command.scope_type, 64)?;
    ensure_max_chars("密钥作用域ID", &command.scope_id, 128)?;
    ensure_max_chars("密钥编码", &command.code, 128)?;
    ensure_max_chars("密钥明文", &command.plaintext, 4096)?;
    Ok(command)
}

pub fn mask_secret_value(value: &str) -> String {
    let value = value.trim();
    let chars = value.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return String::new();
    }
    if chars.len() <= 8 {
        return "****".to_owned();
    }
    let prefix = chars.iter().take(4).collect::<String>();
    let suffix = chars
        .iter()
        .skip(chars.len().saturating_sub(4))
        .collect::<String>();
    format!("{prefix}****{suffix}")
}

fn seal_secret_value(plaintext: &str) -> String {
    let nonce = next_id().to_string();
    let key = secret_encryption_key();
    let mut sealed = Vec::with_capacity(plaintext.len());
    for (index, byte) in plaintext.as_bytes().iter().enumerate() {
        sealed.push(byte ^ key[index % key.len()]);
    }
    format!(
        "novex:v1:{}:{}",
        nonce,
        STANDARD_NO_PAD.encode(sealed.as_slice())
    )
}

fn secret_encryption_key() -> Vec<u8> {
    let raw = std::env::var("NOVEX_SECRET_ENCRYPTION_KEY")
        .or_else(|_| std::env::var("AUTH_JWT_SECRET"))
        .unwrap_or_else(|_| "local-test-secret-key-for-novex-secrets".to_owned());
    Sha256::digest(raw.as_bytes()).to_vec()
}

impl From<SecretRecord> for SecretRecordPublicResp {
    fn from(record: SecretRecord) -> Self {
        Self {
            id: record.id,
            scope_type: record.scope_type,
            scope_id: record.scope_id,
            code: record.code,
            key_version: record.key_version,
            masked_value: record.masked_value,
            expires_at: record.expires_at.map(format_datetime),
            rotated_at: record.rotated_at.map(format_datetime),
            last_used_at: record.last_used_at.map(format_datetime),
            metadata: record.metadata,
            status: record.status,
            create_time: format_datetime(record.create_time),
            update_time: record.update_time.map(format_datetime),
        }
    }
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_secret_size() -> u64 {
    DEFAULT_SECRET_PAGE_SIZE
}

fn default_enabled_status() -> i16 {
    ENABLED_STATUS
}
