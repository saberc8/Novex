use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};

use crate::shared::{error::AppError, id::next_id};

#[derive(Debug, Clone, FromRow)]
pub struct IdentityProviderRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub provider_type: String,
    pub code: String,
    pub name: String,
    pub client_id: Option<String>,
    pub secret_ref: Option<String>,
    pub allowed_domains: Value,
    pub tenant_policy: Value,
    pub status: i16,
}

#[derive(Debug, Clone)]
pub struct OAuthStateSaveRecord {
    pub tenant_id: i64,
    pub provider_id: i64,
    pub state_hash: String,
    pub redirect_uri: String,
    pub requested_scopes: Value,
    pub code_verifier_hash: Option<String>,
    pub expires_at: NaiveDateTime,
    pub create_user: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct OAuthStateRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub provider_id: i64,
    pub state_hash: String,
    pub redirect_uri: String,
    pub requested_scopes: Value,
    pub code_verifier_hash: Option<String>,
    pub expires_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct ExternalAccountRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub provider_id: i64,
    pub user_id: i64,
    pub external_subject: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub metadata: Value,
    pub status: i16,
}

#[derive(Debug, Clone)]
pub struct IdentityProviderFilter<'a> {
    pub tenant_id: i64,
    pub provider_type: Option<&'a str>,
    pub provider_code: Option<&'a str>,
    pub status: Option<i16>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct IdentityProviderListRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub provider_type: String,
    pub code: String,
    pub name: String,
    pub client_id: Option<String>,
    pub secret_ref: Option<String>,
    pub allowed_domains: Value,
    pub tenant_policy: Value,
    pub status: i16,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct ExternalAccountFilter<'a> {
    pub tenant_id: i64,
    pub provider_code: Option<&'a str>,
    pub status: Option<i16>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct ExternalAccountListRecord {
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
    pub last_login_at: Option<NaiveDateTime>,
    pub status: i16,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct IdentityRepository {
    db: PgPool,
}

impl IdentityRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn find_provider_by_code(
        &self,
        tenant_id: i64,
        code: &str,
    ) -> Result<Option<IdentityProviderRecord>, AppError> {
        let provider = sqlx::query_as::<_, IdentityProviderRecord>(
            r#"
SELECT id, tenant_id, provider_type, code, name, client_id, secret_ref,
       allowed_domains, tenant_policy, status
FROM sys_identity_provider
WHERE tenant_id = $1
  AND code = $2
LIMIT 1;
"#,
        )
        .bind(tenant_id)
        .bind(code)
        .fetch_optional(&self.db)
        .await?;

        Ok(provider)
    }

    pub async fn save_oauth_state(&self, record: &OAuthStateSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO sys_oauth_state
    (id, tenant_id, provider_id, state_hash, redirect_uri, requested_scopes,
     code_verifier_hash, expires_at, create_user, create_time)
VALUES
    ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
ON CONFLICT (state_hash) DO NOTHING;
"#,
        )
        .bind(next_id())
        .bind(record.tenant_id)
        .bind(record.provider_id)
        .bind(&record.state_hash)
        .bind(&record.redirect_uri)
        .bind(&record.requested_scopes)
        .bind(&record.code_verifier_hash)
        .bind(record.expires_at)
        .bind(record.create_user)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn consume_oauth_state(
        &self,
        tenant_id: i64,
        provider_id: i64,
        state_hash: &str,
        redirect_uri: &str,
    ) -> Result<Option<OAuthStateRecord>, AppError> {
        let state = sqlx::query_as::<_, OAuthStateRecord>(
            r#"
UPDATE sys_oauth_state
SET consumed_at = NOW()
WHERE tenant_id = $1
  AND provider_id = $2
  AND state_hash = $3
  AND redirect_uri = $4
  AND consumed_at IS NULL
  AND expires_at > NOW()
RETURNING id, tenant_id, provider_id, state_hash, redirect_uri, requested_scopes,
          code_verifier_hash, expires_at;
"#,
        )
        .bind(tenant_id)
        .bind(provider_id)
        .bind(state_hash)
        .bind(redirect_uri)
        .fetch_optional(&self.db)
        .await?;

        Ok(state)
    }

    pub async fn find_external_account_by_subject(
        &self,
        tenant_id: i64,
        provider_id: i64,
        external_subject: &str,
    ) -> Result<Option<ExternalAccountRecord>, AppError> {
        let account = sqlx::query_as::<_, ExternalAccountRecord>(
            r#"
SELECT id, tenant_id, provider_id, user_id, external_subject, display_name,
       email, metadata, status
FROM sys_external_account
WHERE tenant_id = $1
  AND provider_id = $2
  AND external_subject = $3
  AND status = 1
LIMIT 1;
"#,
        )
        .bind(tenant_id)
        .bind(provider_id)
        .bind(external_subject)
        .fetch_optional(&self.db)
        .await?;

        Ok(account)
    }

    pub async fn touch_external_account_login(
        &self,
        account_id: i64,
        display_name: Option<&str>,
        email: Option<&str>,
        metadata: &Value,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE sys_external_account
SET display_name = COALESCE($2, display_name),
    email = COALESCE($3, email),
    metadata = $4,
    last_login_at = NOW(),
    update_time = NOW()
WHERE id = $1;
"#,
        )
        .bind(account_id)
        .bind(display_name)
        .bind(email)
        .bind(metadata)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn count_identity_providers(
        &self,
        filter: &IdentityProviderFilter<'_>,
    ) -> Result<i64, AppError> {
        let mut query =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM sys_identity_provider AS p");
        query.push(" WHERE 1 = 1");
        push_provider_filters(&mut query, filter);
        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_identity_providers(
        &self,
        filter: &IdentityProviderFilter<'_>,
    ) -> Result<Vec<IdentityProviderListRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
SELECT
    p.id,
    p.tenant_id,
    p.provider_type,
    p.code,
    p.name,
    p.client_id,
    p.secret_ref,
    p.allowed_domains,
    p.tenant_policy,
    p.status,
    p.create_time,
    p.update_time
FROM sys_identity_provider AS p
"#,
        );
        query.push(" WHERE 1 = 1");
        push_provider_filters(&mut query, filter);
        query
            .push(" ORDER BY p.create_time DESC, p.id DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);
        Ok(query
            .build_query_as::<IdentityProviderListRecord>()
            .fetch_all(&self.db)
            .await?)
    }

    pub async fn count_external_accounts(
        &self,
        filter: &ExternalAccountFilter<'_>,
    ) -> Result<i64, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT COUNT(*) FROM sys_external_account AS a \
             JOIN sys_identity_provider AS p ON p.id = a.provider_id AND p.tenant_id = a.tenant_id",
        );
        query.push(" WHERE 1 = 1");
        push_external_account_filters(&mut query, filter);
        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_external_accounts(
        &self,
        filter: &ExternalAccountFilter<'_>,
    ) -> Result<Vec<ExternalAccountListRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
SELECT
    a.id,
    a.tenant_id,
    a.provider_id,
    p.code AS provider_code,
    p.provider_type,
    a.user_id,
    a.external_subject,
    a.display_name,
    a.email,
    a.metadata,
    a.last_login_at,
    a.status,
    a.create_time,
    a.update_time
FROM sys_external_account AS a
JOIN sys_identity_provider AS p ON p.id = a.provider_id AND p.tenant_id = a.tenant_id
"#,
        );
        query.push(" WHERE 1 = 1");
        push_external_account_filters(&mut query, filter);
        query
            .push(" ORDER BY a.create_time DESC, a.id DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);
        Ok(query
            .build_query_as::<ExternalAccountListRecord>()
            .fetch_all(&self.db)
            .await?)
    }
}

fn push_provider_filters(
    query: &mut QueryBuilder<'_, Postgres>,
    filter: &IdentityProviderFilter<'_>,
) {
    query
        .push(" AND p.tenant_id = ")
        .push_bind(filter.tenant_id);
    if let Some(provider_type) = non_empty(filter.provider_type) {
        query
            .push(" AND p.provider_type = ")
            .push_bind(provider_type.to_owned());
    }
    if let Some(provider_code) = non_empty(filter.provider_code) {
        query
            .push(" AND p.code = ")
            .push_bind(provider_code.to_owned());
    }
    if let Some(status) = filter.status.filter(|value| *value > 0) {
        query.push(" AND p.status = ").push_bind(status);
    }
}

fn push_external_account_filters(
    query: &mut QueryBuilder<'_, Postgres>,
    filter: &ExternalAccountFilter<'_>,
) {
    query
        .push(" AND a.tenant_id = ")
        .push_bind(filter.tenant_id);
    if let Some(provider_code) = non_empty(filter.provider_code) {
        query
            .push(" AND p.code = ")
            .push_bind(provider_code.to_owned());
    }
    if let Some(status) = filter.status.filter(|value| *value > 0) {
        query.push(" AND a.status = ").push_bind(status);
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    #[test]
    fn github_identity_provider_seed_is_separate_from_repo_connector() {
        let identity_migration =
            include_str!("../../../migrations/202606060007_seed_github_identity_provider.sql");
        let connector_migration =
            include_str!("../../../migrations/202606060006_create_ai_connector_credential.sql");

        assert!(identity_migration.contains("sys_identity_provider"));
        assert!(identity_migration.contains("'github.login'"));
        assert!(identity_migration.contains("env:GITHUB_OAUTH_CLIENT_SECRET"));
        assert!(connector_migration.contains("ai_connector_credential"));
        assert!(connector_migration.contains("GitHub login remains in sys_identity_provider"));
    }
}
