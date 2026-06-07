use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

use crate::{
    application::system::format_datetime,
    infrastructure::persistence::identity_repository::{
        ExternalAccountFilter, ExternalAccountListRecord, IdentityProviderFilter,
        IdentityProviderListRecord, IdentityRepository,
    },
    shared::{
        error::AppError,
        pagination::{PageQuery, PageResult, DEFAULT_PAGE},
    },
};

const DEFAULT_TENANT_ID: i64 = 1;
const DEFAULT_IDENTITY_PAGE_SIZE: u64 = 20;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityResourceQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_identity_size")]
    pub size: u64,
    #[serde(default)]
    pub provider_type: Option<String>,
    #[serde(default)]
    pub provider_code: Option<String>,
    #[serde(default)]
    pub status: Option<i16>,
}

impl Default for IdentityResourceQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            size: DEFAULT_IDENTITY_PAGE_SIZE,
            provider_type: None,
            provider_code: None,
            status: Some(1),
        }
    }
}

impl IdentityResourceQuery {
    fn page_query(&self) -> PageQuery {
        PageQuery {
            page: self.page,
            size: self.size,
        }
        .normalized()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityProviderResp {
    pub id: i64,
    pub tenant_id: i64,
    pub provider_type: String,
    pub code: String,
    pub name: String,
    pub client_id: Option<String>,
    pub secret_ref: Option<String>,
    pub masked_secret_ref: String,
    pub allowed_domains: Value,
    pub tenant_policy: Value,
    pub status: i16,
    pub create_time: String,
    pub update_time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAccountResp {
    pub id: i64,
    pub tenant_id: i64,
    pub provider_id: i64,
    pub provider_code: String,
    pub provider_type: String,
    pub user_id: i64,
    pub external_subject: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub metadata: Value,
    pub last_login_at: Option<String>,
    pub status: i16,
    pub create_time: String,
    pub update_time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityPolicyResp {
    pub provider_id: i64,
    pub provider_code: String,
    pub provider_name: String,
    pub provider_type: String,
    pub allowed_domains: Value,
    pub tenant_policy: Value,
    pub status: i16,
    pub create_time: String,
}

#[derive(Debug, Clone)]
pub struct IdentityProviderService {
    tenant_id: i64,
    repo: IdentityRepository,
}

impl IdentityProviderService {
    pub fn new(db: PgPool) -> Self {
        Self::for_tenant(db, DEFAULT_TENANT_ID)
    }

    pub fn for_tenant(db: PgPool, tenant_id: i64) -> Self {
        Self {
            tenant_id,
            repo: IdentityRepository::new(db),
        }
    }

    pub async fn list_providers(
        &self,
        query: IdentityResourceQuery,
    ) -> Result<PageResult<IdentityProviderResp>, AppError> {
        let page = query.page_query();
        let filter = IdentityProviderFilter {
            tenant_id: self.tenant_id,
            provider_type: query.provider_type.as_deref(),
            provider_code: query.provider_code.as_deref(),
            status: query.status,
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_identity_providers(&filter).await?;
        let list = self
            .repo
            .list_identity_providers(&filter)
            .await?
            .into_iter()
            .map(IdentityProviderResp::from)
            .collect();

        Ok(PageResult::new(list, total))
    }

    pub async fn list_accounts(
        &self,
        query: IdentityResourceQuery,
    ) -> Result<PageResult<ExternalAccountResp>, AppError> {
        let page = query.page_query();
        let filter = ExternalAccountFilter {
            tenant_id: self.tenant_id,
            provider_code: query.provider_code.as_deref(),
            status: query.status,
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_external_accounts(&filter).await?;
        let list = self
            .repo
            .list_external_accounts(&filter)
            .await?
            .into_iter()
            .map(ExternalAccountResp::from)
            .collect();

        Ok(PageResult::new(list, total))
    }

    pub async fn list_policies(
        &self,
        query: IdentityResourceQuery,
    ) -> Result<PageResult<IdentityPolicyResp>, AppError> {
        let providers = self.list_providers(query).await?;
        let total = providers.total;
        let list = providers
            .list
            .into_iter()
            .map(|provider| IdentityPolicyResp {
                provider_id: provider.id,
                provider_code: provider.code,
                provider_name: provider.name,
                provider_type: provider.provider_type,
                allowed_domains: provider.allowed_domains,
                tenant_policy: provider.tenant_policy,
                status: provider.status,
                create_time: provider.create_time,
            })
            .collect();

        Ok(PageResult::new(list, total))
    }
}

impl From<IdentityProviderListRecord> for IdentityProviderResp {
    fn from(record: IdentityProviderListRecord) -> Self {
        let masked_secret_ref = record
            .secret_ref
            .as_deref()
            .map(mask_secret_ref)
            .unwrap_or_default();
        Self {
            id: record.id,
            tenant_id: record.tenant_id,
            provider_type: record.provider_type,
            code: record.code,
            name: record.name,
            client_id: record.client_id,
            secret_ref: record.secret_ref,
            masked_secret_ref,
            allowed_domains: record.allowed_domains,
            tenant_policy: record.tenant_policy,
            status: record.status,
            create_time: format_datetime(record.create_time),
            update_time: record.update_time.map(format_datetime),
        }
    }
}

impl From<ExternalAccountListRecord> for ExternalAccountResp {
    fn from(record: ExternalAccountListRecord) -> Self {
        Self {
            id: record.id,
            tenant_id: record.tenant_id,
            provider_id: record.provider_id,
            provider_code: record.provider_code,
            provider_type: record.provider_type,
            user_id: record.user_id,
            external_subject: record.external_subject,
            display_name: record.display_name,
            email: record.email,
            metadata: record.metadata,
            last_login_at: record.last_login_at.map(format_datetime),
            status: record.status,
            create_time: format_datetime(record.create_time),
            update_time: record.update_time.map(format_datetime),
        }
    }
}

fn mask_secret_ref(secret_ref: &str) -> String {
    if let Some(env_name) = secret_ref.trim().strip_prefix("env:") {
        let prefix: String = env_name.chars().take(4).collect();
        return format!("env:{prefix}****");
    }
    "****".to_owned()
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_identity_size() -> u64 {
    DEFAULT_IDENTITY_PAGE_SIZE
}
