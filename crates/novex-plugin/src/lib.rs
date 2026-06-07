use std::collections::BTreeSet;

use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-plugin";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginRuntime {
    HostedHttp,
    McpServer,
    BuiltinAdapter,
}

impl PluginRuntime {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HostedHttp => "hosted_http",
            Self::McpServer => "mcp_server",
            Self::BuiltinAdapter => "builtin_adapter",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapabilityKind {
    Skill,
    Tool,
    Connector,
    Trigger,
    OAuthClient,
    UiConfig,
    EvalCase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCapability {
    pub kind: PluginCapabilityKind,
    pub code: String,
    pub permission_code: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginNetworkPolicy {
    pub allowlist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub code: String,
    pub name: String,
    pub version: String,
    pub runtime: PluginRuntime,
    pub capabilities: Vec<PluginCapability>,
    pub permission_grants: Vec<String>,
    pub network: PluginNetworkPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginManifestError {
    MissingField(&'static str),
    MissingCapabilities,
    CapabilityMissingPermission {
        capability_code: String,
    },
    PermissionNotGranted {
        capability_code: String,
        permission_code: String,
    },
    HostedHttpRequiresNetworkAllowlist,
}

pub fn validate_plugin_manifest(manifest: &PluginManifest) -> Result<(), PluginManifestError> {
    ensure_non_empty("code", &manifest.code)?;
    ensure_non_empty("name", &manifest.name)?;
    ensure_non_empty("version", &manifest.version)?;

    if manifest.capabilities.is_empty() {
        return Err(PluginManifestError::MissingCapabilities);
    }

    if matches!(manifest.runtime, PluginRuntime::HostedHttp)
        && manifest
            .network
            .allowlist
            .iter()
            .all(|host| host.trim().is_empty())
    {
        return Err(PluginManifestError::HostedHttpRequiresNetworkAllowlist);
    }

    let granted_permissions = manifest
        .permission_grants
        .iter()
        .map(|permission| permission.trim())
        .filter(|permission| !permission.is_empty())
        .collect::<BTreeSet<_>>();

    for capability in &manifest.capabilities {
        let capability_code = capability.code.trim();
        ensure_non_empty("capability.code", capability_code)?;
        let permission_code = capability.permission_code.trim();
        if permission_code.is_empty() {
            return Err(PluginManifestError::CapabilityMissingPermission {
                capability_code: capability_code.to_owned(),
            });
        }
        if !granted_permissions.contains(permission_code) {
            return Err(PluginManifestError::PermissionNotGranted {
                capability_code: capability_code.to_owned(),
                permission_code: permission_code.to_owned(),
            });
        }
    }

    Ok(())
}

pub fn required_plugin_permissions(manifest: &PluginManifest) -> Vec<String> {
    manifest
        .capabilities
        .iter()
        .map(|capability| capability.permission_code.trim())
        .filter(|permission| !permission.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

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

fn ensure_non_empty(field: &'static str, value: &str) -> Result<(), PluginManifestError> {
    if value.trim().is_empty() {
        return Err(PluginManifestError::MissingField(field));
    }
    Ok(())
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Plugin System",
        "ai-foundation",
        "Plugin manifest, installation, permissions, capabilities, versioning, and tenant enablement boundaries.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::FoundationStatus;

    #[test]
    fn module_describes_plugin_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-plugin");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }

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
}
