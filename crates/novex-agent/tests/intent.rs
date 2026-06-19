use novex_agent::{route_intent, AgentIntent};

#[test]
fn agent_runtime_routes_training_knowledge_tool_and_handoff_intents() {
    assert_eq!(
        route_intent("员工培训资料什么时候开始?"),
        AgentIntent::RagQuestion
    );
    assert_eq!(
        route_intent("generate a training quiz"),
        AgentIntent::TrainingQuiz
    );
    assert_eq!(
        route_intent("send a Feishu reminder"),
        AgentIntent::ToolTask
    );
    assert_eq!(
        route_intent("I need a human to review this"),
        AgentIntent::HumanHandoff
    );
}
