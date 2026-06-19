use serde::{Deserialize, Serialize};

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
