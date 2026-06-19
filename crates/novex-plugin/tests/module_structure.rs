use std::fs;
use std::path::Path;

use novex_ai_core::FoundationStatus;
use novex_plugin::{
    builtin_plugin_manifest, module, required_plugin_permissions, validate_plugin_manifest,
    PluginCapability, PluginCapabilityKind, PluginManifest, PluginManifestError,
    PluginNetworkPolicy, PluginRuntime,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_plugin_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["builtin", "module", "types", "validation"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum PluginRuntime",
        "pub struct PluginManifest",
        "pub enum PluginManifestError",
        "pub fn validate_plugin_manifest",
        "pub fn builtin_plugin_manifest",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn plugin_domain_modules_exist() {
    for module in [
        "src/builtin.rs",
        "src/module.rs",
        "src/types.rs",
        "src/validation.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_plugin_contracts() {
    let module = module();
    assert_eq!(module.id, "novex-plugin");
    assert_eq!(module.status, FoundationStatus::Skeleton);
    assert_eq!(PluginRuntime::HostedHttp.as_str(), "hosted_http");

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
        network: PluginNetworkPolicy {
            allowlist: vec!["api.example.com".to_owned()],
        },
    };
    validate_plugin_manifest(&manifest).unwrap();
    assert_eq!(
        required_plugin_permissions(&manifest),
        vec!["ai:tool:dryRun".to_owned()]
    );

    let builtin = builtin_plugin_manifest("builtin.github-basic").unwrap();
    assert_eq!(builtin.runtime, PluginRuntime::BuiltinAdapter);

    let mut invalid = manifest;
    invalid.permission_grants.clear();
    assert_eq!(
        validate_plugin_manifest(&invalid).unwrap_err(),
        PluginManifestError::PermissionNotGranted {
            capability_code: "external.webhook.post".to_owned(),
            permission_code: "ai:tool:dryRun".to_owned(),
        }
    );
}
