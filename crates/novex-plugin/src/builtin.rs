use crate::types::{
    PluginCapability, PluginCapabilityKind, PluginManifest, PluginNetworkPolicy, PluginRuntime,
};

pub fn builtin_plugin_manifest(code: &str) -> Option<PluginManifest> {
    match code.trim() {
        "builtin.github-basic" => Some(PluginManifest {
            code: "builtin.github-basic".to_owned(),
            name: "GitHub Basic".to_owned(),
            version: "0.1.0".to_owned(),
            runtime: PluginRuntime::BuiltinAdapter,
            capabilities: vec![
                connector("github.default", "ai:connector:list"),
                tool("github.repo.search", "ai:tool:dryRun"),
                tool("github.repo.read", "ai:tool:dryRun"),
            ],
            permission_grants: permissions(["ai:connector:list", "ai:tool:dryRun"]),
            network: PluginNetworkPolicy::default(),
        }),
        "builtin.media-basic" => Some(PluginManifest {
            code: "builtin.media-basic".to_owned(),
            name: "Media Basic".to_owned(),
            version: "0.1.0".to_owned(),
            runtime: PluginRuntime::BuiltinAdapter,
            capabilities: vec![tool("media.image.generate", "ai:tool:dryRun")],
            permission_grants: permissions(["ai:tool:dryRun"]),
            network: PluginNetworkPolicy::default(),
        }),
        "builtin.agent-tools" => Some(PluginManifest {
            code: "builtin.agent-tools".to_owned(),
            name: "Built-in Agent Tools".to_owned(),
            version: "0.1.0".to_owned(),
            runtime: PluginRuntime::BuiltinAdapter,
            capabilities: vec![
                connector("github.repo", "ai:connector:list"),
                connector("feishu.message", "ai:connector:list"),
                tool("github.repo.search", "ai:tool:dryRun"),
                tool("github.repo.read", "ai:tool:dryRun"),
                tool("media.image.generate", "ai:tool:dryRun"),
                tool("feishu.message.send", "ai:agent:run"),
                trigger("agent.webhook", "ai:trigger:list"),
            ],
            permission_grants: permissions([
                "ai:agent:run",
                "ai:agent:resume",
                "ai:connector:list",
                "ai:tool:dryRun",
                "ai:trigger:list",
            ]),
            network: PluginNetworkPolicy::default(),
        }),
        "builtin.training-pack" => Some(PluginManifest {
            code: "builtin.training-pack".to_owned(),
            name: "Built-in Training Pack".to_owned(),
            version: "0.1.0".to_owned(),
            runtime: PluginRuntime::BuiltinAdapter,
            capabilities: vec![
                skill("training_quiz", "ai:skill:list"),
                skill("training_reminder", "ai:skill:list"),
                connector("feishu.message", "ai:connector:list"),
                tool("rag.search", "ai:knowledge:ask"),
                tool("feishu.message.send", "ai:agent:run"),
                trigger("training.reminder.schedule", "ai:trigger:list"),
            ],
            permission_grants: permissions([
                "ai:agent:run",
                "ai:connector:list",
                "ai:eval:run",
                "ai:knowledge:ask",
                "ai:skill:list",
                "ai:trigger:list",
            ]),
            network: PluginNetworkPolicy::default(),
        }),
        _ => None,
    }
}

fn skill(code: &str, permission_code: &str) -> PluginCapability {
    capability(PluginCapabilityKind::Skill, code, permission_code)
}

fn tool(code: &str, permission_code: &str) -> PluginCapability {
    capability(PluginCapabilityKind::Tool, code, permission_code)
}

fn connector(code: &str, permission_code: &str) -> PluginCapability {
    capability(PluginCapabilityKind::Connector, code, permission_code)
}

fn trigger(code: &str, permission_code: &str) -> PluginCapability {
    capability(PluginCapabilityKind::Trigger, code, permission_code)
}

fn capability(kind: PluginCapabilityKind, code: &str, permission_code: &str) -> PluginCapability {
    PluginCapability {
        kind,
        code: code.to_owned(),
        permission_code: permission_code.to_owned(),
    }
}

fn permissions<const N: usize>(items: [&str; N]) -> Vec<String> {
    items.into_iter().map(ToOwned::to_owned).collect()
}
