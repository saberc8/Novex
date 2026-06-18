use std::{env, future::Future, time::Duration};

use novex_connectors::{
    parse_credential_scope, parse_github_code_search_response, resolve_env_secret_ref,
    select_connector_credential, ConnectorCredentialBinding, FeishuTextMessage,
    ResolvedConnectorCredential,
};
use novex_mcp::{
    parse_mcp_tool_call_response, McpStreamableHttpRequestPlan, McpStreamableHttpResponse,
    McpToolInvocationRequest, McpToolInvocationResult,
};
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
const MCP_STREAMABLE_HTTP_TIMEOUT: Duration = Duration::from_secs(20);

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
    execute_mcp_tool_with_http_dispatch(
        tool_code,
        input,
        mcp_tool,
        |key| env::var(key).ok(),
        |request_plan, bearer_token| async move {
            dispatch_mcp_streamable_http_request(request_plan, bearer_token).await
        },
    )
    .await
}

async fn execute_mcp_tool_with_http_dispatch<EnvGet, Dispatch, DispatchFuture>(
    tool_code: &str,
    input: &Value,
    mcp_tool: Option<&McpToolExecutionRecord>,
    mut env_get: EnvGet,
    dispatch: Dispatch,
) -> AgentToolExecution
where
    EnvGet: FnMut(&str) -> Option<String>,
    Dispatch: FnOnce(McpStreamableHttpRequestPlan, Option<String>) -> DispatchFuture,
    DispatchFuture: Future<Output = Result<McpStreamableHttpResponse, String>>,
{
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
    let resolved_secret = tool
        .secret_ref
        .as_deref()
        .and_then(|secret_ref| resolve_env_secret_ref(secret_ref, &mut env_get));
    let auth = mcp_auth_payload_from_resolved(
        tool.secret_ref.as_deref(),
        &tool.auth_type,
        resolved_secret.is_some(),
    );
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

    let live_request_plan = mcp_streamable_http_request_plan(tool, &request);
    if mcp_live_execution_enabled(&tool.metadata) {
        let live_request = live_request_plan
            .as_ref()
            .map(McpStreamableHttpRequestPlan::sanitized_evidence)
            .unwrap_or(Value::Null);
        let Some(live_request_plan) = live_request_plan else {
            let error = "MCP Streamable HTTP endpoint is not configured".to_owned();
            return failed_mcp_live_tool_execution(
                tool,
                &request,
                &live_request,
                &auth,
                error,
                None,
                None,
            );
        };
        if !mcp_live_auth_supported(&tool.auth_type) {
            let error = format!(
                "MCP live dispatch auth type `{}` is not supported",
                tool.auth_type
            );
            return failed_mcp_live_tool_execution(
                tool,
                &request,
                &live_request,
                &auth,
                error,
                None,
                None,
            );
        }
        if mcp_auth_requires_secret(&tool.auth_type) && resolved_secret.is_none() {
            let error = "MCP live dispatch secret is not resolved".to_owned();
            return failed_mcp_live_tool_execution(
                tool,
                &request,
                &live_request,
                &auth,
                error,
                None,
                None,
            );
        }
        let bearer_token = if mcp_auth_uses_bearer_token(&tool.auth_type) {
            resolved_secret
        } else {
            None
        };
        let response = match dispatch(live_request_plan, bearer_token).await {
            Ok(response) => response,
            Err(error) => {
                return failed_mcp_live_tool_execution(
                    tool,
                    &request,
                    &live_request,
                    &auth,
                    error,
                    None,
                    None,
                );
            }
        };
        let response_meta = json!({
            "httpStatus": response.http_status,
            "contentType": response.content_type,
        });
        let result = match parse_mcp_tool_call_response(&tool.tool_code, &response) {
            Ok(result) => result,
            Err(error) => {
                let error_detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                    json!({
                        "message": error.message,
                    })
                });
                let message = format!("MCP live dispatch response parse failed: {}", error.message);
                return failed_mcp_live_tool_execution(
                    tool,
                    &request,
                    &live_request,
                    &auth,
                    message,
                    Some(response_meta),
                    Some(error_detail),
                );
            }
        };
        return AgentToolExecution::succeeded(
            json!({
                "dryRun": result.dry_run,
                "toolCode": result.tool_code,
                "status": result.status,
                "provider": "mcp",
                "live": true,
                "server": mcp_server_payload(tool),
                "request": request,
                "liveRequest": live_request,
                "responseMeta": response_meta,
                "response": result.output,
                "auth": auth,
                "mocked": false,
            }),
            result.dry_run,
            format!("Agent executed MCP tool {} via live HTTP.", tool.tool_code),
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

fn failed_mcp_live_tool_execution(
    tool: &McpToolExecutionRecord,
    request: &McpToolInvocationRequest,
    live_request: &Value,
    auth: &Value,
    error: String,
    response_meta: Option<Value>,
    error_detail: Option<Value>,
) -> AgentToolExecution {
    let mut payload = json!({
        "dryRun": false,
        "toolCode": tool.tool_code,
        "status": "failed",
        "provider": "mcp",
        "live": true,
        "server": mcp_server_payload(tool),
        "request": request,
        "liveRequest": live_request,
        "auth": auth,
        "mocked": false,
        "error": error,
    });
    if let Value::Object(payload) = &mut payload {
        if let Some(response_meta) = response_meta {
            payload.insert("responseMeta".to_owned(), response_meta);
        }
        if let Some(error_detail) = error_detail {
            payload.insert("errorDetail".to_owned(), error_detail);
        }
    }

    AgentToolExecution::failed(
        payload,
        error,
        "Agent failed to execute MCP tool.".to_owned(),
    )
}

async fn dispatch_mcp_streamable_http_request(
    request_plan: McpStreamableHttpRequestPlan,
    bearer_token: Option<String>,
) -> Result<McpStreamableHttpResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(MCP_STREAMABLE_HTTP_TIMEOUT)
        .user_agent("novex-mcp-streamable-http")
        .build()
        .map_err(|err| format!("MCP live dispatch client init failed: {err}"))?;
    let mut request = client.post(&request_plan.endpoint_url);
    for (header, value) in &request_plan.headers {
        request = request.header(header.as_str(), value.as_str());
    }
    if let Some(token) = bearer_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        request = request.bearer_auth(token);
    }

    let response = request
        .json(&request_plan.body)
        .send()
        .await
        .map_err(|err| format!("MCP live dispatch failed: {err}"))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/json")
        .to_owned();
    let body = response
        .text()
        .await
        .map_err(|err| format!("MCP live dispatch response read failed: {err}"))?;

    Ok(McpStreamableHttpResponse::new(status, content_type, body))
}

fn mcp_streamable_http_request_plan(
    tool: &McpToolExecutionRecord,
    request: &McpToolInvocationRequest,
) -> Option<McpStreamableHttpRequestPlan> {
    let endpoint_url = tool.endpoint_url.as_deref()?;
    if !matches!(tool.transport_kind.as_str(), "streamable_http" | "sse") {
        return None;
    }

    Some(McpStreamableHttpRequestPlan::tools_call(
        endpoint_url,
        format!("mcp-tool-{}", tool.id),
        request,
        tool.secret_ref.as_deref(),
    ))
}

fn mcp_streamable_http_request_payload(
    tool: &McpToolExecutionRecord,
    request: &McpToolInvocationRequest,
) -> Value {
    mcp_streamable_http_request_plan(tool, request)
        .map(|request_plan| request_plan.sanitized_evidence())
        .unwrap_or(Value::Null)
}

fn mcp_live_execution_enabled(metadata: &Value) -> bool {
    metadata
        .get("liveExecutionEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn mcp_normalized_auth_type(auth_type: &str) -> String {
    auth_type.trim().to_ascii_lowercase()
}

fn mcp_live_auth_supported(auth_type: &str) -> bool {
    matches!(
        mcp_normalized_auth_type(auth_type).as_str(),
        "" | "none" | "bearer" | "bearer_env"
    )
}

fn mcp_auth_requires_secret(auth_type: &str) -> bool {
    matches!(
        mcp_normalized_auth_type(auth_type).as_str(),
        "bearer" | "bearer_env" | "headers" | "oauth"
    )
}

fn mcp_auth_uses_bearer_token(auth_type: &str) -> bool {
    matches!(
        mcp_normalized_auth_type(auth_type).as_str(),
        "bearer" | "bearer_env"
    )
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

#[allow(dead_code)]
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
    mcp_auth_payload_from_resolved(secret_ref, auth_type, resolved)
}

fn mcp_auth_payload_from_resolved(
    secret_ref: Option<&str>,
    auth_type: &str,
    resolved: bool,
) -> Value {
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
    use axum::{
        extract::State,
        http::{HeaderMap, Method, StatusCode},
        response::IntoResponse,
        routing::post,
        Json, Router,
    };
    use novex_tools::{ToolExecutorBinding, ToolExecutorKind};
    use std::{collections::BTreeMap, sync::Arc};
    use tokio::{
        net::TcpListener,
        sync::{oneshot, Mutex},
    };

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

    fn live_mcp_tool_record(metadata: Value) -> McpToolExecutionRecord {
        McpToolExecutionRecord {
            id: 21,
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
            metadata,
        }
    }

    #[derive(Debug)]
    struct LocalMcpServerCapture {
        method: String,
        headers: BTreeMap<String, String>,
        body: Value,
    }

    #[derive(Clone)]
    struct LocalMcpServerState {
        capture_tx: Arc<Mutex<Option<oneshot::Sender<LocalMcpServerCapture>>>>,
        shutdown_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    }

    async fn run_one_shot_mcp_server() -> (String, oneshot::Receiver<LocalMcpServerCapture>) {
        let (capture_tx, capture_rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let state = LocalMcpServerState {
            capture_tx: Arc::new(Mutex::new(Some(capture_tx))),
            shutdown_tx: Arc::new(Mutex::new(Some(shutdown_tx))),
        };
        let app = Router::new()
            .route("/mcp", post(local_mcp_tools_call_handler))
            .with_state(state);
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("local MCP server should bind");
        let addr = listener
            .local_addr()
            .expect("local MCP server address should be available");
        tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            let _ = server.await;
        });

        (format!("http://{addr}/mcp"), capture_rx)
    }

    async fn local_mcp_tools_call_handler(
        State(state): State<LocalMcpServerState>,
        method: Method,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> impl IntoResponse {
        let captured_headers = headers
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_owned(), value.to_owned()))
            })
            .collect::<BTreeMap<_, _>>();
        let capture = LocalMcpServerCapture {
            method: method.as_str().to_owned(),
            headers: captured_headers,
            body,
        };
        if let Some(sender) = state.capture_tx.lock().await.take() {
            let _ = sender.send(capture);
        }
        if let Some(sender) = state.shutdown_tx.lock().await.take() {
            let _ = sender.send(());
        }

        (
            StatusCode::OK,
            [("content-type", "application/json")],
            Json(json!({
                "jsonrpc": "2.0",
                "id": "mcp-tool-21",
                "result": {
                    "content": [{"type": "text", "text": "Local MCP found Codex docs"}],
                    "structuredContent": {"hits": 1, "source": "local-smoke"},
                    "isError": false
                }
            })),
        )
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

    #[tokio::test]
    async fn mcp_tool_execution_live_http_dispatch_uses_streamable_http_plan() {
        let tool = live_mcp_tool_record(json!({"liveExecutionEnabled": true}));

        let execution = execute_mcp_tool_with_http_dispatch(
            "mcp.docs.search",
            &json!({"query": "codex"}),
            Some(&tool),
            |key| (key == "DOCS_MCP_TOKEN").then(|| "secret-token".to_owned()),
            |plan, bearer_token| async move {
                assert_eq!(bearer_token.as_deref(), Some("secret-token"));
                assert_eq!(plan.endpoint_url, "https://mcp.example.com/mcp");
                assert_eq!(plan.http_method, "POST");
                assert_eq!(
                    plan.header_value("MCP-Protocol-Version").as_deref(),
                    Some(novex_mcp::MCP_PROTOCOL_VERSION)
                );
                assert_eq!(plan.body["method"], "tools/call");
                assert_eq!(plan.body["params"]["name"], "search");
                assert_eq!(plan.body["params"]["arguments"]["query"], "codex");
                Ok(novex_mcp::McpStreamableHttpResponse::new(
                    200,
                    "application/json",
                    json!({
                        "jsonrpc": "2.0",
                        "id": "mcp-tool-21",
                        "result": {
                            "content": [{"type": "text", "text": "Found Codex docs"}],
                            "structuredContent": {"hits": 1},
                            "isError": false
                        }
                    })
                    .to_string(),
                ))
            },
        )
        .await;

        assert!(execution.succeeded_status());
        assert!(!execution.dry_run);
        assert_eq!(execution.response_payload["provider"], "mcp");
        assert_eq!(execution.response_payload["live"], true);
        assert_eq!(execution.response_payload["mocked"], false);
        assert_eq!(
            execution.response_payload["response"]["structuredContent"]["hits"],
            1
        );
        assert_eq!(
            execution.response_payload["liveRequest"]["body"]["method"],
            "tools/call"
        );
        assert_eq!(
            execution.response_payload["auth"]["secretRef"],
            "env:DOCS_MCP_TOKEN"
        );
        assert_eq!(execution.response_payload["auth"]["resolved"], true);
        assert!(!execution
            .response_payload
            .to_string()
            .contains("secret-token"));
    }

    #[tokio::test]
    async fn mcp_tool_execution_live_http_dispatch_failure_returns_safe_payload() {
        let tool = live_mcp_tool_record(json!({"liveExecutionEnabled": true}));

        let execution = execute_mcp_tool_with_http_dispatch(
            "mcp.docs.search",
            &json!({"query": "codex"}),
            Some(&tool),
            |key| (key == "DOCS_MCP_TOKEN").then(|| "secret-token".to_owned()),
            |_plan, _bearer_token| async move { Err("MCP dispatch failed: timeout".to_owned()) },
        )
        .await;

        assert!(!execution.succeeded_status());
        assert!(!execution.dry_run);
        assert_eq!(execution.response_payload["provider"], "mcp");
        assert_eq!(execution.response_payload["live"], true);
        assert_eq!(
            execution.response_payload["liveRequest"]["secretRef"],
            "env:DOCS_MCP_TOKEN"
        );
        assert_eq!(execution.response_payload["auth"]["resolved"], true);
        assert_eq!(
            execution.response_payload["error"],
            "MCP dispatch failed: timeout"
        );
        assert!(!execution
            .response_payload
            .to_string()
            .contains("secret-token"));
    }

    #[tokio::test]
    async fn mcp_tool_execution_live_http_dispatch_reaches_local_streamable_http_server() {
        let (endpoint_url, captured_request) = run_one_shot_mcp_server().await;
        let mut tool = live_mcp_tool_record(json!({"liveExecutionEnabled": true}));
        tool.endpoint_url = Some(endpoint_url);

        let execution = execute_mcp_tool_with_http_dispatch(
            "mcp.docs.search",
            &json!({"query": "codex", "limit": 3}),
            Some(&tool),
            |key| (key == "DOCS_MCP_TOKEN").then(|| "local-secret-token".to_owned()),
            |plan, bearer_token| async move {
                dispatch_mcp_streamable_http_request(plan, bearer_token).await
            },
        )
        .await;
        let captured = captured_request
            .await
            .expect("local MCP server should capture one request");

        assert!(execution.succeeded_status());
        assert!(!execution.dry_run);
        assert_eq!(execution.response_payload["provider"], "mcp");
        assert_eq!(execution.response_payload["live"], true);
        assert_eq!(
            execution.response_payload["response"]["structuredContent"]["hits"],
            1
        );
        assert_eq!(captured.method, "POST");
        assert_eq!(
            captured.headers["authorization"],
            "Bearer local-secret-token"
        );
        assert_eq!(
            captured.headers["mcp-protocol-version"],
            novex_mcp::MCP_PROTOCOL_VERSION
        );
        assert!(captured.headers["accept"].contains("application/json"));
        assert!(captured.headers["accept"].contains("text/event-stream"));
        assert!(captured.headers["content-type"].contains("application/json"));
        assert_eq!(captured.body["method"], "tools/call");
        assert_eq!(captured.body["params"]["name"], "search");
        assert_eq!(captured.body["params"]["arguments"]["query"], "codex");
        assert_eq!(captured.body["params"]["arguments"]["limit"], 3);
        assert!(!execution
            .response_payload
            .to_string()
            .contains("local-secret-token"));
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
