use std::collections::{BTreeMap, BTreeSet};

use novex_ai_core::FoundationModule;
use novex_connectors::{GitHubCodeSearchRequest, GitHubFileReadRequest};
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
pub struct AgentToolExecution {
    pub response_payload: Value,
    pub status: String,
    pub dry_run: bool,
    pub error_message: Option<String>,
    pub final_output: String,
}

impl AgentToolExecution {
    pub fn succeeded(response_payload: Value, dry_run: bool, final_output: String) -> Self {
        Self {
            response_payload,
            status: "succeeded".to_owned(),
            dry_run,
            error_message: None,
            final_output,
        }
    }

    pub fn failed(response_payload: Value, error_message: String, final_output: String) -> Self {
        Self {
            response_payload,
            status: "failed".to_owned(),
            dry_run: false,
            error_message: Some(error_message),
            final_output,
        }
    }

    pub fn cancelled(response_payload: Value, final_output: String) -> Self {
        Self {
            response_payload,
            status: "cancelled".to_owned(),
            dry_run: false,
            error_message: Some(final_output.clone()),
            final_output,
        }
    }

    pub fn succeeded_status(&self) -> bool {
        self.status == "succeeded"
    }

    pub fn cancelled_status(&self) -> bool {
        self.status == "cancelled"
    }
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
    #[serde(default)]
    pub concurrency: ToolConcurrencyPolicy,
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
pub enum ToolExecutionLock {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolConcurrencyPolicy {
    pub lock: ToolExecutionLock,
    pub supports_parallel_calls: bool,
    pub waits_for_runtime_cancellation: bool,
    pub exclusive_group: Option<String>,
}

impl ToolConcurrencyPolicy {
    pub fn shared() -> Self {
        Self {
            lock: ToolExecutionLock::Shared,
            supports_parallel_calls: true,
            waits_for_runtime_cancellation: false,
            exclusive_group: None,
        }
    }

    pub fn exclusive(group: impl Into<String>) -> Self {
        let group = group.into().trim().to_owned();
        Self {
            lock: ToolExecutionLock::Exclusive,
            supports_parallel_calls: false,
            waits_for_runtime_cancellation: false,
            exclusive_group: (!group.is_empty()).then_some(group),
        }
    }

    pub fn exclusive_waits_for_runtime_cancellation(group: impl Into<String>) -> Self {
        Self {
            waits_for_runtime_cancellation: true,
            ..Self::exclusive(group)
        }
    }
}

impl Default for ToolConcurrencyPolicy {
    fn default() -> Self {
        Self {
            lock: ToolExecutionLock::Exclusive,
            supports_parallel_calls: false,
            waits_for_runtime_cancellation: false,
            exclusive_group: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolBatchExecutionMode {
    Parallel,
    Serial,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolBatchPlan {
    pub mode: ToolBatchExecutionMode,
    pub calls: Vec<RoutedToolCall>,
    pub serial_reason: Option<String>,
}

impl ToolBatchPlan {
    pub fn from_routed_calls(calls: Vec<RoutedToolCall>) -> Self {
        let mut exclusive_groups = BTreeSet::new();
        for call in &calls {
            let policy = &call.tool.concurrency;
            if let Some(group) = policy.exclusive_group.as_deref() {
                if !exclusive_groups.insert(group.to_owned()) {
                    return Self {
                        mode: ToolBatchExecutionMode::Serial,
                        serial_reason: Some(format!("exclusive_group:{group}")),
                        calls,
                    };
                }
            }
        }

        for call in &calls {
            let policy = &call.tool.concurrency;
            if policy.lock == ToolExecutionLock::Exclusive || !policy.supports_parallel_calls {
                return Self {
                    mode: ToolBatchExecutionMode::Serial,
                    serial_reason: Some(format!("exclusive_tool:{}", call.tool.code)),
                    calls,
                };
            }
        }

        Self {
            mode: ToolBatchExecutionMode::Parallel,
            calls,
            serial_reason: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutorKind {
    Builtin,
    Connector,
    Mcp,
    Model,
    Http,
    Sandbox,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutorBinding {
    pub tool_code: String,
    pub executor_code: String,
    pub kind: ToolExecutorKind,
    pub supports_background_tasks: bool,
    pub waits_for_runtime_cancellation: bool,
}

impl ToolExecutorBinding {
    pub fn new(
        tool_code: impl Into<String>,
        executor_code: impl Into<String>,
        kind: ToolExecutorKind,
    ) -> Self {
        Self {
            tool_code: tool_code.into().trim().to_owned(),
            executor_code: executor_code.into().trim().to_owned(),
            kind,
            supports_background_tasks: false,
            waits_for_runtime_cancellation: false,
        }
    }

    pub fn with_background_tasks(mut self) -> Self {
        self.supports_background_tasks = true;
        self
    }

    pub fn waits_for_runtime_cancellation(mut self) -> Self {
        self.waits_for_runtime_cancellation = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutorDispatchPlan {
    pub tool_code: String,
    pub executor_code: String,
    pub kind: ToolExecutorKind,
    pub requires_connector_credential: bool,
    pub requires_mcp_tool: bool,
    pub requires_model_runtime: bool,
    pub supports_background_tasks: bool,
    pub waits_for_runtime_cancellation: bool,
}

impl ToolExecutorDispatchPlan {
    pub fn from_binding(binding: &ToolExecutorBinding) -> Self {
        Self {
            tool_code: binding.tool_code.trim().to_owned(),
            executor_code: binding.executor_code.trim().to_owned(),
            kind: binding.kind,
            requires_connector_credential: matches!(binding.kind, ToolExecutorKind::Connector),
            requires_mcp_tool: matches!(binding.kind, ToolExecutorKind::Mcp),
            requires_model_runtime: matches!(binding.kind, ToolExecutorKind::Model),
            supports_background_tasks: binding.supports_background_tasks,
            waits_for_runtime_cancellation: binding.waits_for_runtime_cancellation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutorRegistryErrorKind {
    EmptyToolCode,
    EmptyExecutorCode,
    DuplicateToolCode,
    MissingExecutor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutorRegistryError {
    pub kind: ToolExecutorRegistryErrorKind,
    pub tool_code: Option<String>,
    pub executor_code: Option<String>,
    pub message: String,
}

impl ToolExecutorRegistryError {
    fn empty_tool_code() -> Self {
        Self {
            kind: ToolExecutorRegistryErrorKind::EmptyToolCode,
            tool_code: None,
            executor_code: None,
            message: "tool code is empty".to_owned(),
        }
    }

    fn empty_executor_code(tool_code: impl Into<String>) -> Self {
        let tool_code = tool_code.into();
        Self {
            kind: ToolExecutorRegistryErrorKind::EmptyExecutorCode,
            tool_code: Some(tool_code.clone()),
            executor_code: None,
            message: format!("executor code is empty for tool `{tool_code}`"),
        }
    }

    fn duplicate_tool_code(tool_code: impl Into<String>) -> Self {
        let tool_code = tool_code.into();
        Self {
            kind: ToolExecutorRegistryErrorKind::DuplicateToolCode,
            tool_code: Some(tool_code.clone()),
            executor_code: None,
            message: format!("duplicate executor binding for tool `{tool_code}`"),
        }
    }

    fn missing_executor(tool_code: impl Into<String>) -> Self {
        let tool_code = tool_code.into();
        Self {
            kind: ToolExecutorRegistryErrorKind::MissingExecutor,
            tool_code: Some(tool_code.clone()),
            executor_code: None,
            message: format!("tool `{tool_code}` has no registered executor"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutorRegistry {
    bindings: BTreeMap<String, ToolExecutorBinding>,
}

impl ToolExecutorRegistry {
    pub fn from_bindings(
        bindings: impl IntoIterator<Item = ToolExecutorBinding>,
    ) -> Result<Self, ToolExecutorRegistryError> {
        let mut registry = BTreeMap::new();
        for mut binding in bindings {
            binding.tool_code = binding.tool_code.trim().to_owned();
            binding.executor_code = binding.executor_code.trim().to_owned();
            if binding.tool_code.is_empty() {
                return Err(ToolExecutorRegistryError::empty_tool_code());
            }
            if binding.executor_code.is_empty() {
                return Err(ToolExecutorRegistryError::empty_executor_code(
                    binding.tool_code,
                ));
            }
            if registry.contains_key(&binding.tool_code) {
                return Err(ToolExecutorRegistryError::duplicate_tool_code(
                    binding.tool_code,
                ));
            }
            registry.insert(binding.tool_code.clone(), binding);
        }
        Ok(Self { bindings: registry })
    }

    pub fn tool_codes(&self) -> Vec<String> {
        self.bindings.keys().cloned().collect()
    }

    pub fn executor_for(
        &self,
        tool_code: impl AsRef<str>,
    ) -> Result<&ToolExecutorBinding, ToolExecutorRegistryError> {
        let tool_code = tool_code.as_ref().trim();
        if tool_code.is_empty() {
            return Err(ToolExecutorRegistryError::empty_tool_code());
        }
        self.bindings
            .get(tool_code)
            .ok_or_else(|| ToolExecutorRegistryError::missing_executor(tool_code))
    }
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

    pub fn tool_concurrency_policy(&self, tool_code: &str) -> Option<&ToolConcurrencyPolicy> {
        self.tools
            .get(tool_code.trim())
            .map(|tool| &tool.concurrency)
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
                "concurrencyPolicy": self.concurrency,
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

pub fn feishu_message_text_from_tool_input(input: &Value) -> String {
    non_empty_json_string(input.get("message"))
        .or_else(|| non_empty_json_string(input.get("text")))
        .or_else(|| non_empty_json_string(input.get("input")))
        .unwrap_or_else(|| "Novex notification".to_owned())
}

pub fn media_image_request_from_tool_input(input: &Value) -> MediaImageGenerationRequest {
    let prompt = non_empty_json_string(input.get("prompt"))
        .or_else(|| non_empty_json_string(input.get("message")))
        .or_else(|| non_empty_json_string(input.get("input")))
        .or_else(|| non_empty_json_string(input.get("text")))
        .unwrap_or_else(|| "Novex generated image".to_owned());
    let mut request = MediaImageGenerationRequest::new(prompt);
    if let Some(size) = non_empty_json_string(input.get("size")) {
        request = request.with_size(size);
    }
    if let Some(count) = json_usize(input.get("n")).or_else(|| json_usize(input.get("count"))) {
        request = request.with_count(count);
    }
    request
}

pub fn github_search_request_from_tool_input(input: &Value) -> Option<GitHubCodeSearchRequest> {
    let input_text = non_empty_json_string(input.get("input"));
    let repository = github_repository_from_tool_input(input)?;
    let query = non_empty_json_string(input.get("query"))
        .or_else(|| non_empty_json_string(input.get("search")))
        .or_else(|| {
            input_text
                .as_deref()
                .and_then(|text| github_search_query_from_text(text, &repository))
        })
        .or(input_text)?;
    let mut request = GitHubCodeSearchRequest::new(repository, query);
    if let Some(path) = non_empty_json_string(input.get("path")).or_else(|| {
        non_empty_json_string(input.get("input"))
            .as_deref()
            .and_then(github_search_path_from_text)
    }) {
        request = request.with_path(path);
    }
    if let Some(limit) = json_usize(input.get("limit")).or_else(|| json_usize(input.get("perPage")))
    {
        request = request.with_limit(limit);
    }
    Some(request)
}

pub fn github_read_request_from_tool_input(input: &Value) -> Option<GitHubFileReadRequest> {
    let input_text = non_empty_json_string(input.get("input"));
    let repository = github_repository_from_tool_input(input)?;
    let path = non_empty_json_string(input.get("path"))
        .or_else(|| non_empty_json_string(input.get("filePath")))
        .or_else(|| {
            input_text
                .as_deref()
                .and_then(|text| github_read_path_from_text(text, &repository))
        })?;
    let mut request = GitHubFileReadRequest::new(repository, path);
    if let Some(reference) = non_empty_json_string(input.get("ref"))
        .or_else(|| non_empty_json_string(input.get("reference")))
        .or_else(|| non_empty_json_string(input.get("branch")))
        .or_else(|| input_text.as_deref().and_then(github_ref_from_text))
    {
        request = request.with_ref(reference);
    }
    Some(request)
}

fn github_repository_from_tool_input(input: &Value) -> Option<String> {
    non_empty_json_string(input.get("repository"))
        .or_else(|| non_empty_json_string(input.get("repo")))
        .or_else(|| {
            non_empty_json_string(input.get("input"))
                .as_deref()
                .and_then(github_repository_from_text)
        })
        .filter(|value| value.contains('/') && !value.contains(".."))
}

fn github_repository_from_text(text: &str) -> Option<String> {
    github_text_tokens(text)
        .into_iter()
        .find(|token| is_github_repository_token(token))
}

fn github_search_query_from_text(text: &str, repository: &str) -> Option<String> {
    let tokens = github_text_tokens(text);
    let repo_index = tokens.iter().position(|token| token == repository)?;
    let mut start = repo_index + 1;
    if tokens
        .get(start)
        .is_some_and(|token| token.eq_ignore_ascii_case("for"))
    {
        start += 1;
    }
    let mut end = tokens.len();
    for index in start..tokens.len() {
        if tokens[index].eq_ignore_ascii_case("under")
            || tokens[index].eq_ignore_ascii_case("path")
            || (tokens[index].eq_ignore_ascii_case("in")
                && tokens
                    .get(index + 1)
                    .is_some_and(|token| token.eq_ignore_ascii_case("path")))
        {
            end = index;
            break;
        }
    }

    let query = tokens[start..end]
        .iter()
        .filter(|token| !github_search_filler_token(token))
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    if query.is_empty() {
        None
    } else {
        Some(query)
    }
}

fn github_search_path_from_text(text: &str) -> Option<String> {
    let tokens = github_text_tokens(text);
    for (index, token) in tokens.iter().enumerate() {
        if token.eq_ignore_ascii_case("under") || token.eq_ignore_ascii_case("path") {
            return tokens.get(index + 1).cloned();
        }
        if token.eq_ignore_ascii_case("in")
            && tokens
                .get(index + 1)
                .is_some_and(|next| next.eq_ignore_ascii_case("path"))
        {
            return tokens.get(index + 2).cloned();
        }
    }
    None
}

fn github_read_path_from_text(text: &str, repository: &str) -> Option<String> {
    let tokens = github_text_tokens(text);
    let repo_index = tokens.iter().position(|token| token == repository)?;
    for token in tokens.iter().skip(repo_index + 1) {
        if github_ref_keyword(token) {
            return None;
        }
        if token.eq_ignore_ascii_case("file") || token.eq_ignore_ascii_case("path") {
            continue;
        }
        return Some(token.clone());
    }
    None
}

fn github_ref_from_text(text: &str) -> Option<String> {
    let tokens = github_text_tokens(text);
    for (index, token) in tokens.iter().enumerate() {
        if github_ref_keyword(token) {
            return tokens.get(index + 1).cloned();
        }
    }
    None
}

fn github_text_tokens(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|token| {
            let token = token.trim_matches(|ch: char| {
                matches!(
                    ch,
                    ',' | ';' | ':' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
                )
            });
            if token.is_empty() {
                None
            } else {
                Some(token.to_owned())
            }
        })
        .collect()
}

fn is_github_repository_token(token: &str) -> bool {
    let Some((owner, repo)) = token.split_once('/') else {
        return false;
    };
    !owner.is_empty()
        && !repo.is_empty()
        && !owner.contains("..")
        && !repo.contains("..")
        && !owner.contains('/')
        && !repo.contains('/')
}

fn github_search_filler_token(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "search" | "github" | "repo" | "repository" | "code" | "for"
    )
}

fn github_ref_keyword(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "ref" | "reference" | "branch"
    )
}

fn json_usize(value: Option<&Value>) -> Option<usize> {
    let value = value?;
    if let Some(number) = value.as_u64() {
        return Some(number.min(usize::MAX as u64) as usize);
    }
    value.as_str()?.trim().parse::<usize>().ok()
}

fn non_empty_json_string(value: Option<&Value>) -> Option<String> {
    value?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
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
    fn agent_tool_input_feishu_message_text_prefers_explicit_message() {
        assert_eq!(
            feishu_message_text_from_tool_input(&serde_json::json!({
                "message": "Complete training today",
                "input": "ignored"
            })),
            "Complete training today"
        );
        assert_eq!(
            feishu_message_text_from_tool_input(&serde_json::json!({
                "input": "send a Feishu reminder"
            })),
            "send a Feishu reminder"
        );
        assert_eq!(
            feishu_message_text_from_tool_input(&serde_json::json!({})),
            "Novex notification"
        );
    }

    #[test]
    fn agent_tool_input_media_image_request_prefers_prompt_size_and_count() {
        let request = media_image_request_from_tool_input(&serde_json::json!({
            "prompt": "Create a course poster",
            "input": "ignored",
            "size": "1024x1024",
            "count": 2
        }));

        assert_eq!(request.prompt, "Create a course poster");
        assert_eq!(request.size.as_deref(), Some("1024x1024"));
        assert_eq!(request.count, 2);
        assert_eq!(
            request.to_provider_payload(),
            serde_json::json!({
                "prompt": "Create a course poster",
                "n": 2,
                "size": "1024x1024"
            })
        );
    }

    #[test]
    fn agent_tool_input_github_search_accepts_structured_and_natural_language() {
        let structured = github_search_request_from_tool_input(&serde_json::json!({
            "repository": "acme/app",
            "query": "parser worker",
            "path": "src",
            "limit": 5
        }))
        .expect("github search input should be valid");

        assert_eq!(structured.repository, "acme/app");
        assert_eq!(structured.query, "parser worker");
        assert_eq!(structured.path.as_deref(), Some("src"));
        assert_eq!(structured.limit, 5);

        let natural_language = github_search_request_from_tool_input(&serde_json::json!({
            "input": "search GitHub repo acme/app for parser worker under src"
        }))
        .expect("github search natural-language input should be valid");

        assert_eq!(natural_language.repository, "acme/app");
        assert_eq!(natural_language.query, "parser worker");
        assert_eq!(natural_language.path.as_deref(), Some("src"));
    }

    #[test]
    fn agent_tool_input_github_read_accepts_structured_and_natural_language() {
        let structured = github_read_request_from_tool_input(&serde_json::json!({
            "repository": "acme/app",
            "path": "src/lib.rs",
            "ref": "main"
        }))
        .expect("github read input should be valid");

        assert_eq!(structured.repository, "acme/app");
        assert_eq!(structured.path, "src/lib.rs");
        assert_eq!(structured.reference.as_deref(), Some("main"));

        let natural_language = github_read_request_from_tool_input(&serde_json::json!({
            "input": "read GitHub file acme/app src/lib.rs ref main"
        }))
        .expect("github read natural-language input should be valid");

        assert_eq!(natural_language.repository, "acme/app");
        assert_eq!(natural_language.path, "src/lib.rs");
        assert_eq!(natural_language.reference.as_deref(), Some("main"));
    }

    #[test]
    fn agent_tool_execution_envelope_builds_success_failure_and_cancelled_statuses() {
        let succeeded = AgentToolExecution::succeeded(
            serde_json::json!({"status": "succeeded", "answer": "ok"}),
            true,
            "Agent dry-run executed.".to_owned(),
        );
        assert_eq!(succeeded.status, "succeeded");
        assert!(succeeded.dry_run);
        assert_eq!(succeeded.error_message, None);
        assert_eq!(succeeded.final_output, "Agent dry-run executed.");
        assert_eq!(succeeded.response_payload["answer"], "ok");
        assert!(succeeded.succeeded_status());
        assert!(!succeeded.cancelled_status());

        let failed = AgentToolExecution::failed(
            serde_json::json!({"status": "failed", "error": "boom"}),
            "boom".to_owned(),
            "Agent failed.".to_owned(),
        );
        assert_eq!(failed.status, "failed");
        assert!(!failed.dry_run);
        assert_eq!(failed.error_message.as_deref(), Some("boom"));
        assert_eq!(failed.final_output, "Agent failed.");
        assert!(!failed.succeeded_status());
        assert!(!failed.cancelled_status());

        let cancelled = AgentToolExecution::cancelled(
            serde_json::json!({"status": "cancelled"}),
            "Tool was cancelled.".to_owned(),
        );
        assert_eq!(cancelled.status, "cancelled");
        assert!(!cancelled.dry_run);
        assert_eq!(
            cancelled.error_message.as_deref(),
            Some("Tool was cancelled.")
        );
        assert_eq!(cancelled.final_output, "Tool was cancelled.");
        assert!(!cancelled.succeeded_status());
        assert!(cancelled.cancelled_status());
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
            concurrency: ToolConcurrencyPolicy::shared(),
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

    #[test]
    fn tool_executor_registry_routes_known_agent_tools() {
        let registry =
            ToolExecutorRegistry::from_bindings(agent_model_loop_tool_executor_bindings())
                .expect("agent executor registry should build");

        let rag = registry
            .executor_for(" rag.search ")
            .expect("rag.search should have an executor");
        assert_eq!(rag.executor_code, "builtin.rag.search");
        assert_eq!(rag.kind, ToolExecutorKind::Builtin);

        let media = registry
            .executor_for("media.image.generate")
            .expect("media image should have an executor");
        assert_eq!(media.kind, ToolExecutorKind::Model);
        assert!(media.supports_background_tasks);
    }

    #[test]
    fn tool_executor_dispatch_plan_derives_runtime_dependencies() {
        let connector = ToolExecutorDispatchPlan::from_binding(&ToolExecutorBinding::new(
            "github.repo.search",
            "connector.github.repo.search",
            ToolExecutorKind::Connector,
        ));
        assert_eq!(connector.tool_code, "github.repo.search");
        assert_eq!(connector.executor_code, "connector.github.repo.search");
        assert!(connector.requires_connector_credential);
        assert!(!connector.requires_mcp_tool);
        assert!(!connector.requires_model_runtime);

        let model = ToolExecutorDispatchPlan::from_binding(
            &ToolExecutorBinding::new(
                "media.image.generate",
                "model.media.image.generate",
                ToolExecutorKind::Model,
            )
            .with_background_tasks()
            .waits_for_runtime_cancellation(),
        );
        assert!(model.requires_model_runtime);
        assert!(model.supports_background_tasks);
        assert!(model.waits_for_runtime_cancellation);

        let mcp = ToolExecutorDispatchPlan::from_binding(&ToolExecutorBinding::new(
            "mcp.repo.lookup",
            "mcp.repo.lookup",
            ToolExecutorKind::Mcp,
        ));
        assert!(mcp.requires_mcp_tool);
    }

    #[test]
    fn tool_executor_registry_rejects_duplicate_and_missing_bindings() {
        let duplicate = ToolExecutorRegistry::from_bindings(vec![
            ToolExecutorBinding::new(
                "rag.search",
                "builtin.rag.search",
                ToolExecutorKind::Builtin,
            ),
            ToolExecutorBinding::new(
                "rag.search",
                "builtin.rag.search.v2",
                ToolExecutorKind::Builtin,
            ),
        ])
        .unwrap_err();
        assert_eq!(
            duplicate.kind,
            ToolExecutorRegistryErrorKind::DuplicateToolCode
        );

        let missing = ToolExecutorRegistry::default()
            .executor_for("sandbox.exec")
            .unwrap_err();
        assert_eq!(missing.kind, ToolExecutorRegistryErrorKind::MissingExecutor);
        assert_eq!(missing.tool_code.as_deref(), Some("sandbox.exec"));
    }

    #[test]
    fn agent_model_loop_executor_bindings_cover_agent_model_loop_tools() {
        let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
            .expect("agent model loop tools should build a router");
        let registry =
            ToolExecutorRegistry::from_bindings(agent_model_loop_tool_executor_bindings())
                .expect("agent executor registry should build");

        assert_eq!(registry.tool_codes(), router.tool_codes());
    }

    #[test]
    fn tool_router_reports_parallel_policy_for_read_only_tools() {
        let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
            .expect("agent model loop tools should build a router");

        let rag = router.tool_concurrency_policy("rag.search").unwrap();
        assert_eq!(rag.lock, ToolExecutionLock::Shared);
        assert!(rag.supports_parallel_calls);

        let media = router
            .tool_concurrency_policy("media.image.generate")
            .unwrap();
        assert_eq!(media.lock, ToolExecutionLock::Exclusive);
        assert!(!media.supports_parallel_calls);
    }

    #[test]
    fn tool_batch_plan_allows_parallel_read_only_calls() {
        let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
            .expect("agent model loop tools should build a router");
        let calls = vec![
            router
                .route_tool_call(
                    "call-1",
                    "rag.search",
                    serde_json::json!({"query":"policy"}),
                )
                .unwrap(),
            router
                .route_tool_call(
                    "call-2",
                    "github.repo.read",
                    serde_json::json!({"repository":"org/repo","path":"README.md"}),
                )
                .unwrap(),
        ];

        let plan = ToolBatchPlan::from_routed_calls(calls);

        assert_eq!(plan.mode, ToolBatchExecutionMode::Parallel);
        assert_eq!(plan.serial_reason, None);
    }

    #[test]
    fn tool_batch_plan_serializes_non_parallel_calls() {
        let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
            .expect("agent model loop tools should build a router");
        let calls = vec![
            router
                .route_tool_call(
                    "call-1",
                    "rag.search",
                    serde_json::json!({"query":"policy"}),
                )
                .unwrap(),
            router
                .route_tool_call(
                    "call-2",
                    "media.image.generate",
                    serde_json::json!({"prompt":"poster"}),
                )
                .unwrap(),
        ];

        let plan = ToolBatchPlan::from_routed_calls(calls);

        assert_eq!(plan.mode, ToolBatchExecutionMode::Serial);
        assert_eq!(
            plan.serial_reason.as_deref(),
            Some("exclusive_tool:media.image.generate")
        );
    }

    #[test]
    fn tool_batch_plan_serializes_duplicate_exclusive_groups() {
        let mut first = test_tool_definition("connector.write.one");
        first.concurrency = ToolConcurrencyPolicy::exclusive("connector:crm");
        let mut second = test_tool_definition("connector.write.two");
        second.concurrency = ToolConcurrencyPolicy::exclusive("connector:crm");
        let router = ToolRouter::from_definitions(vec![first, second])
            .expect("router should build from exclusive test tools");
        let calls = vec![
            router
                .route_tool_call("call-1", "connector.write.one", serde_json::json!({}))
                .unwrap(),
            router
                .route_tool_call("call-2", "connector.write.two", serde_json::json!({}))
                .unwrap(),
        ];

        let plan = ToolBatchPlan::from_routed_calls(calls);

        assert_eq!(plan.mode, ToolBatchExecutionMode::Serial);
        assert_eq!(
            plan.serial_reason.as_deref(),
            Some("exclusive_group:connector:crm")
        );
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
            concurrency: ToolConcurrencyPolicy::shared(),
        }
    }
}
