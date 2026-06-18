use novex_tools::{ToolExecutorDispatchPlan, ToolKind};

pub(super) const FEISHU_TOOL_CODE: &str = "feishu.message.send";
pub(super) const MEDIA_IMAGE_TOOL_CODE: &str = "media.image.generate";
pub(super) const GITHUB_REPO_SEARCH_TOOL_CODE: &str = "github.repo.search";
pub(super) const GITHUB_REPO_READ_TOOL_CODE: &str = "github.repo.read";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentToolExecutorSelection {
    Mcp,
    FeishuMessage,
    MediaImage,
    GitHubRepoSearch,
    GitHubRepoRead,
    DryRun,
}

impl AgentToolExecutorSelection {
    pub(super) fn from_dispatch(
        tool_code: &str,
        tool_kind: ToolKind,
        executor_dispatch: Option<&ToolExecutorDispatchPlan>,
    ) -> Self {
        if agent_tool_requires_mcp_lookup(tool_kind, executor_dispatch) {
            return Self::Mcp;
        }

        let executor_code = executor_dispatch.map(|plan| plan.executor_code.as_str());
        match executor_code {
            Some("connector.feishu.message.send") => return Self::FeishuMessage,
            Some("model.media.image.generate") => return Self::MediaImage,
            Some("connector.github.repo.search") => return Self::GitHubRepoSearch,
            Some("connector.github.repo.read") => return Self::GitHubRepoRead,
            _ => {}
        }

        match tool_code {
            FEISHU_TOOL_CODE => Self::FeishuMessage,
            MEDIA_IMAGE_TOOL_CODE => Self::MediaImage,
            GITHUB_REPO_SEARCH_TOOL_CODE => Self::GitHubRepoSearch,
            GITHUB_REPO_READ_TOOL_CODE => Self::GitHubRepoRead,
            _ => Self::DryRun,
        }
    }
}

pub(super) fn agent_tool_requires_github_connector_credential(
    tool_code: &str,
    executor_dispatch: Option<&ToolExecutorDispatchPlan>,
) -> bool {
    executor_dispatch.is_some_and(|plan| {
        plan.requires_connector_credential && plan.executor_code.starts_with("connector.github.")
    }) || matches!(
        tool_code,
        GITHUB_REPO_SEARCH_TOOL_CODE | GITHUB_REPO_READ_TOOL_CODE
    )
}

pub(super) fn agent_tool_requires_mcp_lookup(
    tool_kind: ToolKind,
    executor_dispatch: Option<&ToolExecutorDispatchPlan>,
) -> bool {
    executor_dispatch.is_some_and(|plan| plan.requires_mcp_tool)
        || matches!(tool_kind, ToolKind::Mcp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_tools::{ToolExecutorBinding, ToolExecutorKind};

    fn dispatch_plan(
        tool_code: &str,
        executor_code: &str,
        kind: ToolExecutorKind,
    ) -> ToolExecutorDispatchPlan {
        ToolExecutorDispatchPlan::from_binding(&ToolExecutorBinding::new(
            tool_code,
            executor_code,
            kind,
        ))
    }

    #[test]
    fn agent_tool_executor_selection_prefers_executor_code_and_keeps_legacy_fallbacks() {
        let media_dispatch = dispatch_plan(
            "custom.media.alias",
            "model.media.image.generate",
            ToolExecutorKind::Model,
        );
        assert_eq!(
            AgentToolExecutorSelection::from_dispatch(
                "custom.media.alias",
                ToolKind::Function,
                Some(&media_dispatch),
            ),
            AgentToolExecutorSelection::MediaImage
        );
        assert_eq!(
            AgentToolExecutorSelection::from_dispatch(
                MEDIA_IMAGE_TOOL_CODE,
                ToolKind::Function,
                None,
            ),
            AgentToolExecutorSelection::MediaImage
        );

        let mcp_dispatch =
            dispatch_plan("mcp.docs.search", "mcp.docs.search", ToolExecutorKind::Mcp);
        assert_eq!(
            AgentToolExecutorSelection::from_dispatch(
                "mcp.docs.search",
                ToolKind::Function,
                Some(&mcp_dispatch),
            ),
            AgentToolExecutorSelection::Mcp
        );
        assert_eq!(
            AgentToolExecutorSelection::from_dispatch("unknown.tool", ToolKind::Function, None),
            AgentToolExecutorSelection::DryRun
        );
    }

    #[test]
    fn agent_tool_executor_selection_dependency_helpers_are_targeted() {
        let github_dispatch = dispatch_plan(
            "github.repo.search",
            "connector.github.repo.search",
            ToolExecutorKind::Connector,
        );
        assert!(agent_tool_requires_github_connector_credential(
            "custom.github.alias",
            Some(&github_dispatch),
        ));
        assert!(agent_tool_requires_github_connector_credential(
            GITHUB_REPO_READ_TOOL_CODE,
            None,
        ));

        let feishu_dispatch = dispatch_plan(
            FEISHU_TOOL_CODE,
            "connector.feishu.message.send",
            ToolExecutorKind::Connector,
        );
        assert!(!agent_tool_requires_github_connector_credential(
            FEISHU_TOOL_CODE,
            Some(&feishu_dispatch),
        ));
        assert!(!agent_tool_requires_github_connector_credential(
            "rag.search",
            None,
        ));

        let mcp_dispatch =
            dispatch_plan("mcp.docs.search", "mcp.docs.search", ToolExecutorKind::Mcp);
        assert!(agent_tool_requires_mcp_lookup(
            ToolKind::Function,
            Some(&mcp_dispatch),
        ));
        assert!(agent_tool_requires_mcp_lookup(ToolKind::Mcp, None));
        assert!(!agent_tool_requires_mcp_lookup(ToolKind::Function, None));
    }
}
