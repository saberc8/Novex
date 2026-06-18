use novex_ai_core::TaskBudget;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    application::{
        ai::agent_service::{AgentRunCommand, AgentRunResp, AgentService},
        system::ensure_max_chars,
    },
    shared::error::AppError,
};

const CUSTOMER_SERVICE_MAX_QUESTION_CHARS: usize = 2_000;
const CUSTOMER_SERVICE_MAX_ID_CHARS: usize = 128;
const CUSTOMER_SERVICE_MAX_PROMPT_CHARS: usize = 4_000;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerServiceAgentCommand {
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub customer_id: Option<String>,
    #[serde(default)]
    pub external_key: Option<String>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub dataset_id: Option<i64>,
    #[serde(default)]
    pub allow_ticket: bool,
    #[serde(default)]
    pub allow_handoff: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerServicePolicy {
    pub allow_ticket_create: bool,
    pub allow_handoff_request: bool,
    pub hidden_customer_fields: Vec<String>,
}

impl Default for CustomerServicePolicy {
    fn default() -> Self {
        Self {
            allow_ticket_create: false,
            allow_handoff_request: false,
            hidden_customer_fields: vec![
                "pii".to_owned(),
                "paymentMethod".to_owned(),
                "secretNotes".to_owned(),
            ],
        }
    }
}

pub struct CustomerServiceAgentService {
    agent_service: AgentService,
}

impl CustomerServiceAgentService {
    pub fn for_tenant(db: PgPool, tenant_id: i64) -> Self {
        Self {
            agent_service: AgentService::for_tenant(db, tenant_id),
        }
    }

    pub async fn create_customer_service_run(
        &self,
        user_id: i64,
        command: CustomerServiceAgentCommand,
    ) -> Result<AgentRunResp, AppError> {
        let run_command = customer_service_agent_run_command(command)?;
        self.agent_service.create_run(user_id, run_command).await
    }
}

pub fn customer_service_agent_run_command(
    command: CustomerServiceAgentCommand,
) -> Result<AgentRunCommand, AppError> {
    let command = normalize_customer_service_command(command)?;
    let policy = CustomerServicePolicy {
        allow_ticket_create: command.allow_ticket,
        allow_handoff_request: command.allow_handoff,
        ..Default::default()
    };

    Ok(AgentRunCommand {
        input: customer_service_model_input(&command, &policy)?,
        runtime_mode: Some("model_loop".to_owned()),
        execution_mode: None,
        model_route_id: None,
        auto_approve: false,
        budget: customer_service_task_budget(),
        workbench_context: None,
    })
}

pub fn customer_service_task_budget() -> TaskBudget {
    TaskBudget {
        max_steps: Some(8),
        max_tool_calls: Some(1),
        max_seconds: Some(60),
        max_cost_cents: Some(0),
    }
}

pub fn build_customer_service_system_prompt(policy: &CustomerServicePolicy) -> String {
    format!(
        "You are a customer service agent flow. You must cite retrieved FAQ or knowledge chunks for grounded answers, and say insufficient evidence when no citation supports the answer. Use customer.lookup only for tenant-scoped customer context. Do not create tickets or handoffs without approval and explicit policy allowance. Do not leak hidden customer fields: {}.",
        policy.hidden_customer_fields.join(", ")
    )
}

pub fn customer_service_tool_requires_approval(
    tool_code: &str,
    policy: &CustomerServicePolicy,
) -> bool {
    match tool_code {
        "ticket.create" => policy.allow_ticket_create,
        "handoff.request" => policy.allow_handoff_request,
        _ => false,
    }
}

fn customer_service_model_input(
    command: &CustomerServiceAgentCommand,
    policy: &CustomerServicePolicy,
) -> Result<String, AppError> {
    let input = json!({
        "system": build_customer_service_system_prompt(policy),
        "task": "Answer the customer service request using the allowed tools and policy.",
        "question": command.question,
        "customer": {
            "customerId": command.customer_id,
            "externalKey": command.external_key,
            "conversationId": command.conversation_id,
        },
        "knowledge": {
            "datasetId": command.dataset_id,
            "preferredTool": "faq.search",
        },
        "allowedTools": customer_service_allowed_tools(policy),
        "toolHints": {
            "faq.search": {
                "datasetId": command.dataset_id,
                "query": command.question,
            },
            "customer.lookup": {
                "customerId": command.customer_id,
                "externalKey": command.external_key,
            },
            "ticket.create": {
                "requiresApproval": true,
                "allowed": policy.allow_ticket_create,
            },
            "handoff.request": {
                "requiresApproval": true,
                "allowed": policy.allow_handoff_request,
            }
        }
    })
    .to_string();
    ensure_max_chars(
        "Customer Service Agent 输入",
        &input,
        CUSTOMER_SERVICE_MAX_PROMPT_CHARS,
    )?;
    Ok(input)
}

fn customer_service_allowed_tools(policy: &CustomerServicePolicy) -> Vec<&'static str> {
    let mut tools = vec!["faq.search", "customer.lookup"];
    if policy.allow_ticket_create {
        tools.push("ticket.create");
    }
    if policy.allow_handoff_request {
        tools.push("handoff.request");
    }
    tools
}

fn normalize_customer_service_command(
    mut command: CustomerServiceAgentCommand,
) -> Result<CustomerServiceAgentCommand, AppError> {
    command.question = command.question.trim().to_owned();
    if command.question.is_empty() {
        return Err(AppError::bad_request("客服问题不能为空"));
    }
    ensure_max_chars(
        "客服问题",
        &command.question,
        CUSTOMER_SERVICE_MAX_QUESTION_CHARS,
    )?;
    command.customer_id = normalize_optional_customer_id("客户 ID", command.customer_id)?;
    command.external_key = normalize_optional_customer_id("客户外部 Key", command.external_key)?;
    command.conversation_id = normalize_optional_customer_id("会话 ID", command.conversation_id)?;
    if let Some(dataset_id) = command.dataset_id {
        if dataset_id <= 0 {
            return Err(AppError::bad_request("知识库 ID 不合法"));
        }
    }
    Ok(command)
}

fn normalize_optional_customer_id(
    field_name: &str,
    value: Option<String>,
) -> Result<Option<String>, AppError> {
    let value = value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(value) = &value {
        ensure_max_chars(field_name, value, CUSTOMER_SERVICE_MAX_ID_CHARS)?;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use novex_ai_core::TaskBudget;

    use super::*;

    #[test]
    fn customer_service_prompt_requires_citations_or_insufficient_evidence() {
        let prompt = build_customer_service_system_prompt(&CustomerServicePolicy::default());

        assert!(prompt.contains("cite retrieved FAQ or knowledge chunks"));
        assert!(prompt.contains("insufficient evidence"));
        assert!(prompt.contains("Do not create tickets or handoffs without approval"));
        assert!(prompt.contains("Do not leak hidden customer fields"));
    }

    #[test]
    fn customer_service_flow_routes_faq_question_to_rag_search() {
        let command = CustomerServiceAgentCommand {
            question: "How do refunds work?".to_owned(),
            customer_id: Some("cus_123".to_owned()),
            conversation_id: Some("conv_1".to_owned()),
            dataset_id: Some(7),
            external_key: None,
            allow_ticket: false,
            allow_handoff: false,
        };

        let run = customer_service_agent_run_command(command).expect("command should adapt");

        assert_eq!(run.runtime_mode.as_deref(), Some("model_loop"));
        assert_eq!(run.budget.max_steps, Some(8));
        assert_eq!(run.budget.max_tool_calls, Some(1));
        assert!(run.input.contains("faq.search"));
        assert!(run.input.contains("\"datasetId\":7"));
        assert!(run.input.contains("How do refunds work?"));
    }

    #[test]
    fn customer_service_flow_requires_approval_for_ticket_create() {
        let policy = CustomerServicePolicy {
            allow_ticket_create: true,
            allow_handoff_request: true,
            ..Default::default()
        };

        assert!(customer_service_tool_requires_approval(
            "ticket.create",
            &policy
        ));
        assert!(customer_service_tool_requires_approval(
            "handoff.request",
            &policy
        ));
        assert!(!customer_service_tool_requires_approval(
            "faq.search",
            &policy
        ));
    }

    #[test]
    fn customer_service_agent_budget_matches_poc_runtime_contract() {
        assert_eq!(
            customer_service_task_budget(),
            TaskBudget {
                max_steps: Some(8),
                max_tool_calls: Some(1),
                max_seconds: Some(60),
                max_cost_cents: Some(0),
            }
        );
    }
}
