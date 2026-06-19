use crate::concurrency::ToolConcurrencyPolicy;
use crate::executor::{ToolExecutorBinding, ToolExecutorKind};
use crate::types::{ApprovalPolicy, ToolDefinition, ToolRiskLevel};
use serde_json::json;

pub fn agent_model_loop_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            code: "rag.search".to_owned(),
            name: "Search knowledge".to_owned(),
            description: "Search tenant-scoped knowledge base chunks and return grounded hits."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {"type": "string"},
                    "datasetId": {"type": "integer"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 20}
                }
            }),
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "hits": {"type": "array"},
                    "citations": {"type": "array"},
                    "answer": {"type": "string"}
                }
            })),
            risk_level: ToolRiskLevel::Low,
            approval_policy: ApprovalPolicy::OnRisk,
            permission_code: Some("ai:knowledge:ask".to_owned()),
            concurrency: ToolConcurrencyPolicy::shared(),
        },
        ToolDefinition {
            code: "web.search".to_owned(),
            name: "Search web".to_owned(),
            description: "Search fresh external web results when the run enables web search."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 10}
                }
            }),
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "dryRun": {"type": "boolean"},
                    "status": {"type": "string"},
                    "query": {"type": "string"},
                    "results": {"type": "array"},
                    "message": {"type": "string"}
                }
            })),
            risk_level: ToolRiskLevel::Low,
            approval_policy: ApprovalPolicy::OnRisk,
            permission_code: Some("ai:agent:run".to_owned()),
            concurrency: ToolConcurrencyPolicy::shared(),
        },
        ToolDefinition {
            code: "github.repo.search".to_owned(),
            name: "Search GitHub repository".to_owned(),
            description: "Search code in an authorized GitHub repository or organization."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {"type": "string"},
                    "repository": {"type": "string"},
                    "path": {"type": "string"}
                }
            }),
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "items": {"type": "array"},
                    "toolCode": {"type": "string"}
                }
            })),
            risk_level: ToolRiskLevel::Low,
            approval_policy: ApprovalPolicy::OnRisk,
            permission_code: Some("ai:tool:dryRun".to_owned()),
            concurrency: ToolConcurrencyPolicy::shared(),
        },
        ToolDefinition {
            code: "github.repo.read".to_owned(),
            name: "Read GitHub file".to_owned(),
            description: "Read a file from an authorized GitHub repository.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["repository", "path"],
                "properties": {
                    "repository": {"type": "string"},
                    "path": {"type": "string"},
                    "ref": {"type": "string"}
                }
            }),
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "content": {"type": "string"},
                    "path": {"type": "string"},
                    "sha": {"type": "string"}
                }
            })),
            risk_level: ToolRiskLevel::Low,
            approval_policy: ApprovalPolicy::OnRisk,
            permission_code: Some("ai:tool:dryRun".to_owned()),
            concurrency: ToolConcurrencyPolicy::shared(),
        },
        ToolDefinition {
            code: "media.image.generate".to_owned(),
            name: "Generate image".to_owned(),
            description: "Generate an image asset through the tenant-bound image model route."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["prompt"],
                "properties": {
                    "prompt": {"type": "string"},
                    "size": {"type": "string"},
                    "count": {"type": "integer", "minimum": 1, "maximum": 4}
                }
            }),
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "assetUrl": {"type": "string"},
                    "jobId": {"type": "integer"},
                    "assetId": {"type": "integer"}
                }
            })),
            risk_level: ToolRiskLevel::Medium,
            approval_policy: ApprovalPolicy::OnRisk,
            permission_code: Some("ai:tool:dryRun".to_owned()),
            concurrency: ToolConcurrencyPolicy::exclusive("media:image"),
        },
        ToolDefinition {
            code: "feishu.message.send".to_owned(),
            name: "Send Feishu message".to_owned(),
            description: "Send an audited Feishu message through a tenant connector.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["message"],
                "properties": {
                    "message": {"type": "string"},
                    "recipient": {"type": "string"}
                }
            }),
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "status": {"type": "string"},
                    "dryRun": {"type": "boolean"},
                    "toolCode": {"type": "string"}
                }
            })),
            risk_level: ToolRiskLevel::Medium,
            approval_policy: ApprovalPolicy::OnRisk,
            permission_code: Some("ai:agent:run".to_owned()),
            concurrency: ToolConcurrencyPolicy::exclusive("connector:feishu"),
        },
    ]
}

pub fn agent_model_loop_tool_executor_bindings() -> Vec<ToolExecutorBinding> {
    vec![
        ToolExecutorBinding::new(
            "rag.search",
            "builtin.rag.search",
            ToolExecutorKind::Builtin,
        ),
        ToolExecutorBinding::new(
            "web.search",
            "builtin.web.search",
            ToolExecutorKind::Builtin,
        ),
        ToolExecutorBinding::new(
            "github.repo.search",
            "connector.github.repo.search",
            ToolExecutorKind::Connector,
        ),
        ToolExecutorBinding::new(
            "github.repo.read",
            "connector.github.repo.read",
            ToolExecutorKind::Connector,
        ),
        ToolExecutorBinding::new(
            "media.image.generate",
            "model.media.image.generate",
            ToolExecutorKind::Model,
        )
        .with_background_tasks(),
        ToolExecutorBinding::new(
            "feishu.message.send",
            "connector.feishu.message.send",
            ToolExecutorKind::Connector,
        ),
    ]
}

pub fn customer_service_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            code: "faq.search".to_owned(),
            name: "FAQ Search".to_owned(),
            description:
                "Search tenant-scoped customer service FAQ or policy knowledge with citations."
                    .to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["query", "datasetId"],
                "properties": {
                    "query": {"type": "string"},
                    "datasetId": {"type": "integer"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 10}
                }
            }),
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "answer": {"type": "string"},
                    "hits": {"type": "array"},
                    "citations": {"type": "array"}
                }
            })),
            risk_level: ToolRiskLevel::Low,
            approval_policy: ApprovalPolicy::OnRisk,
            permission_code: Some("ai:customer-service:read".to_owned()),
            concurrency: ToolConcurrencyPolicy::shared(),
        },
        ToolDefinition {
            code: "customer.lookup".to_owned(),
            name: "Customer Lookup".to_owned(),
            description: "Read tenant-scoped customer context needed to answer a support request."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "customerId": {"type": "string"},
                    "externalKey": {"type": "string"}
                },
                "anyOf": [
                    {"required": ["customerId"]},
                    {"required": ["externalKey"]}
                ]
            }),
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "customerId": {"type": "string"},
                    "profile": {"type": "object"},
                    "entitlements": {"type": "array"}
                }
            })),
            risk_level: ToolRiskLevel::Medium,
            approval_policy: ApprovalPolicy::OnRisk,
            permission_code: Some("ai:customer-service:read".to_owned()),
            concurrency: ToolConcurrencyPolicy::exclusive("customer:lookup"),
        },
        ToolDefinition {
            code: "ticket.create".to_owned(),
            name: "Create Support Ticket".to_owned(),
            description: "Create an audited support ticket for a customer after policy approval."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["customerId", "title", "description", "priority"],
                "properties": {
                    "customerId": {"type": "string"},
                    "title": {"type": "string"},
                    "description": {"type": "string"},
                    "priority": {"type": "string", "enum": ["low", "normal", "high", "urgent"]}
                }
            }),
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "ticketId": {"type": "string"},
                    "status": {"type": "string"},
                    "auditId": {"type": "string"}
                }
            })),
            risk_level: ToolRiskLevel::High,
            approval_policy: ApprovalPolicy::Always,
            permission_code: Some("ai:customer-service:ticket".to_owned()),
            concurrency: ToolConcurrencyPolicy::exclusive("customer:write"),
        },
        ToolDefinition {
            code: "handoff.request".to_owned(),
            name: "Request Human Handoff".to_owned(),
            description: "Request a human support handoff with conversation summary and reason."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["conversationId", "reason", "summary"],
                "properties": {
                    "conversationId": {"type": "string"},
                    "reason": {"type": "string"},
                    "summary": {"type": "string"}
                }
            }),
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "handoffId": {"type": "string"},
                    "status": {"type": "string"},
                    "auditId": {"type": "string"}
                }
            })),
            risk_level: ToolRiskLevel::High,
            approval_policy: ApprovalPolicy::Always,
            permission_code: Some("ai:customer-service:handoff".to_owned()),
            concurrency: ToolConcurrencyPolicy::exclusive("customer:write"),
        },
    ]
}
