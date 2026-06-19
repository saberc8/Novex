use novex_ai_core::FoundationModule;

use crate::CRATE_ID;

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Memory",
        "ai-foundation",
        "Session, user, organization, project memory policy, retention, and retrieval boundaries.",
    )
}
