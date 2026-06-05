use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-mcp";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpServerStatus {
    Registered,
    Discovering,
    Connected,
    Degraded,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDescriptor {
    pub server_id: String,
    pub tool_name: String,
    pub permission_code: String,
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "MCP Gateway",
        "ai-foundation",
        "MCP server registration, tool discovery, tenant authorization, secret, and audit boundaries.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::FoundationStatus;

    #[test]
    fn module_describes_mcp_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-mcp");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }
}
