use novex_ai_core::FoundationModule;

use crate::CRATE_ID;

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Trigger Router",
        "ai-foundation",
        "Webhook, schedule, plugin event, connector event, idempotency, retry, and routing boundaries.",
    )
}
