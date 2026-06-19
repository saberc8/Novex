use novex_plugin::{
    required_plugin_permissions, validate_plugin_manifest, PluginCapability, PluginCapabilityKind,
    PluginManifest, PluginManifestError, PluginNetworkPolicy, PluginRuntime,
};

#[test]
fn plugin_manifest_validates_capability_permissions_and_network_policy() {
    let manifest = PluginManifest {
        code: "builtin.github-basic".to_owned(),
        name: "GitHub Basic".to_owned(),
        version: "0.1.0".to_owned(),
        runtime: PluginRuntime::HostedHttp,
        capabilities: vec![
            PluginCapability {
                kind: PluginCapabilityKind::Connector,
                code: "github.default".to_owned(),
                permission_code: "ai:connector:list".to_owned(),
            },
            PluginCapability {
                kind: PluginCapabilityKind::Tool,
                code: "github.repo.search".to_owned(),
                permission_code: "ai:tool:dryRun".to_owned(),
            },
        ],
        permission_grants: vec![
            "ai:connector:list".to_owned(),
            "ai:tool:dryRun".to_owned(),
            "ai:tool:dryRun".to_owned(),
        ],
        network: PluginNetworkPolicy {
            allowlist: vec!["api.github.com".to_owned()],
        },
    };

    validate_plugin_manifest(&manifest).expect("manifest should be valid");

    assert_eq!(
        required_plugin_permissions(&manifest),
        vec!["ai:connector:list".to_owned(), "ai:tool:dryRun".to_owned()]
    );
}

#[test]
fn plugin_manifest_rejects_capability_without_permission_grant() {
    let manifest = PluginManifest {
        code: "builtin.training-pack".to_owned(),
        name: "Training Pack".to_owned(),
        version: "0.1.0".to_owned(),
        runtime: PluginRuntime::BuiltinAdapter,
        capabilities: vec![PluginCapability {
            kind: PluginCapabilityKind::Trigger,
            code: "training.reminder.schedule".to_owned(),
            permission_code: "ai:trigger:list".to_owned(),
        }],
        permission_grants: vec!["ai:agent:run".to_owned()],
        network: PluginNetworkPolicy::default(),
    };

    let err = validate_plugin_manifest(&manifest).unwrap_err();

    assert_eq!(
        err,
        PluginManifestError::PermissionNotGranted {
            capability_code: "training.reminder.schedule".to_owned(),
            permission_code: "ai:trigger:list".to_owned(),
        }
    );
}

#[test]
fn plugin_manifest_rejects_hosted_http_without_network_allowlist() {
    let manifest = PluginManifest {
        code: "external.webhook-tool".to_owned(),
        name: "External Webhook Tool".to_owned(),
        version: "0.1.0".to_owned(),
        runtime: PluginRuntime::HostedHttp,
        capabilities: vec![PluginCapability {
            kind: PluginCapabilityKind::Tool,
            code: "external.webhook.post".to_owned(),
            permission_code: "ai:tool:dryRun".to_owned(),
        }],
        permission_grants: vec!["ai:tool:dryRun".to_owned()],
        network: PluginNetworkPolicy::default(),
    };

    let err = validate_plugin_manifest(&manifest).unwrap_err();

    assert_eq!(err, PluginManifestError::HostedHttpRequiresNetworkAllowlist);
}
