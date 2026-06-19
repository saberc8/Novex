mod credential;
mod feishu;
mod github;
mod kind;
mod module;

pub use credential::{
    credential_scope_code, parse_credential_scope, resolve_env_secret_ref,
    select_connector_credential, ConnectorCredentialBinding, ConnectorCredentialSource,
    CredentialScope, ResolvedConnectorCredential,
};
pub use feishu::FeishuTextMessage;
pub use github::{
    parse_github_code_search_response, GitHubCodeSearchItem, GitHubCodeSearchRequest,
    GitHubFileReadRequest,
};
pub use kind::ConnectorKind;
pub use module::module;

pub const CRATE_ID: &str = "novex-connectors";
