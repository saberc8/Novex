use std::{env, time::Duration};

use novex_connectors::{
    parse_credential_scope, parse_github_code_search_response, resolve_env_secret_ref,
    select_connector_credential, ConnectorCredentialBinding, FeishuTextMessage,
    ResolvedConnectorCredential,
};
use novex_mcp::{McpStreamableHttpRequestPlan, McpToolInvocationRequest, McpToolInvocationResult};
use novex_model::ModelRoutePurpose;
use novex_tools::{
    feishu_message_text_from_tool_input, github_read_request_from_tool_input,
    github_search_request_from_tool_input, media_image_request_from_tool_input, AgentToolExecution,
    ToolExecutorDispatchPlan, ToolKind,
};
use serde_json::{json, Value};

use super::model_service::ModelRuntimeService;
use crate::infrastructure::persistence::ai_capability_repository::{
    ConnectorCredentialLookupRecord, McpToolExecutionRecord, ToolLookupRecord,
};

const FEISHU_WEBHOOK_TIMEOUT: Duration = Duration::from_secs(10);
const GITHUB_CONNECTOR_TIMEOUT: Duration = Duration::from_secs(15);

type GitHubConnectorAuth = ResolvedConnectorCredential;

pub(super) const FEISHU_TOOL_CODE: &str = "feishu.message.send";
pub(super) const MEDIA_IMAGE_TOOL_CODE: &str = "media.image.generate";
pub(super) const GITHUB_REPO_SEARCH_TOOL_CODE: &str = "github.repo.search";
pub(super) const GITHUB_REPO_READ_TOOL_CODE: &str = "github.repo.read";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FeishuWebhookConfig {
    pub(super) webhook_url: String,
}

impl FeishuWebhookConfig {
    fn from_env() -> Option<Self> {
        Self::from_env_map(|key| env::var(key).ok())
    }

    pub(super) fn from_env_map<F>(mut env_get: F) -> Option<Self>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let webhook_url = env_get("FEISHU_WEBHOOK_URL")
            .or_else(|| env_get("NOVEX_FEISHU_WEBHOOK_URL"))
            .map(|value| value.trim().trim_end_matches('/').to_owned())
            .filter(|value| !value.is_empty())?;

        Some(Self { webhook_url })
    }
}

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

pub(super) fn agent_tool_kind(tool: &ToolLookupRecord) -> ToolKind {
    let executor = tool.executor_kind.trim().to_ascii_lowercase();
    let kind = tool.tool_kind.trim().to_ascii_lowercase();
    match executor.as_str() {
        "mcp" => ToolKind::Mcp,
        "connector" => ToolKind::Connector,
        "model" => ToolKind::Model,
        "media" => ToolKind::Media,
        "sandbox" => ToolKind::Sandbox,
        "http" => ToolKind::Http,
        _ => match kind.as_str() {
            "mcp" => ToolKind::Mcp,
            "connector" => ToolKind::Connector,
            "media" => ToolKind::Media,
            "model" => ToolKind::Model,
            "http" => ToolKind::Http,
            _ => ToolKind::Function,
        },
    }
}

pub(super) async fn execute_agent_tool(
    tool: &ToolLookupRecord,
    input: &Value,
    connector_credential: Option<&ConnectorCredentialLookupRecord>,
    mcp_tool: Option<&McpToolExecutionRecord>,
    executor_dispatch: Option<&ToolExecutorDispatchPlan>,
    model_runtime: Option<&ModelRuntimeService>,
) -> AgentToolExecution {
    let tool_code = tool.code.as_str();
    match AgentToolExecutorSelection::from_dispatch(
        tool_code,
        agent_tool_kind(tool),
        executor_dispatch,
    ) {
        AgentToolExecutorSelection::Mcp => {
            return execute_mcp_tool(&tool.code, input, mcp_tool).await;
        }
        AgentToolExecutorSelection::FeishuMessage => {
            return execute_feishu_message_tool(input).await;
        }
        AgentToolExecutorSelection::MediaImage => {
            return execute_media_image_tool(input, model_runtime).await;
        }
        AgentToolExecutorSelection::GitHubRepoSearch => {
            return execute_github_repo_search_tool(input, connector_credential).await;
        }
        AgentToolExecutorSelection::GitHubRepoRead => {
            return execute_github_repo_read_tool(input, connector_credential).await;
        }
        AgentToolExecutorSelection::DryRun => {}
    }

    AgentToolExecution::succeeded(
        json!({
            "dryRun": true,
            "toolCode": tool_code,
            "status": "succeeded",
            "inputEcho": input,
            "message": "agent dry-run only; no external side effects"
        }),
        true,
        format!("Agent dry-run executed {tool_code}."),
    )
}

async fn execute_mcp_tool(
    tool_code: &str,
    input: &Value,
    mcp_tool: Option<&McpToolExecutionRecord>,
) -> AgentToolExecution {
    let Some(tool) = mcp_tool else {
        return AgentToolExecution::failed(
            json!({
                "dryRun": false,
                "toolCode": tool_code,
                "status": "failed",
                "provider": "mcp",
                "error": "MCP tool registration not found",
            }),
            "MCP tool registration not found".to_owned(),
            "Agent failed to execute MCP tool.".to_owned(),
        );
    };

    let request = McpToolInvocationRequest {
        server_code: tool.server_code.clone(),
        tool_name: tool.tool_name.clone(),
        arguments: input.clone(),
    };
    let auth = mcp_auth_payload(tool.secret_ref.as_deref(), &tool.auth_type);
    if let Some(mock_response) = tool.metadata.get("mockResponse").cloned() {
        let result = McpToolInvocationResult {
            tool_code: tool.tool_code.clone(),
            status: "succeeded".to_owned(),
            output: mock_response,
            dry_run: false,
        };
        return AgentToolExecution::succeeded(
            json!({
                "dryRun": result.dry_run,
                "toolCode": result.tool_code,
                "status": result.status,
                "provider": "mcp",
                "server": mcp_server_payload(tool),
                "request": request,
                "response": result.output,
                "auth": auth,
                "mocked": true,
            }),
            result.dry_run,
            format!(
                "Agent executed MCP tool {} via configured mock response.",
                tool.tool_code
            ),
        );
    }

    let result = McpToolInvocationResult {
        tool_code: tool.tool_code.clone(),
        status: "succeeded".to_owned(),
        output: json!({
            "message": "MCP live client is not configured; dry-run only",
            "endpointUrl": tool.endpoint_url,
            "serverCode": tool.server_code,
            "toolName": tool.tool_name,
            "arguments": input,
        }),
        dry_run: true,
    };
    let live_request = mcp_streamable_http_request_payload(tool, &request);
    AgentToolExecution::succeeded(
        json!({
            "dryRun": result.dry_run,
            "toolCode": result.tool_code,
            "status": result.status,
            "provider": "mcp",
            "server": mcp_server_payload(tool),
            "request": request,
            "liveRequest": live_request,
            "response": result.output,
            "auth": auth,
            "mocked": false,
        }),
        result.dry_run,
        format!("Agent dry-run prepared MCP tool {}.", tool.tool_code),
    )
}

fn mcp_streamable_http_request_payload(
    tool: &McpToolExecutionRecord,
    request: &McpToolInvocationRequest,
) -> Value {
    let Some(endpoint_url) = tool.endpoint_url.as_deref() else {
        return Value::Null;
    };
    if !matches!(tool.transport_kind.as_str(), "streamable_http" | "sse") {
        return Value::Null;
    }

    McpStreamableHttpRequestPlan::tools_call(
        endpoint_url,
        format!("mcp-tool-{}", tool.id),
        request,
        tool.secret_ref.as_deref(),
    )
    .sanitized_evidence()
}

fn mcp_server_payload(tool: &McpToolExecutionRecord) -> Value {
    json!({
        "serverId": tool.server_id,
        "serverCode": tool.server_code,
        "serverName": tool.server_name,
        "endpointUrl": tool.endpoint_url,
        "transportKind": tool.transport_kind,
        "authType": tool.auth_type,
    })
}

fn mcp_auth_payload(secret_ref: Option<&str>, auth_type: &str) -> Value {
    mcp_auth_payload_from_sources(secret_ref, auth_type, |key| env::var(key).ok())
}

pub(super) fn mcp_auth_payload_from_sources<F>(
    secret_ref: Option<&str>,
    auth_type: &str,
    mut env_get: F,
) -> Value
where
    F: FnMut(&str) -> Option<String>,
{
    let resolved = secret_ref
        .and_then(|secret_ref| resolve_env_secret_ref(secret_ref, &mut env_get))
        .is_some();
    json!({
        "type": auth_type,
        "secretRef": secret_ref,
        "resolved": resolved,
    })
}

async fn execute_github_repo_search_tool(
    input: &Value,
    connector_credential: Option<&ConnectorCredentialLookupRecord>,
) -> AgentToolExecution {
    let Some(request) = github_search_request_from_tool_input(input) else {
        return AgentToolExecution::failed(
            json!({
                "dryRun": false,
                "toolCode": GITHUB_REPO_SEARCH_TOOL_CODE,
                "status": "failed",
                "provider": "github",
                "inputEcho": input,
                "error": "GitHub repository and query are required",
            }),
            "GitHub repository and query are required".to_owned(),
            "Agent failed to search GitHub repository.".to_owned(),
        );
    };
    let request_payload = json!({
        "repository": request.repository,
        "query": request.query,
        "path": request.path,
        "limit": request.limit,
    });
    let Some(auth) = github_connector_auth(connector_credential) else {
        return AgentToolExecution::succeeded(
            json!({
                "dryRun": true,
                "toolCode": GITHUB_REPO_SEARCH_TOOL_CODE,
                "status": "succeeded",
                "provider": "github",
                "requestPayload": request_payload,
                "message": "GitHub connector credential not configured; dry-run only"
            }),
            true,
            "Agent dry-run prepared GitHub repo search.".to_owned(),
        );
    };

    let client = match github_http_client() {
        Ok(client) => client,
        Err(execution) => return execution,
    };
    let response = match client
        .get(github_api_url(&request.rest_path()))
        .query(&request.query_pairs())
        .bearer_auth(&auth.token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            let error = format!("GitHub repo search failed: {err}");
            return AgentToolExecution::failed(
                json!({
                    "dryRun": false,
                    "toolCode": GITHUB_REPO_SEARCH_TOOL_CODE,
                    "status": "failed",
                    "provider": "github",
                    "requestPayload": request_payload,
                    "error": error,
                }),
                error,
                "Agent failed to search GitHub repository.".to_owned(),
            );
        }
    };

    let status = response.status();
    let provider_payload = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !status.is_success() {
        let error = format!("GitHub repo search failed: HTTP {}", status.as_u16());
        return AgentToolExecution::failed(
            json!({
                "dryRun": false,
                "toolCode": GITHUB_REPO_SEARCH_TOOL_CODE,
                "status": "failed",
                "provider": "github",
                "requestPayload": request_payload,
                "response": provider_payload,
                "error": error,
            }),
            error,
            "Agent failed to search GitHub repository.".to_owned(),
        );
    }

    let items = parse_github_code_search_response(&provider_payload);
    AgentToolExecution::succeeded(
        json!({
            "dryRun": false,
            "toolCode": GITHUB_REPO_SEARCH_TOOL_CODE,
            "status": "succeeded",
            "provider": "github",
            "credentialSource": auth.source.code(),
            "credentialSecretRef": auth.secret_ref,
            "requestPayload": request_payload,
            "items": items,
            "response": provider_payload,
        }),
        false,
        format!("Agent found {} GitHub code result(s).", items.len()),
    )
}

async fn execute_github_repo_read_tool(
    input: &Value,
    connector_credential: Option<&ConnectorCredentialLookupRecord>,
) -> AgentToolExecution {
    let Some(request) = github_read_request_from_tool_input(input) else {
        return AgentToolExecution::failed(
            json!({
                "dryRun": false,
                "toolCode": GITHUB_REPO_READ_TOOL_CODE,
                "status": "failed",
                "provider": "github",
                "inputEcho": input,
                "error": "GitHub repository and path are required",
            }),
            "GitHub repository and path are required".to_owned(),
            "Agent failed to read GitHub file.".to_owned(),
        );
    };
    let request_payload = json!({
        "repository": request.repository,
        "path": request.path,
        "ref": request.reference,
    });
    let Some(auth) = github_connector_auth(connector_credential) else {
        return AgentToolExecution::succeeded(
            json!({
                "dryRun": true,
                "toolCode": GITHUB_REPO_READ_TOOL_CODE,
                "status": "succeeded",
                "provider": "github",
                "requestPayload": request_payload,
                "message": "GitHub connector credential not configured; dry-run only"
            }),
            true,
            "Agent dry-run prepared GitHub file read.".to_owned(),
        );
    };

    let client = match github_http_client() {
        Ok(client) => client,
        Err(execution) => return execution,
    };
    let response = match client
        .get(github_api_url(&request.rest_path()))
        .query(&request.query_pairs())
        .bearer_auth(&auth.token)
        .header("Accept", "application/vnd.github.raw+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            let error = format!("GitHub file read failed: {err}");
            return AgentToolExecution::failed(
                json!({
                    "dryRun": false,
                    "toolCode": GITHUB_REPO_READ_TOOL_CODE,
                    "status": "failed",
                    "provider": "github",
                    "requestPayload": request_payload,
                    "error": error,
                }),
                error,
                "Agent failed to read GitHub file.".to_owned(),
            );
        }
    };

    let status = response.status();
    let content = response.text().await.unwrap_or_default();
    if !status.is_success() {
        let error = format!("GitHub file read failed: HTTP {}", status.as_u16());
        return AgentToolExecution::failed(
            json!({
                "dryRun": false,
                "toolCode": GITHUB_REPO_READ_TOOL_CODE,
                "status": "failed",
                "provider": "github",
                "requestPayload": request_payload,
                "responsePreview": content.chars().take(1000).collect::<String>(),
                "error": error,
            }),
            error,
            "Agent failed to read GitHub file.".to_owned(),
        );
    }

    AgentToolExecution::succeeded(
        json!({
            "dryRun": false,
            "toolCode": GITHUB_REPO_READ_TOOL_CODE,
            "status": "succeeded",
            "provider": "github",
            "credentialSource": auth.source.code(),
            "credentialSecretRef": auth.secret_ref,
            "requestPayload": request_payload,
            "content": content,
        }),
        false,
        "Agent read GitHub file.".to_owned(),
    )
}

async fn execute_media_image_tool(
    input: &Value,
    model_runtime: Option<&ModelRuntimeService>,
) -> AgentToolExecution {
    let request = media_image_request_from_tool_input(input);
    let request_payload = request.to_provider_payload();
    let route = match model_runtime {
        Some(model_runtime) => match model_runtime
            .resolve_route_for_purpose(ModelRoutePurpose::MediaGeneration)
            .await
        {
            Ok(Some(route)) => route,
            Ok(None) => {
                return AgentToolExecution::succeeded(
                    json!({
                        "dryRun": true,
                        "toolCode": MEDIA_IMAGE_TOOL_CODE,
                        "status": "succeeded",
                        "provider": "right-code-draw",
                        "requestPayload": request_payload,
                        "message": "Draw model route not configured; dry-run only"
                    }),
                    true,
                    "Agent dry-run prepared image generation request.".to_owned(),
                );
            }
            Err(err) => {
                let error = format!("图片生成模型路由解析失败: {err}");
                return AgentToolExecution::failed(
                    json!({
                        "dryRun": false,
                        "toolCode": MEDIA_IMAGE_TOOL_CODE,
                        "status": "failed",
                        "provider": "right-code-draw",
                        "requestPayload": request_payload,
                        "error": error,
                    }),
                    error,
                    "Agent failed to generate image.".to_owned(),
                );
            }
        },
        None => {
            return AgentToolExecution::succeeded(
                json!({
                    "dryRun": true,
                    "toolCode": MEDIA_IMAGE_TOOL_CODE,
                    "status": "succeeded",
                    "provider": "right-code-draw",
                    "requestPayload": request_payload,
                    "message": "Draw model route not configured; dry-run only"
                }),
                true,
                "Agent dry-run prepared image generation request.".to_owned(),
            );
        }
    };
    let route_id = route.route_id().to_owned();
    let provider = route.provider().as_str().to_owned();
    let model = route.model().map(ToOwned::to_owned);
    let endpoint = route.endpoint().to_owned();

    if endpoint.trim().is_empty() {
        return AgentToolExecution::succeeded(
            json!({
                "dryRun": true,
                "toolCode": MEDIA_IMAGE_TOOL_CODE,
                "status": "succeeded",
                "provider": provider,
                "modelRoute": route_id,
                "requestPayload": request_payload,
                "message": "Draw model route not configured; dry-run only"
            }),
            true,
            "Agent dry-run prepared image generation request.".to_owned(),
        );
    }

    let response = match model_runtime
        .expect("model runtime is required after media route resolution")
        .generate_media_image_for_source(&route, &request, "ai.agent.media.image")
        .await
    {
        Ok(response) => response,
        Err(err) => {
            let error = err.to_string();
            return AgentToolExecution::failed(
                json!({
                    "dryRun": false,
                    "toolCode": MEDIA_IMAGE_TOOL_CODE,
                    "status": "failed",
                    "provider": provider,
                    "modelRoute": route_id,
                    "model": model,
                    "requestPayload": request_payload,
                    "error": error,
                }),
                error,
                "Agent failed to generate image.".to_owned(),
            );
        }
    };
    let provider_payload = response.provider_payload;

    AgentToolExecution::succeeded(
        json!({
            "dryRun": false,
            "toolCode": MEDIA_IMAGE_TOOL_CODE,
            "status": "succeeded",
            "provider": provider,
            "modelRoute": route_id,
            "model": model,
            "assetUrl": response.asset_url,
            "providerAssetId": response.provider_asset_id,
            "requestPayload": request_payload,
            "response": provider_payload,
            "message": "Image generated"
        }),
        false,
        "Agent generated image asset.".to_owned(),
    )
}

async fn execute_feishu_message_tool(input: &Value) -> AgentToolExecution {
    let text = feishu_message_text_from_tool_input(input);
    let message = FeishuTextMessage::new(text);
    let payload = message.to_webhook_payload();
    let Some(config) = FeishuWebhookConfig::from_env() else {
        return AgentToolExecution::succeeded(
            json!({
                "dryRun": true,
                "toolCode": FEISHU_TOOL_CODE,
                "status": "succeeded",
                "provider": "feishu",
                "requestPayload": payload,
                "message": "Feishu webhook not configured; dry-run only"
            }),
            true,
            "Agent dry-run prepared Feishu message.".to_owned(),
        );
    };

    let client = match reqwest::Client::builder()
        .timeout(FEISHU_WEBHOOK_TIMEOUT)
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            let error = format!("Feishu 客户端初始化失败: {err}");
            return AgentToolExecution::failed(
                json!({
                    "dryRun": false,
                    "toolCode": FEISHU_TOOL_CODE,
                    "status": "failed",
                    "provider": "feishu",
                    "requestPayload": payload,
                    "error": error,
                }),
                error,
                "Agent failed to send Feishu message.".to_owned(),
            );
        }
    };

    let response = match client.post(&config.webhook_url).json(&payload).send().await {
        Ok(response) => response,
        Err(err) => {
            let error = format!("Feishu 消息发送失败: {err}");
            return AgentToolExecution::failed(
                json!({
                    "dryRun": false,
                    "toolCode": FEISHU_TOOL_CODE,
                    "status": "failed",
                    "provider": "feishu",
                    "requestPayload": payload,
                    "error": error,
                }),
                error,
                "Agent failed to send Feishu message.".to_owned(),
            );
        }
    };

    let status = response.status();
    let response_payload = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !status.is_success() || feishu_response_code(&response_payload).is_some_and(|code| code != 0)
    {
        let error = format!(
            "Feishu 消息发送失败: HTTP {status}, code {:?}",
            feishu_response_code(&response_payload)
        );
        return AgentToolExecution::failed(
            json!({
                "dryRun": false,
                "toolCode": FEISHU_TOOL_CODE,
                "status": "failed",
                "provider": "feishu",
                "requestPayload": payload,
                "response": response_payload,
                "error": error,
            }),
            error,
            "Agent failed to send Feishu message.".to_owned(),
        );
    }

    AgentToolExecution::succeeded(
        json!({
            "dryRun": false,
            "toolCode": FEISHU_TOOL_CODE,
            "status": "succeeded",
            "provider": "feishu",
            "requestPayload": payload,
            "response": response_payload,
            "message": "Feishu message sent"
        }),
        false,
        "Agent sent Feishu message.".to_owned(),
    )
}

fn github_connector_auth(
    credential: Option<&ConnectorCredentialLookupRecord>,
) -> Option<GitHubConnectorAuth> {
    github_connector_auth_from_sources(credential, |key| env::var(key).ok())
}

pub(super) fn github_connector_auth_from_sources<F>(
    credential: Option<&ConnectorCredentialLookupRecord>,
    env_get: F,
) -> Option<GitHubConnectorAuth>
where
    F: FnMut(&str) -> Option<String>,
{
    let binding = credential.and_then(connector_credential_binding);
    select_connector_credential(
        binding.as_ref(),
        &["GITHUB_CONNECTOR_TOKEN", "NOVEX_GITHUB_CONNECTOR_TOKEN"],
        env_get,
    )
}

fn connector_credential_binding(
    credential: &ConnectorCredentialLookupRecord,
) -> Option<ConnectorCredentialBinding> {
    Some(ConnectorCredentialBinding {
        connector_code: credential.connector_code.clone(),
        scope: parse_credential_scope(&credential.scope_type)?,
        scope_id: credential.scope_id.clone(),
        auth_type: credential.auth_type.clone(),
        secret_ref: credential.secret_ref.clone(),
        scopes: connector_scopes_from_value(&credential.scopes),
    })
}

fn connector_scopes_from_value(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn github_api_base_url() -> String {
    env::var("GITHUB_API_BASE_URL")
        .or_else(|_| env::var("NOVEX_GITHUB_API_BASE_URL"))
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "https://api.github.com".to_owned())
}

fn github_api_url(path: &str) -> String {
    format!("{}{}", github_api_base_url(), path)
}

fn github_http_client() -> Result<reqwest::Client, AgentToolExecution> {
    reqwest::Client::builder()
        .timeout(GITHUB_CONNECTOR_TIMEOUT)
        .user_agent("novex-github-connector-poc")
        .build()
        .map_err(|err| {
            let error = format!("GitHub connector client init failed: {err}");
            AgentToolExecution::failed(
                json!({
                    "dryRun": false,
                    "status": "failed",
                    "provider": "github",
                    "error": error,
                }),
                error,
                "Agent failed to initialize GitHub connector.".to_owned(),
            )
        })
}

fn feishu_response_code(value: &Value) -> Option<i64> {
    value
        .get("code")
        .or_else(|| value.get("StatusCode"))
        .and_then(Value::as_i64)
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

    #[tokio::test]
    async fn mcp_tool_execution_uses_mock_response_without_exposing_secret() {
        let tool = McpToolExecutionRecord {
            id: 11,
            server_id: 42,
            server_code: "docs".to_owned(),
            server_name: "Docs".to_owned(),
            endpoint_url: Some("https://mcp.example.com/mcp".to_owned()),
            transport_kind: "streamable_http".to_owned(),
            auth_type: "bearer_env".to_owned(),
            secret_ref: Some("env:DOCS_MCP_TOKEN".to_owned()),
            tool_name: "search".to_owned(),
            tool_code: "mcp.docs.search".to_owned(),
            description: "Search docs".to_owned(),
            input_schema: json!({"type":"object"}),
            output_schema: json!({"type":"object"}),
            risk_level: 1,
            permission_code: Some("ai:mcp:docs:search".to_owned()),
            metadata: json!({
                "mockResponse": {
                    "hits": [
                        {
                            "title": "Codex migration",
                            "score": 0.98
                        }
                    ]
                }
            }),
        };

        let execution =
            execute_mcp_tool("mcp.docs.search", &json!({"query": "codex"}), Some(&tool)).await;

        assert!(execution.succeeded_status());
        assert!(!execution.dry_run);
        assert_eq!(execution.response_payload["provider"], "mcp");
        assert_eq!(
            execution.response_payload["response"]["hits"][0]["title"],
            "Codex migration"
        );
        assert_eq!(
            execution.response_payload["auth"]["secretRef"],
            "env:DOCS_MCP_TOKEN"
        );
        assert!(execution
            .response_payload
            .to_string()
            .contains("DOCS_MCP_TOKEN"));
        let auth = mcp_auth_payload_from_sources(Some("env:DOCS_MCP_TOKEN"), "bearer_env", |key| {
            (key == "DOCS_MCP_TOKEN").then(|| "test-token".to_owned())
        });
        assert_eq!(auth["resolved"], true);
        assert!(!auth.to_string().contains("test-token"));
    }

    #[tokio::test]
    async fn mcp_tool_execution_dry_run_includes_sanitized_live_request_plan() {
        let tool = McpToolExecutionRecord {
            id: 12,
            server_id: 43,
            server_code: "docs".to_owned(),
            server_name: "Docs".to_owned(),
            endpoint_url: Some("https://mcp.example.com/mcp".to_owned()),
            transport_kind: "streamable_http".to_owned(),
            auth_type: "bearer_env".to_owned(),
            secret_ref: Some("env:DOCS_MCP_TOKEN".to_owned()),
            tool_name: "search".to_owned(),
            tool_code: "mcp.docs.search".to_owned(),
            description: "Search docs".to_owned(),
            input_schema: json!({"type":"object"}),
            output_schema: json!({"type":"object"}),
            risk_level: 1,
            permission_code: Some("ai:mcp:docs:search".to_owned()),
            metadata: json!({}),
        };

        let execution =
            execute_mcp_tool("mcp.docs.search", &json!({"query": "codex"}), Some(&tool)).await;

        assert!(execution.succeeded_status());
        assert!(execution.dry_run);
        assert_eq!(
            execution.response_payload["liveRequest"]["body"]["method"],
            "tools/call"
        );
        assert_eq!(
            execution.response_payload["liveRequest"]["headers"]["Accept"],
            "application/json, text/event-stream"
        );
        assert_eq!(
            execution.response_payload["liveRequest"]["body"]["params"]["arguments"]["query"],
            "codex"
        );
        assert_eq!(
            execution.response_payload["liveRequest"]["secretRef"],
            "env:DOCS_MCP_TOKEN"
        );
        assert!(!execution
            .response_payload
            .to_string()
            .contains("test-token"));
    }

    #[test]
    fn feishu_webhook_config_reads_env_map_without_leaking_url_to_payload() {
        let config = FeishuWebhookConfig::from_env_map(|key| match key {
            "FEISHU_WEBHOOK_URL" => {
                Some(" https://open.feishu.cn/open-apis/bot/v2/hook/abc/ ".to_owned())
            }
            _ => None,
        })
        .expect("feishu webhook config should be present");

        assert_eq!(
            config.webhook_url,
            "https://open.feishu.cn/open-apis/bot/v2/hook/abc"
        );
    }

    #[test]
    fn github_connector_auth_prefers_db_credential_secret_ref_over_env_default() {
        let credential = ConnectorCredentialLookupRecord {
            id: 9001,
            connector_id: 3220001,
            connector_code: "github.default".to_owned(),
            scope_type: "tenant".to_owned(),
            scope_id: "1".to_owned(),
            auth_type: "oauth_app".to_owned(),
            secret_ref: "env:DB_GITHUB_TOKEN".to_owned(),
            scopes: serde_json::json!(["repo"]),
            metadata: serde_json::json!({}),
        };

        let auth = github_connector_auth_from_sources(Some(&credential), |key| match key {
            "DB_GITHUB_TOKEN" => Some(" db-token ".to_owned()),
            "GITHUB_CONNECTOR_TOKEN" => Some("env-token".to_owned()),
            _ => None,
        })
        .expect("db credential should resolve");

        assert_eq!(auth.token, "db-token");
        assert_eq!(auth.source.code(), "connector_credential");
        assert_eq!(auth.secret_ref.as_deref(), Some("env:DB_GITHUB_TOKEN"));
    }

    #[test]
    fn github_connector_auth_falls_back_to_env_when_credential_is_missing() {
        let auth = github_connector_auth_from_sources(None, |key| match key {
            "GITHUB_CONNECTOR_TOKEN" => Some(" env-token ".to_owned()),
            _ => None,
        })
        .expect("env token should resolve");

        assert_eq!(auth.token, "env-token");
        assert_eq!(auth.source.code(), "env");
        assert_eq!(auth.secret_ref, None);
    }

    #[test]
    fn agent_concrete_tool_executors_live_in_executor_module() {
        let source = include_str!("agent_tool_executor.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "pub(super) async fn execute_agent_tool(",
            "async fn execute_mcp_tool(",
            "async fn execute_github_repo_search_tool(",
            "async fn execute_github_repo_read_tool(",
            "async fn execute_media_image_tool(",
            "async fn execute_feishu_message_tool(",
        ] {
            assert!(source.contains(needle), "{needle} missing");
        }
    }
}
