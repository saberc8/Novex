use std::future::Future;

use futures_util::future::join_all;
use novex_ai_core::RunStatus;
use novex_tools::{AgentToolExecution, ToolBatchExecutionMode};
use serde_json::json;

use super::agent_service::{
    AgentRunCancellationToken, ExecutedAgentToolCall, PreparedAgentToolCall,
};
use crate::shared::error::AppError;

pub(super) async fn execute_agent_tool_io_batch<F, Fut>(
    mode: ToolBatchExecutionMode,
    prepared_calls: Vec<PreparedAgentToolCall>,
    cancel_token: AgentRunCancellationToken,
    execute: F,
) -> Result<Vec<ExecutedAgentToolCall>, AppError>
where
    F: Fn(PreparedAgentToolCall) -> Fut,
    Fut: Future<Output = Result<ExecutedAgentToolCall, AppError>>,
{
    match mode {
        ToolBatchExecutionMode::Parallel => {
            let results = join_all(prepared_calls.into_iter().map(|prepared| {
                execute_agent_tool_io_with_timeout_and_cancel(
                    prepared,
                    cancel_token.clone(),
                    &execute,
                )
            }))
            .await;
            results.into_iter().collect()
        }
        ToolBatchExecutionMode::Serial => {
            let mut executions = Vec::with_capacity(prepared_calls.len());
            for prepared in prepared_calls {
                executions.push(
                    execute_agent_tool_io_with_timeout_and_cancel(
                        prepared,
                        cancel_token.clone(),
                        &execute,
                    )
                    .await?,
                );
            }
            Ok(executions)
        }
    }
}

async fn execute_agent_tool_io_with_timeout_and_cancel<F, Fut>(
    prepared: PreparedAgentToolCall,
    cancel_token: AgentRunCancellationToken,
    execute: &F,
) -> Result<ExecutedAgentToolCall, AppError>
where
    F: Fn(PreparedAgentToolCall) -> Fut,
    Fut: Future<Output = Result<ExecutedAgentToolCall, AppError>>,
{
    let timeout = prepared.timeout;
    tokio::select! {
        biased;
        _ = cancel_token.cancelled() => Ok(ExecutedAgentToolCall {
            execution: AgentToolExecution::cancelled(
                json!({
                    "status": "cancelled",
                    "cancelReason": "external_cancel",
                    "cancelStage": "tool_io",
                    "toolCode": prepared.tool.code,
                    "callId": prepared.call_id,
                }),
                format!("Tool `{}` was cancelled by run cancellation.", prepared.tool.code),
            ),
            prepared,
            terminal_status: RunStatus::Cancelled,
        }),
        result = tokio::time::timeout(timeout, execute(prepared.clone())) => match result {
            Ok(result) => result,
            Err(_) => Ok(ExecutedAgentToolCall {
                execution: AgentToolExecution::cancelled(
                    json!({
                        "status": "cancelled",
                        "cancelReason": "tool_io_timeout",
                        "toolCode": prepared.tool.code,
                        "callId": prepared.call_id,
                        "timeoutMs": timeout.as_millis() as u64,
                    }),
                    format!(
                        "Tool `{}` was cancelled after {} ms.",
                        prepared.tool.code,
                        timeout.as_millis()
                    ),
                ),
                prepared,
                terminal_status: RunStatus::Cancelled,
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn agent_tool_io_runtime_owns_batch_task_control() {
        let source = include_str!("agent_tool_io_runtime.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "pub(super) async fn execute_agent_tool_io_batch",
            "async fn execute_agent_tool_io_with_timeout_and_cancel",
            "ToolBatchExecutionMode::Parallel",
            "ToolBatchExecutionMode::Serial",
            "AgentToolExecution::cancelled(",
        ] {
            assert!(source.contains(needle), "{needle} missing");
        }
    }
}
