use std::collections::BTreeMap;

use novex_ai_core::FoundationModule;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const CRATE_ID: &str = "novex-tools";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Function,
    Http,
    Connector,
    Mcp,
    Sandbox,
    Model,
    Media,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    Never,
    OnRisk,
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionPolicyInput {
    pub tool_code: String,
    pub risk_level: ToolRiskLevel,
    pub approval_policy: ApprovalPolicy,
    pub permission_code: Option<String>,
    pub auto_approved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionPolicyDecision {
    pub tool_code: String,
    pub risk_level: ToolRiskLevel,
    pub permission_code: Option<String>,
    pub requires_approval: bool,
    pub can_execute: bool,
    pub pause_reason: Option<String>,
    pub policy_reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub code: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub risk_level: ToolRiskLevel,
    pub approval_policy: ApprovalPolicy,
    pub permission_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub output_schema: Option<Value>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRouteErrorKind {
    EmptyToolCode,
    DuplicateToolCode,
    UnknownTool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRouteError {
    pub kind: ToolRouteErrorKind,
    pub tool_code: Option<String>,
    pub message: String,
}

impl ToolRouteError {
    fn empty_tool_code() -> Self {
        Self {
            kind: ToolRouteErrorKind::EmptyToolCode,
            tool_code: None,
            message: "tool code is empty".to_owned(),
        }
    }

    fn duplicate_tool_code(code: impl Into<String>) -> Self {
        let code = code.into();
        Self {
            kind: ToolRouteErrorKind::DuplicateToolCode,
            tool_code: Some(code.clone()),
            message: format!("duplicate tool code `{code}`"),
        }
    }

    fn unknown_tool(code: impl Into<String>) -> Self {
        let code = code.into();
        Self {
            kind: ToolRouteErrorKind::UnknownTool,
            tool_code: Some(code.clone()),
            message: format!("model requested unregistered tool `{code}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutedToolCall {
    pub call_id: String,
    pub tool: ToolDefinition,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRouter {
    tools: BTreeMap<String, ToolDefinition>,
}

impl ToolRouter {
    pub fn from_definitions(
        definitions: impl IntoIterator<Item = ToolDefinition>,
    ) -> Result<Self, ToolRouteError> {
        let mut tools = BTreeMap::new();
        for mut definition in definitions {
            let code = definition.code.trim().to_owned();
            if code.is_empty() {
                return Err(ToolRouteError::empty_tool_code());
            }
            if tools.contains_key(&code) {
                return Err(ToolRouteError::duplicate_tool_code(code));
            }
            definition.code = code.clone();
            tools.insert(code, definition);
        }
        Ok(Self { tools })
    }

    pub fn tool_codes(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub fn model_tool_specs(&self) -> Vec<ModelToolSpec> {
        self.tools
            .values()
            .map(ToolDefinition::to_model_tool_spec)
            .collect()
    }

    pub fn route_tool_call(
        &self,
        call_id: impl Into<String>,
        tool_code: impl AsRef<str>,
        arguments: Value,
    ) -> Result<RoutedToolCall, ToolRouteError> {
        let tool_code = tool_code.as_ref().trim();
        if tool_code.is_empty() {
            return Err(ToolRouteError::empty_tool_code());
        }
        let Some(tool) = self.tools.get(tool_code) else {
            return Err(ToolRouteError::unknown_tool(tool_code));
        };
        Ok(RoutedToolCall {
            call_id: call_id.into(),
            tool: tool.clone(),
            arguments,
        })
    }
}

impl ToolDefinition {
    pub fn to_model_tool_spec(&self) -> ModelToolSpec {
        ModelToolSpec {
            name: self.code.clone(),
            description: self.description.clone(),
            parameters: self.input_schema.clone(),
            output_schema: self.output_schema.clone(),
            metadata: serde_json::json!({
                "displayName": self.name,
                "riskLevel": tool_risk_code(self.risk_level),
                "approvalPolicy": approval_policy_code(self.approval_policy),
                "permissionCode": self.permission_code,
            }),
        }
    }
}

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
        },
    ]
}

pub fn tool_risk_code(risk: ToolRiskLevel) -> &'static str {
    match risk {
        ToolRiskLevel::Low => "low",
        ToolRiskLevel::Medium => "medium",
        ToolRiskLevel::High => "high",
    }
}

pub fn approval_policy_code(policy: ApprovalPolicy) -> &'static str {
    match policy {
        ApprovalPolicy::Never => "never",
        ApprovalPolicy::OnRisk => "on_risk",
        ApprovalPolicy::Always => "always",
    }
}

pub fn evaluate_tool_execution_policy(
    input: ToolExecutionPolicyInput,
) -> ToolExecutionPolicyDecision {
    let requires_approval = match input.approval_policy {
        ApprovalPolicy::Always => true,
        ApprovalPolicy::Never => false,
        ApprovalPolicy::OnRisk => {
            matches!(input.risk_level, ToolRiskLevel::High)
                || (matches!(input.risk_level, ToolRiskLevel::Medium) && !input.auto_approved)
        }
    };
    let policy_reason = if matches!(input.risk_level, ToolRiskLevel::High) && requires_approval {
        "high_risk_requires_manual_approval"
    } else if matches!(input.approval_policy, ApprovalPolicy::Always) {
        "approval_policy_always"
    } else if requires_approval {
        "risk_requires_approval"
    } else if input.auto_approved {
        "auto_approved"
    } else {
        "low_risk_allowed"
    }
    .to_owned();

    ToolExecutionPolicyDecision {
        tool_code: input.tool_code,
        risk_level: input.risk_level,
        permission_code: input.permission_code,
        requires_approval,
        can_execute: !requires_approval,
        pause_reason: requires_approval.then(|| "approval".to_owned()),
        policy_reason,
    }
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
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaImageGenerationRequest {
    pub prompt: String,
    pub size: Option<String>,
    pub count: usize,
}

impl MediaImageGenerationRequest {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into().trim().to_owned(),
            size: None,
            count: 1,
        }
    }

    pub fn with_size(mut self, size: impl Into<String>) -> Self {
        let size = size.into().trim().to_owned();
        if !size.is_empty() {
            self.size = Some(size);
        }
        self
    }

    pub fn with_count(mut self, count: usize) -> Self {
        self.count = count.max(1);
        self
    }

    pub fn to_provider_payload(&self) -> Value {
        let mut payload = json!({
            "prompt": self.prompt,
            "n": self.count,
        });
        if let (Some(object), Some(size)) = (payload.as_object_mut(), self.size.as_deref()) {
            object.insert("size".to_owned(), json!(size));
        }
        payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaImageGenerationResult {
    pub asset_url: String,
    pub provider_asset_id: Option<String>,
}

pub fn parse_media_image_generation_response(value: &Value) -> Option<MediaImageGenerationResult> {
    let asset_url = media_image_url(value)?.trim().to_owned();
    if asset_url.is_empty() {
        return None;
    }
    Some(MediaImageGenerationResult {
        asset_url,
        provider_asset_id: media_provider_asset_id(value),
    })
}

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "Tool Registry",
        "ai-foundation",
        "Tool schema, risk, permissions, approval, executor, audit, and replay boundaries.",
    )
}

fn media_image_url(value: &Value) -> Option<&str> {
    value
        .get("imageUrl")
        .or_else(|| value.get("image_url"))
        .or_else(|| value.get("assetUrl"))
        .or_else(|| value.get("asset_url"))
        .or_else(|| value.get("url"))
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("data")?
                .as_array()?
                .first()?
                .get("url")
                .and_then(Value::as_str)
        })
}

fn media_provider_asset_id(value: &Value) -> Option<String> {
    value
        .get("id")
        .or_else(|| value.get("assetId"))
        .or_else(|| value.get("asset_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_ai_core::FoundationStatus;

    #[test]
    fn module_describes_tool_boundary() {
        let module = module();

        assert_eq!(module.id, "novex-tools");
        assert_eq!(module.status, FoundationStatus::Skeleton);
    }

    #[test]
    fn media_image_generation_request_builds_provider_payload() {
        let request = MediaImageGenerationRequest::new("Create a training poster")
            .with_size("1024x1024")
            .with_count(2);

        assert_eq!(request.prompt, "Create a training poster");
        assert_eq!(
            request.to_provider_payload(),
            serde_json::json!({
                "prompt": "Create a training poster",
                "size": "1024x1024",
                "n": 2
            })
        );
    }

    #[test]
    fn parse_media_image_generation_response_extracts_common_url_shapes() {
        let response = serde_json::json!({
            "id": "img-1",
            "data": [{
                "url": "https://cdn.example.com/img-1.png"
            }]
        });

        let result = parse_media_image_generation_response(&response)
            .expect("media image response should expose an asset url");

        assert_eq!(result.asset_url, "https://cdn.example.com/img-1.png");
        assert_eq!(result.provider_asset_id.as_deref(), Some("img-1"));
    }

    #[test]
    fn tool_execution_policy_evaluates_risk_permission_and_auto_approval() {
        let low = evaluate_tool_execution_policy(ToolExecutionPolicyInput {
            tool_code: "github.repo.read".to_owned(),
            risk_level: ToolRiskLevel::Low,
            approval_policy: ApprovalPolicy::OnRisk,
            permission_code: Some("ai:tool:dryRun".to_owned()),
            auto_approved: false,
        });
        assert!(!low.requires_approval);
        assert!(low.can_execute);

        let medium = evaluate_tool_execution_policy(ToolExecutionPolicyInput {
            tool_code: "media.image.generate".to_owned(),
            risk_level: ToolRiskLevel::Medium,
            approval_policy: ApprovalPolicy::OnRisk,
            permission_code: Some("ai:agent:run".to_owned()),
            auto_approved: false,
        });
        assert!(medium.requires_approval);
        assert_eq!(medium.pause_reason.as_deref(), Some("approval"));

        let high = evaluate_tool_execution_policy(ToolExecutionPolicyInput {
            tool_code: "feishu.message.send".to_owned(),
            risk_level: ToolRiskLevel::High,
            approval_policy: ApprovalPolicy::OnRisk,
            permission_code: Some("ai:agent:run".to_owned()),
            auto_approved: true,
        });
        assert!(high.requires_approval);
        assert_eq!(high.policy_reason, "high_risk_requires_manual_approval");
    }

    #[test]
    fn tool_definition_converts_to_model_visible_spec() {
        let tool = ToolDefinition {
            code: "rag.search".to_owned(),
            name: "Search knowledge".to_owned(),
            description: "Search tenant-scoped knowledge base.".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": { "type": "string" }
                }
            }),
            output_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "hits": { "type": "array" }
                }
            })),
            risk_level: ToolRiskLevel::Low,
            approval_policy: ApprovalPolicy::OnRisk,
            permission_code: Some("ai:knowledge:ask".to_owned()),
        };

        let spec = tool.to_model_tool_spec();

        assert_eq!(spec.name, "rag.search");
        assert_eq!(spec.parameters["required"][0], "query");
        assert_eq!(spec.metadata["riskLevel"], "low");
    }

    #[test]
    fn customer_service_tools_have_risk_and_schema_contracts() {
        let tools = customer_service_tool_definitions();

        assert!(tools.iter().any(|tool| tool.code == "faq.search"));
        assert!(tools.iter().any(|tool| tool.code == "customer.lookup"));
        assert!(tools.iter().any(|tool| tool.code == "ticket.create"));
        assert!(tools.iter().any(|tool| tool.code == "handoff.request"));

        let ticket = tools
            .iter()
            .find(|tool| tool.code == "ticket.create")
            .expect("ticket.create tool should exist");
        assert_eq!(ticket.risk_level, ToolRiskLevel::High);
        assert_eq!(ticket.approval_policy, ApprovalPolicy::Always);
        assert_eq!(
            ticket.permission_code.as_deref(),
            Some("ai:customer-service:ticket")
        );
        assert_eq!(ticket.input_schema["required"][0], "customerId");
    }

    #[test]
    fn tool_router_exposes_sorted_model_visible_specs() {
        let router = ToolRouter::from_definitions(vec![
            test_tool_definition("media.image.generate"),
            test_tool_definition("rag.search"),
        ])
        .unwrap();

        assert_eq!(
            router.tool_codes(),
            vec!["media.image.generate".to_owned(), "rag.search".to_owned()]
        );
        assert_eq!(router.model_tool_specs()[0].name, "media.image.generate");
    }

    #[test]
    fn tool_router_rejects_duplicate_tool_codes() {
        let err = ToolRouter::from_definitions(vec![
            test_tool_definition("rag.search"),
            test_tool_definition("rag.search"),
        ])
        .unwrap_err();

        assert_eq!(err.kind, ToolRouteErrorKind::DuplicateToolCode);
        assert_eq!(err.tool_code.as_deref(), Some("rag.search"));
    }

    #[test]
    fn tool_router_rejects_unknown_model_tool_call() {
        let router = ToolRouter::from_definitions(vec![test_tool_definition("rag.search")])
            .expect("router should build from one definition");

        let err = router
            .route_tool_call("call-1", "sandbox.exec", serde_json::json!({}))
            .unwrap_err();

        assert_eq!(err.kind, ToolRouteErrorKind::UnknownTool);
        assert_eq!(err.tool_code.as_deref(), Some("sandbox.exec"));
    }

    #[test]
    fn agent_model_loop_tool_definitions_cover_builtin_agent_tools() {
        let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
            .expect("agent model loop tools should build a router");
        let codes = router.tool_codes();

        assert!(codes.contains(&"rag.search".to_owned()));
        assert!(codes.contains(&"github.repo.search".to_owned()));
        assert!(codes.contains(&"github.repo.read".to_owned()));
        assert!(codes.contains(&"media.image.generate".to_owned()));
        assert!(codes.contains(&"feishu.message.send".to_owned()));
    }

    fn test_tool_definition(code: &str) -> ToolDefinition {
        ToolDefinition {
            code: code.to_owned(),
            name: code.to_owned(),
            description: format!("Tool {code}"),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            output_schema: None,
            risk_level: ToolRiskLevel::Low,
            approval_policy: ApprovalPolicy::OnRisk,
            permission_code: Some("ai:tool:dryRun".to_owned()),
        }
    }
}
