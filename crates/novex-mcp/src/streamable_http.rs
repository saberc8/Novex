use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::client_error::{McpClientError, McpClientErrorKind};
use crate::json_rpc::McpJsonRpcRequest;
use crate::types::{McpToolInvocationRequest, McpToolInvocationResult};
use crate::MCP_PROTOCOL_VERSION;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStreamableHttpRequestPlan {
    pub endpoint_url: String,
    pub http_method: String,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
    pub secret_ref: Option<String>,
}

impl McpStreamableHttpRequestPlan {
    pub fn tools_call(
        endpoint_url: impl Into<String>,
        request_id: impl Into<String>,
        request: &McpToolInvocationRequest,
        secret_ref: Option<&str>,
    ) -> Self {
        let mut headers = BTreeMap::new();
        headers.insert(
            "Accept".to_owned(),
            "application/json, text/event-stream".to_owned(),
        );
        headers.insert("Content-Type".to_owned(), "application/json".to_owned());
        headers.insert(
            "MCP-Protocol-Version".to_owned(),
            MCP_PROTOCOL_VERSION.to_owned(),
        );

        Self {
            endpoint_url: endpoint_url.into(),
            http_method: "POST".to_owned(),
            headers,
            body: McpJsonRpcRequest::tools_call(request_id, request).into_value(),
            secret_ref: secret_ref.map(ToOwned::to_owned),
        }
    }

    pub fn header_value(&self, name: &str) -> Option<String> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.clone())
    }

    pub fn sanitized_evidence(&self) -> Value {
        json!({
            "endpointUrl": self.endpoint_url,
            "httpMethod": self.http_method,
            "headers": self.headers,
            "body": self.body,
            "secretRef": self.secret_ref,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStreamableHttpResponse {
    pub http_status: u16,
    pub content_type: String,
    pub body: String,
}

impl McpStreamableHttpResponse {
    pub fn new(http_status: u16, content_type: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            http_status,
            content_type: content_type.into(),
            body: body.into(),
        }
    }
}

pub fn parse_mcp_tool_call_response(
    tool_code: impl Into<String>,
    response: &McpStreamableHttpResponse,
) -> Result<McpToolInvocationResult, McpClientError> {
    let tool_code = tool_code.into();
    if response.http_status >= 400 {
        return Err(McpClientError::new(
            McpClientErrorKind::HttpStatus,
            format!("MCP server returned HTTP {}", response.http_status),
        )
        .with_http_status(response.http_status));
    }

    let content_type = response
        .content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let payload = match content_type.as_str() {
        "application/json" => parse_mcp_json_payload(&response.body)?,
        "text/event-stream" => parse_mcp_sse_payload(&response.body)?,
        _ => {
            return Err(McpClientError::new(
                McpClientErrorKind::UnsupportedContentType,
                format!("Unsupported MCP content type `{}`", response.content_type),
            ));
        }
    };

    mcp_tool_result_from_json_rpc(tool_code, payload)
}

fn parse_mcp_json_payload(body: &str) -> Result<Value, McpClientError> {
    serde_json::from_str(body).map_err(|error| {
        McpClientError::new(
            McpClientErrorKind::MalformedJson,
            format!("MCP JSON response is invalid: {error}"),
        )
    })
}

fn parse_mcp_sse_payload(body: &str) -> Result<Value, McpClientError> {
    for event in body.split("\n\n") {
        let data = event
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim)
            .filter(|line| !line.is_empty() && *line != "[DONE]")
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() {
            continue;
        }
        return parse_mcp_json_payload(&data);
    }
    Err(McpClientError::new(
        McpClientErrorKind::MissingResult,
        "MCP event stream did not include a JSON-RPC data message",
    ))
}

fn mcp_tool_result_from_json_rpc(
    tool_code: String,
    payload: Value,
) -> Result<McpToolInvocationResult, McpClientError> {
    if let Some(error) = payload.get("error") {
        let code = error
            .get("code")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("MCP JSON-RPC error");
        return Err(
            McpClientError::new(McpClientErrorKind::JsonRpcError, message.to_owned())
                .with_rpc_code(code),
        );
    }

    let result = payload.get("result").cloned().ok_or_else(|| {
        McpClientError::new(
            McpClientErrorKind::MissingResult,
            "MCP JSON-RPC response missing result",
        )
    })?;
    let is_error = result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(McpToolInvocationResult {
        tool_code,
        status: if is_error { "failed" } else { "succeeded" }.to_owned(),
        output: result,
        dry_run: false,
    })
}
