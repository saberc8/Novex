use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-ai-core";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundationStatus {
    Skeleton,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundationModule {
    pub id: &'static str,
    pub name: &'static str,
    pub layer: &'static str,
    pub status: FoundationStatus,
    pub description: &'static str,
}

impl FoundationModule {
    pub const fn skeleton(
        id: &'static str,
        name: &'static str,
        layer: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            id,
            name,
            layer,
            status: FoundationStatus::Skeleton,
            description,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TenantContext {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub role_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRef {
    pub resource_type: String,
    pub resource_id: String,
    pub tenant_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    WaitingApproval,
    Paused,
    Resuming,
    Cancelling,
    Cancelled,
    Failed,
    Succeeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStepType {
    ModelCall,
    Retrieval,
    Rerank,
    ToolCall,
    Approval,
    HumanInput,
    ConnectorSync,
    MediaJob,
}

pub fn crate_module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "AI Core",
        "foundation",
        "Shared tenant, resource, run graph, trace, and policy contracts.",
    )
}

pub fn foundation_modules() -> Vec<FoundationModule> {
    vec![
        FoundationModule::skeleton(
            "tenant-context",
            "Tenant Context",
            "novex-ai-core",
            "Tenant and caller context passed through AI foundation modules.",
        ),
        FoundationModule::skeleton(
            "resource-ref",
            "Resource Reference",
            "novex-ai-core",
            "Stable references for tenant-scoped AI assets and run artifacts.",
        ),
        FoundationModule::skeleton(
            "run-graph",
            "Run Graph",
            "novex-ai-core",
            "Shared run, step, status, pause, cancel, replay, and event boundaries.",
        ),
        FoundationModule::skeleton(
            "trace",
            "Trace",
            "novex-ai-core",
            "Trace, cost, usage, latency, and replay metadata boundary.",
        ),
        FoundationModule::skeleton(
            "policy",
            "Policy",
            "novex-ai-core",
            "Permission, approval, network zone, and execution policy boundary.",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foundation_modules_describe_m0_skeleton_boundaries() {
        let modules = foundation_modules();

        assert!(modules.iter().any(|module| module.id == "run-graph"));
        assert!(modules.iter().any(|module| module.id == "policy"));
        assert!(modules
            .iter()
            .all(|module| module.status == FoundationStatus::Skeleton));
    }
}
