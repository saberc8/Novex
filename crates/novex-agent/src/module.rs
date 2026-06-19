use novex_ai_core::FoundationModule;

use crate::CRATE_ID;

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Agent Runtime",
        "ai-foundation",
        "Intent routing, planning, ReAct loop, tool loop, and run graph orchestration boundaries.",
    )
}
