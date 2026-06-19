use std::fs;
use std::path::Path;

use novex_ai_core::FoundationStatus;
use novex_connectors::{
    credential_scope_code, module, parse_credential_scope, parse_github_code_search_response,
    select_connector_credential, ConnectorCredentialBinding, ConnectorCredentialSource,
    ConnectorKind, CredentialScope, FeishuTextMessage, GitHubCodeSearchRequest,
    GitHubFileReadRequest,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_connector_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["credential", "feishu", "github", "kind", "module"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum ConnectorKind",
        "pub struct ConnectorCredentialBinding",
        "pub struct FeishuTextMessage",
        "pub struct GitHubCodeSearchRequest",
        "pub fn parse_github_code_search_response",
        "pub fn select_connector_credential",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn connector_domain_modules_exist() {
    for module in [
        "src/credential.rs",
        "src/feishu.rs",
        "src/github.rs",
        "src/kind.rs",
        "src/module.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_connector_contracts() {
    let module = module();
    assert_eq!(module.id, "novex-connectors");
    assert_eq!(module.status, FoundationStatus::Skeleton);

    let kind = ConnectorKind::GitHub;
    assert_eq!(
        serde_json::to_value(kind).unwrap(),
        serde_json::json!("git_hub")
    );

    let binding = ConnectorCredentialBinding {
        connector_code: "github.default".to_owned(),
        scope: CredentialScope::Tenant,
        scope_id: "1".to_owned(),
        auth_type: "oauth_app".to_owned(),
        secret_ref: "env:GITHUB_TOKEN".to_owned(),
        scopes: vec!["repo".to_owned()],
    };
    let credential = select_connector_credential(Some(&binding), &["FALLBACK"], |key| match key {
        "GITHUB_TOKEN" => Some(" token ".to_owned()),
        _ => None,
    })
    .unwrap();
    assert_eq!(credential.token, "token");
    assert_eq!(credential.source, ConnectorCredentialSource::Binding);
    assert_eq!(credential_scope_code(CredentialScope::Tenant), "tenant");
    assert_eq!(
        parse_credential_scope("tenant"),
        Some(CredentialScope::Tenant)
    );

    let message = FeishuTextMessage::new(" hello ");
    assert_eq!(message.text, "hello");
    assert_eq!(
        message.to_webhook_payload()["content"]["text"],
        serde_json::json!("hello")
    );

    let search = GitHubCodeSearchRequest::new("acme/app", "parser")
        .with_path("/src")
        .with_limit(999);
    assert_eq!(
        search.query_pairs()[1],
        ("per_page".to_owned(), "100".to_owned())
    );

    let read = GitHubFileReadRequest::new("acme/app", "/src/../lib.rs").with_ref("main");
    assert_eq!(read.path, "src/lib.rs");
    assert_eq!(
        read.query_pairs(),
        vec![("ref".to_owned(), "main".to_owned())]
    );

    let items = parse_github_code_search_response(&serde_json::json!({
        "items": [{
            "path": "src/lib.rs",
            "repository": { "full_name": "acme/app" }
        }]
    }));
    assert_eq!(items[0].repository, "acme/app");
}
