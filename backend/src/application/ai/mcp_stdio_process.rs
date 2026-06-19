use std::{process::Stdio, time::Duration};

use novex_connectors::resolve_env_secret_ref;
use novex_mcp::{
    parse_mcp_tool_call_response, McpStdioEnvValue, McpStdioLifecyclePhase, McpStdioToolCallPlan,
    McpStreamableHttpResponse, McpToolInvocationResult,
};
use serde_json::Value;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    process::{Child, Command},
    time::timeout,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct McpStdioProcessError {
    pub(crate) phase: String,
    pub(crate) message: String,
    pub(crate) evidence: Value,
}

impl McpStdioProcessError {
    fn new(phase: McpStdioLifecyclePhase, message: impl Into<String>, evidence: &Value) -> Self {
        Self {
            phase: stdio_phase_name(phase).to_owned(),
            message: message.into(),
            evidence: evidence.clone(),
        }
    }
}

pub(crate) async fn execute_mcp_stdio_tool_with_env<EnvGet>(
    plan: McpStdioToolCallPlan,
    tool_code: &str,
    mut env_get: EnvGet,
) -> Result<McpToolInvocationResult, McpStdioProcessError>
where
    EnvGet: FnMut(&str) -> Option<String>,
{
    let evidence = plan.sanitized_evidence();
    let mut command = Command::new(&plan.launch.command);
    command
        .args(&plan.launch.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(working_dir) = &plan.launch.working_dir {
        command.current_dir(working_dir);
    }
    for (name, value) in &plan.launch.env {
        match value {
            McpStdioEnvValue::Literal(value) => {
                command.env(name, value);
            }
            McpStdioEnvValue::SecretRef(secret_ref) => {
                let Some(value) = resolve_env_secret_ref(secret_ref, &mut env_get) else {
                    return Err(McpStdioProcessError::new(
                        McpStdioLifecyclePhase::Spawn,
                        format!("MCP stdio env secret `{secret_ref}` is not resolved"),
                        &evidence,
                    ));
                };
                command.env(name, value);
            }
        }
    }

    let mut child = command.spawn().map_err(|err| {
        McpStdioProcessError::new(
            McpStdioLifecyclePhase::Spawn,
            format!("MCP stdio process spawn failed: {err}"),
            &evidence,
        )
    })?;
    let mut stdin = child.stdin.take().ok_or_else(|| {
        McpStdioProcessError::new(
            McpStdioLifecyclePhase::Spawn,
            "MCP stdio process stdin is unavailable",
            &evidence,
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        McpStdioProcessError::new(
            McpStdioLifecyclePhase::Spawn,
            "MCP stdio process stdout is unavailable",
            &evidence,
        )
    })?;
    let mut stdout = BufReader::new(stdout);

    let result =
        execute_mcp_stdio_json_rpc(&plan, tool_code, &evidence, &mut stdin, &mut stdout).await;
    drop(stdin);
    shutdown_stdio_child(&mut child, plan.launch.lifecycle_policy.shutdown_timeout_ms).await;
    result
}

async fn execute_mcp_stdio_json_rpc<W, R>(
    plan: &McpStdioToolCallPlan,
    tool_code: &str,
    evidence: &Value,
    stdin: &mut W,
    stdout: &mut R,
) -> Result<McpToolInvocationResult, McpStdioProcessError>
where
    W: AsyncWrite + Unpin,
    R: AsyncBufRead + Unpin,
{
    let startup_timeout_ms = plan.launch.lifecycle_policy.startup_timeout_ms;
    write_stdio_json_line(
        stdin,
        &plan.initialize,
        McpStdioLifecyclePhase::Initialize,
        evidence,
    )
    .await?;
    let initialize_response = read_stdio_json_line(
        stdout,
        startup_timeout_ms,
        McpStdioLifecyclePhase::Initialize,
        evidence,
    )
    .await?;
    ensure_stdio_json_rpc_result(
        &initialize_response,
        McpStdioLifecyclePhase::Initialize,
        evidence,
    )?;

    write_stdio_json_line(
        stdin,
        &plan.initialized,
        McpStdioLifecyclePhase::Initialize,
        evidence,
    )
    .await?;
    write_stdio_json_line(
        stdin,
        &plan.tools_call,
        McpStdioLifecyclePhase::CallTools,
        evidence,
    )
    .await?;
    let call_response = read_stdio_json_line(
        stdout,
        startup_timeout_ms,
        McpStdioLifecyclePhase::CallTools,
        evidence,
    )
    .await?;
    let response = McpStreamableHttpResponse::new(200, "application/json", call_response);
    parse_mcp_tool_call_response(tool_code, &response).map_err(|err| {
        McpStdioProcessError::new(
            McpStdioLifecyclePhase::CallTools,
            format!("MCP stdio response parse failed: {}", err.message),
            evidence,
        )
    })
}

async fn write_stdio_json_line<W>(
    stdin: &mut W,
    message: &Value,
    phase: McpStdioLifecyclePhase,
    evidence: &Value,
) -> Result<(), McpStdioProcessError>
where
    W: AsyncWrite + Unpin,
{
    let mut payload = serde_json::to_vec(message).map_err(|err| {
        McpStdioProcessError::new(
            phase,
            format!("MCP stdio JSON-RPC message serialization failed: {err}"),
            evidence,
        )
    })?;
    payload.push(b'\n');
    stdin.write_all(&payload).await.map_err(|err| {
        McpStdioProcessError::new(phase, format!("MCP stdio write failed: {err}"), evidence)
    })?;
    stdin.flush().await.map_err(|err| {
        McpStdioProcessError::new(phase, format!("MCP stdio flush failed: {err}"), evidence)
    })
}

async fn read_stdio_json_line<R>(
    stdout: &mut R,
    timeout_ms: u64,
    phase: McpStdioLifecyclePhase,
    evidence: &Value,
) -> Result<String, McpStdioProcessError>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = String::new();
    let read = timeout(
        Duration::from_millis(timeout_ms),
        stdout.read_line(&mut line),
    )
    .await
    .map_err(|_| {
        McpStdioProcessError::new(
            phase,
            format!("MCP stdio {} timed out", stdio_phase_name(phase)),
            evidence,
        )
    })?
    .map_err(|err| {
        McpStdioProcessError::new(phase, format!("MCP stdio read failed: {err}"), evidence)
    })?;
    if read == 0 {
        return Err(McpStdioProcessError::new(
            phase,
            "MCP stdio process closed stdout",
            evidence,
        ));
    }

    Ok(line.trim_end_matches(['\r', '\n']).to_owned())
}

fn ensure_stdio_json_rpc_result(
    line: &str,
    phase: McpStdioLifecyclePhase,
    evidence: &Value,
) -> Result<(), McpStdioProcessError> {
    let payload = serde_json::from_str::<Value>(line).map_err(|err| {
        McpStdioProcessError::new(
            phase,
            format!("MCP stdio JSON-RPC response is invalid: {err}"),
            evidence,
        )
    })?;
    if let Some(error) = payload
        .get("error")
        .and_then(|value| value.get("message"))
        .and_then(Value::as_str)
    {
        return Err(McpStdioProcessError::new(
            phase,
            format!("MCP stdio JSON-RPC error: {error}"),
            evidence,
        ));
    }
    if payload.get("result").is_none() {
        return Err(McpStdioProcessError::new(
            phase,
            "MCP stdio JSON-RPC response missing result",
            evidence,
        ));
    }
    Ok(())
}

async fn shutdown_stdio_child(child: &mut Child, timeout_ms: u64) {
    if matches!(child.try_wait(), Ok(Some(_))) {
        return;
    }
    if timeout(Duration::from_millis(timeout_ms), child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

fn stdio_phase_name(phase: McpStdioLifecyclePhase) -> &'static str {
    match phase {
        McpStdioLifecyclePhase::Spawn => "spawn",
        McpStdioLifecyclePhase::Initialize => "initialize",
        McpStdioLifecyclePhase::ListTools => "list_tools",
        McpStdioLifecyclePhase::CallTools => "call_tools",
        McpStdioLifecyclePhase::Shutdown => "shutdown",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use novex_mcp::{
        McpStdioEnvValue, McpStdioLaunchConfig, McpStdioLaunchPlan, McpStdioLifecyclePolicy,
        McpStdioToolCallPlan, McpToolInvocationRequest,
    };
    use serde_json::json;

    fn local_stdio_fixture_plan(
        env: BTreeMap<String, McpStdioEnvValue>,
        startup_timeout_ms: u64,
        shutdown_timeout_ms: u64,
    ) -> McpStdioToolCallPlan {
        let script = concat!(
            "read init\n",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":\"tool-call-1-initialize\",\"result\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"serverInfo\":{\"name\":\"fixture\",\"version\":\"1\"}}}'\n",
            "read initialized\n",
            "read call\n",
            "if [ \"$MCP_TOKEN\" = \"super-secret-token\" ]; then hits=1; else hits=0; fi\n",
            "printf '%s\\n' \"{\\\"jsonrpc\\\":\\\"2.0\\\",\\\"id\\\":\\\"tool-call-1\\\",\\\"result\\\":{\\\"content\\\":[{\\\"type\\\":\\\"text\\\",\\\"text\\\":\\\"stdio fixture\\\"}],\\\"structuredContent\\\":{\\\"hits\\\":$hits},\\\"isError\\\":false}}\"\n",
        );
        let launch = McpStdioLaunchPlan::new(McpStdioLaunchConfig {
            command: "/bin/sh".to_owned(),
            args: vec!["-c".to_owned(), script.to_owned()],
            env,
            working_dir: None,
            lifecycle_policy: McpStdioLifecyclePolicy::new(startup_timeout_ms, shutdown_timeout_ms)
                .expect("timeouts should be valid"),
        })
        .expect("fixture launch plan should be valid");
        let request = McpToolInvocationRequest {
            server_code: "docs".to_owned(),
            tool_name: "search".to_owned(),
            arguments: json!({"query": "codex"}),
        };

        McpStdioToolCallPlan::new(launch, "tool-call-1", &request)
    }

    fn hanging_stdio_fixture_plan(
        startup_timeout_ms: u64,
        shutdown_timeout_ms: u64,
    ) -> McpStdioToolCallPlan {
        let launch = McpStdioLaunchPlan::new(McpStdioLaunchConfig {
            command: "/bin/sh".to_owned(),
            args: vec!["-c".to_owned(), "sleep 2".to_owned()],
            env: BTreeMap::new(),
            working_dir: None,
            lifecycle_policy: McpStdioLifecyclePolicy::new(startup_timeout_ms, shutdown_timeout_ms)
                .expect("timeouts should be valid"),
        })
        .expect("fixture launch plan should be valid");
        let request = McpToolInvocationRequest {
            server_code: "docs".to_owned(),
            tool_name: "search".to_owned(),
            arguments: json!({"query": "codex"}),
        };

        McpStdioToolCallPlan::new(launch, "tool-call-1", &request)
    }

    #[tokio::test]
    async fn mcp_stdio_process_executes_local_json_rpc_server_without_leaking_env_secret() {
        let mut env = BTreeMap::new();
        env.insert(
            "MCP_TOKEN".to_owned(),
            McpStdioEnvValue::SecretRef("env:DOCS_MCP_TOKEN".to_owned()),
        );
        let plan = local_stdio_fixture_plan(env, 2_000, 1_000);

        let result = super::execute_mcp_stdio_tool_with_env(plan, "mcp.docs.search", |key| {
            (key == "DOCS_MCP_TOKEN").then(|| "super-secret-token".to_owned())
        })
        .await
        .expect("stdio MCP call should succeed");

        assert_eq!(result.status, "succeeded");
        assert_eq!(result.output["structuredContent"]["hits"], 1);
        assert!(!serde_json::to_string(&result)
            .expect("result should serialize")
            .contains("super-secret-token"));
    }

    #[tokio::test]
    async fn mcp_stdio_process_timeout_returns_safe_error_evidence() {
        let plan = hanging_stdio_fixture_plan(200, 200);

        let err = super::execute_mcp_stdio_tool_with_env(plan, "mcp.docs.search", |_| None)
            .await
            .expect_err("stdio MCP call should timeout");

        assert_eq!(err.phase, "initialize");
        assert!(err.message.contains("timed out"));
        assert_eq!(err.evidence["transportKind"], "stdio");
        assert!(!err.evidence.to_string().contains("super-secret-token"));
    }
}
