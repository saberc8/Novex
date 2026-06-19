use novex_ai_core::FoundationModule;

use crate::CRATE_ID;

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Trace Rollout",
        "ai-foundation",
        "Agent trace bundles, replay summaries, rollout snapshots, and eval capture boundaries.",
    )
}
