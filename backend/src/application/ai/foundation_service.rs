use novex_ai_core::{FoundationModule, FoundationStatus};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundationSummary {
    pub status: FoundationStatus,
    pub total_modules: usize,
    pub modules: Vec<FoundationModule>,
}

#[derive(Debug, Clone, Default)]
pub struct FoundationService;

impl FoundationService {
    pub fn summary() -> FoundationSummary {
        let modules = vec![
            novex_ai_core::crate_module(),
            novex_model::module(),
            novex_rag::module(),
            novex_agent::module(),
            novex_tools::module(),
            novex_connectors::module(),
            novex_mcp::module(),
            novex_plugin::module(),
            novex_trigger::module(),
            novex_memory::module(),
            novex_eval::module(),
        ];

        FoundationSummary {
            status: FoundationStatus::Skeleton,
            total_modules: modules.len(),
            modules,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_lists_required_foundation_crates() {
        let summary = FoundationService::summary();
        let ids = summary
            .modules
            .iter()
            .map(|module| module.id)
            .collect::<Vec<_>>();

        assert!(ids.contains(&"novex-ai-core"));
        assert!(ids.contains(&"novex-model"));
        assert!(ids.contains(&"novex-rag"));
        assert!(summary.total_modules >= 11);
    }
}
