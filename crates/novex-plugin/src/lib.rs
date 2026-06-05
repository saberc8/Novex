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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapabilityKind {
    Tool,
    Connector,
    Trigger,
    OAuthClient,
    UiConfig,
    EvalCase,
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
}
