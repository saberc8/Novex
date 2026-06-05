use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-memory";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    Session,
    User,
    Org,
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryWritePolicy {
    Disabled,
    UserApproved,
    Automatic,
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Memory",
        "ai-foundation",
        "Session, user, organization, project memory policy, retention, and retrieval boundaries.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::FoundationStatus;

    #[test]
    fn module_describes_memory_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-memory");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }
}
