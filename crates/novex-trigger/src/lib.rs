use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-trigger";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSourceKind {
    Webhook,
    Schedule,
    PluginEvent,
    ConnectorEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerTargetKind {
    RunGraph,
    AgentRun,
    Job,
    Notification,
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Trigger Router",
        "ai-foundation",
        "Webhook, schedule, plugin event, connector event, idempotency, retry, and routing boundaries.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::FoundationStatus;

    #[test]
    fn module_describes_trigger_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-trigger");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }
}
