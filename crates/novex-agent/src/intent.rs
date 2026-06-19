use serde::{Deserialize, Serialize};

use crate::text::contains_any;
use crate::tool_selection::select_tool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentIntent {
    Chat,
    RagQuestion,
    ToolTask,
    CodeSearch,
    TrainingQuiz,
    HumanHandoff,
}

pub fn route_intent(input: &str) -> AgentIntent {
    let normalized = input.to_lowercase();
    if contains_any(
        &normalized,
        &["human", "handoff", "人工", "真人", "转人工", "review"],
    ) {
        AgentIntent::HumanHandoff
    } else if contains_any(
        &normalized,
        &["quiz", "exam", "test", "测验", "考试", "出题"],
    ) {
        AgentIntent::TrainingQuiz
    } else if select_tool(input).is_some() {
        AgentIntent::ToolTask
    } else if contains_any(
        &normalized,
        &["code", "repo", "github", "pull request", "issue", "代码"],
    ) {
        AgentIntent::CodeSearch
    } else if contains_any(
        &normalized,
        &[
            "?",
            "？",
            "search",
            "find",
            "knowledge",
            "handbook",
            "资料",
            "知识库",
            "什么时候",
        ],
    ) {
        AgentIntent::RagQuestion
    } else {
        AgentIntent::Chat
    }
}
