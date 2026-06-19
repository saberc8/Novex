use novex_connectors::{
    credential_scope_code, parse_credential_scope, select_connector_credential,
    ConnectorCredentialBinding, ConnectorCredentialSource, CredentialScope,
};

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
