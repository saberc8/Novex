use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialScope {
    Platform,
    Tenant,
    User,
    App,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorCredentialBinding {
    pub connector_code: String,
    pub scope: CredentialScope,
    pub scope_id: String,
    pub auth_type: String,
    pub secret_ref: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorCredentialSource {
    Binding,
    Environment,
}

impl ConnectorCredentialSource {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Binding => "connector_credential",
            Self::Environment => "env",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedConnectorCredential {
    pub token: String,
    pub source: ConnectorCredentialSource,
    pub secret_ref: Option<String>,
}

pub fn parse_credential_scope(value: &str) -> Option<CredentialScope> {
    Some(match value.trim() {
        "platform" => CredentialScope::Platform,
        "tenant" => CredentialScope::Tenant,
        "user" => CredentialScope::User,
        "app" => CredentialScope::App,
        _ => return None,
    })
}

pub const fn credential_scope_code(scope: CredentialScope) -> &'static str {
    match scope {
        CredentialScope::Platform => "platform",
        CredentialScope::Tenant => "tenant",
        CredentialScope::User => "user",
        CredentialScope::App => "app",
    }
}

pub fn select_connector_credential<F>(
    binding: Option<&ConnectorCredentialBinding>,
    fallback_env_keys: &[&str],
    mut env_get: F,
) -> Option<ResolvedConnectorCredential>
where
    F: FnMut(&str) -> Option<String>,
{
    if let Some(binding) = binding {
        if let Some(token) = resolve_env_secret_ref(&binding.secret_ref, &mut env_get) {
            return Some(ResolvedConnectorCredential {
                token,
                source: ConnectorCredentialSource::Binding,
                secret_ref: Some(binding.secret_ref.clone()),
            });
        }
    }

    for env_key in fallback_env_keys {
        let env_key = env_key.trim();
        if env_key.is_empty() {
            continue;
        }
        if let Some(token) = env_get(env_key).and_then(trim_non_empty_owned) {
            return Some(ResolvedConnectorCredential {
                token,
                source: ConnectorCredentialSource::Environment,
                secret_ref: None,
            });
        }
    }

    None
}

pub fn resolve_env_secret_ref<F>(secret_ref: &str, env_get: &mut F) -> Option<String>
where
    F: FnMut(&str) -> Option<String>,
{
    let env_key = secret_ref.trim().strip_prefix("env:")?.trim();
    if env_key.is_empty() {
        return None;
    }
    env_get(env_key).and_then(trim_non_empty_owned)
}

fn trim_non_empty_owned(value: String) -> Option<String> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}
