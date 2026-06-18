use std::future::Future;

use novex_ai_core::RunStatus;
use novex_tools::{AgentToolExecution, ToolBatchExecutionMode};
use serde_json::json;
use tokio::task::JoinHandle;

use super::agent_service::{
    AgentRunCancellationToken, ExecutedAgentToolCall, PreparedAgentToolCall,
};
use crate::shared::error::AppError;

struct AgentToolIoTask {
    handle: Option<JoinHandle<Result<ExecutedAgentToolCall, AppError>>>,
}

impl AgentToolIoTask {
    fn spawn<Fut>(future: Fut) -> Self
    where
        Fut: Future<Output = Result<ExecutedAgentToolCall, AppError>> + Send + 'static,
    {
        Self {
            handle: Some(tokio::spawn(future)),
        }
    }

    async fn wait(mut self) -> Result<ExecutedAgentToolCall, AppError> {
        let handle = self.handle.take().ok_or_else(|| {
            AppError::Anyhow(anyhow::anyhow!("agent tool I/O task already awaited"))
        })?;
        handle.await.map_err(|error| {
            AppError::Anyhow(anyhow::anyhow!("agent tool I/O task failed: {error}"))
        })?
    }
}

impl Drop for AgentToolIoTask {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

pub(super) async fn execute_agent_tool_io_batch<F, Fut>(
    mode: ToolBatchExecutionMode,
    prepared_calls: Vec<PreparedAgentToolCall>,
    cancel_token: AgentRunCancellationToken,
    execute: F,
) -> Result<Vec<ExecutedAgentToolCall>, AppError>
where
    F: Fn(PreparedAgentToolCall) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Result<ExecutedAgentToolCall, AppError>> + Send + 'static,
{
    match mode {
        ToolBatchExecutionMode::Parallel => {
            let tasks = prepared_calls
                .into_iter()
                .map(|prepared| {
                    let execute = execute.clone();
                    let cancel_token = cancel_token.clone();
                    AgentToolIoTask::spawn(async move {
                        execute_agent_tool_io_with_timeout_and_cancel(
                            prepared,
                            cancel_token,
                            execute,
                        )
                        .await
                    })
                })
                .collect::<Vec<_>>();
            let mut executions = Vec::with_capacity(tasks.len());
            for task in tasks {
                executions.push(task.wait().await?);
            }
            Ok(executions)
        }
        ToolBatchExecutionMode::Serial => {
            let mut executions = Vec::with_capacity(prepared_calls.len());
            for prepared in prepared_calls {
                executions.push(
                    execute_agent_tool_io_with_timeout_and_cancel(
                        prepared,
                        cancel_token.clone(),
                        execute.clone(),
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
    execute: F,
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
    use super::*;

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

    #[test]
    fn agent_tool_io_runtime_uses_abort_on_drop_task_supervisor() {
        let source = include_str!("agent_tool_io_runtime.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "struct AgentToolIoTask",
            "JoinHandle<Result<ExecutedAgentToolCall, AppError>>",
            "impl Drop for AgentToolIoTask",
            "handle.abort()",
            "AgentToolIoTask::spawn",
            "task.wait().await",
        ] {
            assert!(source.contains(needle), "{needle} missing");
        }
        assert!(
            !source.contains("join_all"),
            "parallel tool I/O should use owned tasks instead of join_all"
        );
    }

    #[tokio::test]
    async fn agent_tool_io_task_drop_aborts_pending_tool_future() {
        struct AbortGuard(Option<tokio::sync::oneshot::Sender<()>>);

        impl Drop for AbortGuard {
            fn drop(&mut self) {
                if let Some(sender) = self.0.take() {
                    let _ = sender.send(());
                }
            }
        }

        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (aborted_tx, aborted_rx) = tokio::sync::oneshot::channel();
        let task = AgentToolIoTask::spawn(async move {
            let _guard = AbortGuard(Some(aborted_tx));
            let _ = started_tx.send(());
            std::future::pending::<Result<ExecutedAgentToolCall, AppError>>().await
        });

        tokio::time::timeout(std::time::Duration::from_secs(1), started_rx)
            .await
            .unwrap()
            .unwrap();
        drop(task);

        tokio::time::timeout(std::time::Duration::from_secs(1), aborted_rx)
            .await
            .unwrap()
            .unwrap();
    }
}
