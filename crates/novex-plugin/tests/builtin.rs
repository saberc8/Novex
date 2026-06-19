use novex_plugin::{
    builtin_plugin_manifest, required_plugin_permissions, validate_plugin_manifest,
    PluginCapabilityKind, PluginRuntime,
};

#[test]
fn builtin_plugin_manifest_returns_seeded_agent_tools_pack() {
    let manifest = builtin_plugin_manifest("builtin.agent-tools").unwrap();

    assert_eq!(manifest.runtime, PluginRuntime::BuiltinAdapter);
    assert!(manifest.capabilities.iter().any(|capability| {
        capability.kind == PluginCapabilityKind::Tool && capability.code == "feishu.message.send"
    }));
    assert!(manifest.capabilities.iter().any(|capability| {
        capability.kind == PluginCapabilityKind::Trigger && capability.code == "agent.webhook"
    }));
    assert!(validate_plugin_manifest(&manifest).is_ok());
    assert_eq!(
        required_plugin_permissions(&manifest),
        vec![
            "ai:agent:run".to_owned(),
            "ai:connector:list".to_owned(),
            "ai:tool:dryRun".to_owned(),
            "ai:trigger:list".to_owned(),
        ]
    );
}
