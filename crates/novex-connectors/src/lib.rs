use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-connectors";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorKind {
    GitHub,
    Feishu,
    Web,
    Database,
    ObjectStorage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialScope {
    Platform,
    Tenant,
    User,
    App,
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Connectors",
        "ai-foundation",
        "External resource connector schema, credential scope, datasource sync, and tool adapter boundaries.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::FoundationStatus;

    #[test]
    fn module_describes_connector_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-connectors");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }
}
