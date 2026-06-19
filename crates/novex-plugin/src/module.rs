use novex_ai_core::FoundationModule;

use crate::CRATE_ID;

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Plugin System",
        "ai-foundation",
        "Plugin manifest, installation, permissions, capabilities, versioning, and tenant enablement boundaries.",
    )
}
