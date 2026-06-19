use novex_ai_core::{FoundationModule, FoundationStatus};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundationSummary {
    pub status: FoundationStatus,
    pub total_modules: usize,
    pub modules: Vec<FoundationModule>,
    pub milestone_coverage: Vec<FoundationMilestoneCoverage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundationMilestoneCoverage {
    pub id: &'static str,
    pub name: &'static str,
    pub status: &'static str,
    pub summary: &'static str,
    pub evidence: Vec<&'static str>,
    pub limitations: Vec<&'static str>,
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
            milestone_coverage: milestone_coverage(),
        }
    }
}

fn milestone_coverage() -> Vec<FoundationMilestoneCoverage> {
    vec![
        FoundationMilestoneCoverage {
            id: "M0",
            name: "Foundation Skeleton",
            status: "poc_ready",
            summary: "AI foundation crates, control-plane boundaries, permissions, menus, and runtime contracts are present.",
            evidence: vec![
                "11 Rust foundation crates",
                "foundation control-plane migration",
                "AI menu and permission seed",
            ],
            limitations: vec![],
        },
        FoundationMilestoneCoverage {
            id: "M1",
            name: "Knowledge Base MVP",
            status: "poc_limited",
            summary: "Knowledge datasets, documents, parser contract, RAG query path, citations, trace, and training/chat-web pages are present.",
            evidence: vec![
                "novex-rag runtime contracts",
                "parser-worker sidecar contracts",
                "Admin knowledge control plane",
                "training-web and chat-web knowledge pages",
            ],
            limitations: vec![
                "Milvus is wired through control-plane and compose metadata, while the live POC path still supports local/vector fallback when production vector storage is unavailable.",
                "External embedding, rerank, and answer models depend on configured model routes.",
            ],
        },
        FoundationMilestoneCoverage {
            id: "M2",
            name: "Skills, Tools, Connectors, Plugins, MCP",
            status: "poc_ready",
            summary: "Configurable capability registries, tool policy, connector credentials, GitHub/Feishu/media POCs, plugins, triggers, and MCP registry are present.",
            evidence: vec![
                "capability registry APIs",
                "tool execution policy and audit",
                "connector credential masking",
                "trigger webhook POC",
                "MCP server registry",
            ],
            limitations: vec![
                "External connector calls require configured credentials or run as deterministic POC adapters.",
            ],
        },
        FoundationMilestoneCoverage {
            id: "M3",
            name: "Agent Runtime",
            status: "poc_ready",
            summary: "Intent routing, context building, shared tool policy selection, run graph state, pause/resume/cancel, events, traces, and C-end workbench views are present.",
            evidence: vec![
                "novex-agent runtime contract",
                "run graph status machine",
                "Agent control plane APIs",
                "agent-workspace run page",
            ],
            limitations: vec![
                "The POC workflow shows task and agent run execution, not a drag-and-drop workflow editor.",
            ],
        },
        FoundationMilestoneCoverage {
            id: "M4",
            name: "Eval",
            status: "poc_ready",
            summary: "Eval datasets, cases, runner, RAG/intent/tool metrics, regression reports, admin reports, and C-end feedback intake are present.",
            evidence: vec![
                "novex-eval metric contracts",
                "eval runtime APIs",
                "eval seeds",
                "training/chat feedback endpoints",
            ],
            limitations: vec![
                "Runtime actuals are deterministic POC adapters until wired to live model/tool executions.",
            ],
        },
        FoundationMilestoneCoverage {
            id: "M5",
            name: "Unified Admin Delivery",
            status: "poc_ready",
            summary: "Customer delivery is handled through the unified Admin control plane: tenants, roles, menus, model routes, knowledge resources, skills, connectors, plugins, triggers, eval sets, and frontend app configuration are managed directly instead of through separate template manifests.",
            evidence: vec![
                "Admin control plane pages",
                "tenant and RBAC configuration",
                "model route configuration",
                "knowledge and agent configuration",
                "skills, tools, connectors, plugins, triggers, and eval configuration",
                "customer-facing frontend apps",
            ],
            limitations: vec!["Deployment remains operator-applied and environment-specific."],
        },
    ]
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

    #[test]
    fn summary_reports_m0_to_m5_milestone_coverage_with_poc_limitations() {
        let summary = FoundationService::summary();
        let milestones = summary
            .milestone_coverage
            .iter()
            .map(|milestone| milestone.id)
            .collect::<Vec<_>>();

        assert_eq!(milestones, vec!["M0", "M1", "M2", "M3", "M4", "M5"]);
        assert!(summary
            .milestone_coverage
            .iter()
            .any(|milestone| milestone.id == "M1"
                && milestone.status == "poc_limited"
                && milestone
                    .limitations
                    .iter()
                    .any(|limitation| limitation.contains("Milvus"))));
        assert!(summary
            .milestone_coverage
            .iter()
            .any(|milestone| milestone.id == "M5"
                && milestone.name == "Unified Admin Delivery"
                && milestone.status == "poc_ready"
                && milestone
                    .evidence
                    .iter()
                    .any(|evidence| evidence.contains("Admin control plane"))
                && milestone
                    .evidence
                    .iter()
                    .any(|evidence| evidence.contains("tenant and RBAC"))
                && milestone
                    .evidence
                    .iter()
                    .any(|evidence| evidence.contains("skills, tools, connectors"))
                && milestone
                    .limitations
                    .iter()
                    .any(|limitation| limitation.contains("operator-applied"))));
    }
}
