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
}
