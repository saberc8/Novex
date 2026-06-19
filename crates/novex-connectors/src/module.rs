use novex_ai_core::FoundationModule;

use crate::CRATE_ID;

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Connectors",
        "ai-foundation",
        "External resource connector schema, credential scope, datasource sync, and tool adapter boundaries.",
    )
}
