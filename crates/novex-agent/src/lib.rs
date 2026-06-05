use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-agent";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentIntent {
    Chat,
    RagQuestion,
    ToolTask,
    CodeSearch,
    TrainingQuiz,
    HumanHandoff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentLoopKind {
    ReAct,
    Planner,
    SupervisorWorker,
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Agent Runtime",
        "ai-foundation",
        "Intent routing, planning, ReAct loop, tool loop, and run graph orchestration boundaries.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::FoundationStatus;

    #[test]
    fn module_describes_agent_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-agent");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }
}
