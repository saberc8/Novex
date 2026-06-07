use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const CRATE_ID: &str = "novex-connectors";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorKind {
    GitHub,
    Feishu,
    Web,
    Database,
    ObjectStorage,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeishuTextMessage {
    pub text: String,
}

impl FeishuTextMessage {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into().trim().to_owned(),
        }
    }

    pub fn to_webhook_payload(&self) -> Value {
        json!({
            "msg_type": "text",
            "content": {
                "text": self.text,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubCodeSearchRequest {
    pub repository: String,
    pub query: String,
    pub path: Option<String>,
    pub limit: usize,
}

impl GitHubCodeSearchRequest {
    pub fn new(repository: impl Into<String>, query: impl Into<String>) -> Self {
        Self {
            repository: repository.into().trim().to_owned(),
            query: query.into().trim().to_owned(),
            path: None,
            limit: 10,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        let path = path.into().trim().trim_start_matches('/').to_owned();
        if !path.is_empty() {
            self.path = Some(path);
        }
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit.clamp(1, 100);
        self
    }

    pub fn rest_path(&self) -> String {
        "/search/code".to_owned()
    }

    pub fn query_pairs(&self) -> Vec<(String, String)> {
        let mut query = format!("{} repo:{}", self.query, self.repository);
        if let Some(path) = self.path.as_deref() {
            query.push_str(" path:");
            query.push_str(path);
        }
        vec![
            ("q".to_owned(), query),
            ("per_page".to_owned(), self.limit.to_string()),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubFileReadRequest {
    pub repository: String,
    pub path: String,
    pub reference: Option<String>,
}

impl GitHubFileReadRequest {
    pub fn new(repository: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            repository: repository.into().trim().to_owned(),
            path: normalize_github_path(path.into()),
            reference: None,
        }
    }

    pub fn with_ref(mut self, reference: impl Into<String>) -> Self {
        let reference = reference.into().trim().to_owned();
        if !reference.is_empty() {
            self.reference = Some(reference);
        }
        self
    }

    pub fn rest_path(&self) -> String {
        format!("/repos/{}/contents/{}", self.repository, self.path)
    }

    pub fn query_pairs(&self) -> Vec<(String, String)> {
        self.reference
            .as_ref()
            .map(|reference| vec![("ref".to_owned(), reference.clone())])
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubCodeSearchItem {
    pub repository: String,
    pub path: String,
    pub name: Option<String>,
    pub html_url: Option<String>,
    pub score: Option<f32>,
}

pub fn parse_github_code_search_response(value: &Value) -> Vec<GitHubCodeSearchItem> {
    value
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(github_code_search_item_from_value)
        .collect()
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

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Connectors",
        "ai-foundation",
        "External resource connector schema, credential scope, datasource sync, and tool adapter boundaries.",
    )
}

fn normalize_github_path(path: String) -> String {
    path.trim()
        .trim_start_matches('/')
        .split('/')
        .filter(|part| !part.is_empty() && *part != "." && *part != "..")
        .collect::<Vec<_>>()
        .join("/")
}

fn github_code_search_item_from_value(value: &Value) -> Option<GitHubCodeSearchItem> {
    let repository = value
        .get("repository")?
        .get("full_name")?
        .as_str()?
        .trim()
        .to_owned();
    let path = value.get("path")?.as_str()?.trim().to_owned();
    if repository.is_empty() || path.is_empty() {
        return None;
    }
    Some(GitHubCodeSearchItem {
        repository,
        path,
        name: value
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        html_url: value
            .get("html_url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        score: value.get("score").and_then(json_f32),
    })
}

fn json_f32(value: &Value) -> Option<f32> {
    if let Some(value) = value.as_f64() {
        return Some(value as f32);
    }
    value.as_str()?.parse::<f32>().ok()
}

fn trim_non_empty_owned(value: String) -> Option<String> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::FoundationStatus;

    #[test]
    fn module_describes_connector_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-connectors");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }

    #[test]
    fn feishu_text_message_builds_custom_bot_payload() {
        let message = FeishuTextMessage::new("Training starts Monday");

        assert_eq!(message.text, "Training starts Monday");
        assert_eq!(
            message.to_webhook_payload(),
            serde_json::json!({
                "msg_type": "text",
                "content": {
                    "text": "Training starts Monday"
                }
            })
        );
    }

    #[test]
    fn github_code_search_request_builds_rest_path_and_query() {
        let request = GitHubCodeSearchRequest::new("acme/app", "parser worker")
            .with_path("src")
            .with_limit(5);

        assert_eq!(request.repository, "acme/app");
        assert_eq!(request.rest_path(), "/search/code");
        assert_eq!(
            request.query_pairs(),
            vec![
                (
                    "q".to_owned(),
                    "parser worker repo:acme/app path:src".to_owned()
                ),
                ("per_page".to_owned(), "5".to_owned())
            ]
        );
    }

    #[test]
    fn github_file_read_request_builds_contents_path_with_ref() {
        let request = GitHubFileReadRequest::new("acme/app", "src/lib.rs").with_ref("main");

        assert_eq!(request.repository, "acme/app");
        assert_eq!(request.path, "src/lib.rs");
        assert_eq!(request.rest_path(), "/repos/acme/app/contents/src/lib.rs");
        assert_eq!(
            request.query_pairs(),
            vec![("ref".to_owned(), "main".to_owned())]
        );
    }

    #[test]
    fn parse_github_code_search_response_maps_items() {
        let response = serde_json::json!({
            "items": [{
                "name": "lib.rs",
                "path": "src/lib.rs",
                "html_url": "https://github.com/acme/app/blob/main/src/lib.rs",
                "repository": {
                    "full_name": "acme/app"
                },
                "score": 12.5
            }]
        });

        let items = parse_github_code_search_response(&response);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].repository, "acme/app");
        assert_eq!(items[0].path, "src/lib.rs");
        assert_eq!(items[0].score, Some(12.5));
    }

    #[test]
    fn connector_credential_selection_prefers_binding_secret_ref_over_env_default() {
        let binding = ConnectorCredentialBinding {
            connector_code: "github.default".to_owned(),
            scope: CredentialScope::Tenant,
            scope_id: "1".to_owned(),
            auth_type: "oauth_app".to_owned(),
            secret_ref: "env:DB_GITHUB_TOKEN".to_owned(),
            scopes: vec!["repo".to_owned()],
        };

        let credential = select_connector_credential(
            Some(&binding),
            &["GITHUB_CONNECTOR_TOKEN"],
            |key| match key {
                "DB_GITHUB_TOKEN" => Some(" db-token ".to_owned()),
                "GITHUB_CONNECTOR_TOKEN" => Some("env-token".to_owned()),
                _ => None,
            },
        )
        .expect("binding credential should resolve");

        assert_eq!(credential.token, "db-token");
        assert_eq!(credential.source, ConnectorCredentialSource::Binding);
        assert_eq!(credential.source.code(), "connector_credential");
        assert_eq!(
            credential.secret_ref.as_deref(),
            Some("env:DB_GITHUB_TOKEN")
        );
    }

    #[test]
    fn connector_credential_selection_falls_back_to_env_when_binding_missing() {
        let credential =
            select_connector_credential(None, &["GITHUB_CONNECTOR_TOKEN"], |key| match key {
                "GITHUB_CONNECTOR_TOKEN" => Some(" env-token ".to_owned()),
                _ => None,
            })
            .expect("env fallback should resolve");

        assert_eq!(credential.token, "env-token");
        assert_eq!(credential.source, ConnectorCredentialSource::Environment);
        assert_eq!(credential.source.code(), "env");
        assert_eq!(credential.secret_ref, None);
    }

    #[test]
    fn connector_credential_selection_falls_back_when_secret_ref_is_unsupported() {
        let binding = ConnectorCredentialBinding {
            connector_code: "github.default".to_owned(),
            scope: CredentialScope::Tenant,
            scope_id: "1".to_owned(),
            auth_type: "oauth_app".to_owned(),
            secret_ref: "vault:github/token".to_owned(),
            scopes: vec!["repo".to_owned()],
        };

        let credential = select_connector_credential(
            Some(&binding),
            &["GITHUB_CONNECTOR_TOKEN"],
            |key| match key {
                "GITHUB_CONNECTOR_TOKEN" => Some(" env-token ".to_owned()),
                _ => None,
            },
        )
        .expect("env fallback should resolve");

        assert_eq!(credential.token, "env-token");
        assert_eq!(credential.source, ConnectorCredentialSource::Environment);
        assert_eq!(credential.secret_ref, None);
    }

    #[test]
    fn credential_scope_code_round_trips_known_scope_values() {
        assert_eq!(
            parse_credential_scope("tenant"),
            Some(CredentialScope::Tenant)
        );
        assert_eq!(parse_credential_scope("user"), Some(CredentialScope::User));
        assert_eq!(credential_scope_code(CredentialScope::App), "app");
        assert_eq!(parse_credential_scope("login"), None);
    }
}
