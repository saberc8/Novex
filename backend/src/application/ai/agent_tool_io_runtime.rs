use std::{future::Future, time::Instant};

use chrono::Utc;
use novex_ai_core::RunStatus;
use novex_tools::{AgentToolExecution, ToolBatchExecutionMode};
use serde_json::{json, Value};
use tokio::task::JoinHandle;

use super::agent_service::{
    AgentRunCancellationToken, ExecutedAgentToolCall, PreparedAgentToolCall,
};
use crate::shared::error::AppError;

const AGENT_TOOL_IO_TASK_SUPERVISOR: &str = "agent_tool_io_task_supervisor";
const AGENT_TOOL_IO_TASK_RUNTIME_TOKIO: &str = "tokio_task";
const AGENT_TOOL_IO_TASK_RUNTIME_INLINE: &str = "inline";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AgentToolIoMetrics {
    pub(super) execution_mode: ToolBatchExecutionMode,
    pub(super) task_runtime: String,
    pub(super) supervisor: String,
    pub(super) batch_index: usize,
    pub(super) tool_code: String,
    pub(super) call_id: String,
    pub(super) started_at_ms: i64,
    pub(super) finished_at_ms: i64,
    pub(super) duration_ms: u64,
    pub(super) terminal_status: RunStatus,
    pub(super) cancel_reason: Option<String>,
}

impl AgentToolIoMetrics {
    pub(super) fn payload(&self) -> Value {
        let mut payload = json!({
            "executionMode": self.execution_mode,
            "taskRuntime": self.task_runtime,
            "supervisor": self.supervisor,
            "batchIndex": self.batch_index,
            "toolCode": self.tool_code,
            "callId": self.call_id,
            "startedAtMs": self.started_at_ms,
            "finishedAtMs": self.finished_at_ms,
            "durationMs": self.duration_ms,
            "terminalStatus": self.terminal_status,
        });
        if let (Some(object), Some(cancel_reason)) =
            (payload.as_object_mut(), self.cancel_reason.as_deref())
        {
            object.insert("cancelReason".to_owned(), json!(cancel_reason));
        }
        payload
    }
}

struct AgentToolIoMetricsTimer {
    execution_mode: ToolBatchExecutionMode,
    task_runtime: &'static str,
    supervisor: &'static str,
    batch_index: usize,
    tool_code: String,
    call_id: String,
    started_at_ms: i64,
    started_at: Instant,
}

impl AgentToolIoMetricsTimer {
    fn start(
        execution_mode: ToolBatchExecutionMode,
        task_runtime: &'static str,
        prepared: &PreparedAgentToolCall,
    ) -> Self {
        Self {
            execution_mode,
            task_runtime,
            supervisor: AGENT_TOOL_IO_TASK_SUPERVISOR,
            batch_index: prepared.batch_index,
            tool_code: prepared.tool.code.clone(),
            call_id: prepared.call_id.clone(),
            started_at_ms: Utc::now().timestamp_millis(),
            started_at: Instant::now(),
        }
    }

    fn finish(self, executed: &ExecutedAgentToolCall) -> AgentToolIoMetrics {
        let finished_at_ms = Utc::now().timestamp_millis().max(self.started_at_ms);
        let duration_ms = self.started_at.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let cancel_reason = executed
            .execution
            .response_payload
            .get("cancelReason")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

        AgentToolIoMetrics {
            execution_mode: self.execution_mode,
            task_runtime: self.task_runtime.to_owned(),
            supervisor: self.supervisor.to_owned(),
            batch_index: self.batch_index,
            tool_code: self.tool_code,
            call_id: self.call_id,
            started_at_ms: self.started_at_ms,
            finished_at_ms,
            duration_ms,
            terminal_status: executed.terminal_status,
            cancel_reason,
        }
    }
}

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
                            ToolBatchExecutionMode::Parallel,
                            AGENT_TOOL_IO_TASK_RUNTIME_TOKIO,
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
                        ToolBatchExecutionMode::Serial,
                        AGENT_TOOL_IO_TASK_RUNTIME_INLINE,
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
    execution_mode: ToolBatchExecutionMode,
    task_runtime: &'static str,
    execute: F,
) -> Result<ExecutedAgentToolCall, AppError>
where
    F: Fn(PreparedAgentToolCall) -> Fut,
    Fut: Future<Output = Result<ExecutedAgentToolCall, AppError>>,
{
    let timeout = prepared.timeout;
    let metrics_timer = AgentToolIoMetricsTimer::start(execution_mode, task_runtime, &prepared);
    let outcome = tokio::select! {
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
            tool_io_metrics: None,
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
                tool_io_metrics: None,
            }),
        },
    };

    outcome.map(|mut executed| {
        executed.tool_io_metrics = Some(metrics_timer.finish(&executed));
        executed
    })
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

    #[test]
    fn agent_tool_io_runtime_emits_task_metrics_contract() {
        let source = include_str!("agent_tool_io_runtime.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "pub(super) struct AgentToolIoMetrics",
            "tool_io_metrics",
            "AgentToolIoMetricsTimer",
            "executionMode",
            "taskRuntime",
            "supervisor",
            "batchIndex",
            "startedAtMs",
            "finishedAtMs",
            "durationMs",
            "terminalStatus",
            "cancelReason",
        ] {
            assert!(source.contains(needle), "{needle} missing");
        }
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

    fn runtime_test_prepared_tool_call(
        batch_index: usize,
        call_id: &str,
        tool_code: &str,
    ) -> PreparedAgentToolCall {
        PreparedAgentToolCall {
            batch_index,
            call_id: call_id.to_owned(),
            tool: crate::infrastructure::persistence::ai_capability_repository::ToolLookupRecord {
                id: batch_index as i64 + 1,
                code: tool_code.to_owned(),
                tool_kind: "function".to_owned(),
                executor_kind: "agent".to_owned(),
                risk_level: 1,
                approval_policy: 1,
                permission_code: Some("ai:tool:dryRun".to_owned()),
            },
            arguments: json!({ "batchIndex": batch_index }),
            executor_binding: None,
            concurrency_policy: serde_json::Value::Null,
            timeout: std::time::Duration::from_secs(1),
        }
    }

    fn runtime_test_executed_tool_call(prepared: PreparedAgentToolCall) -> ExecutedAgentToolCall {
        ExecutedAgentToolCall {
            prepared,
            execution: AgentToolExecution::succeeded(
                json!({ "status": "succeeded" }),
                true,
                "ok".to_owned(),
            ),
            terminal_status: RunStatus::Succeeded,
            tool_io_metrics: None,
        }
    }

    #[tokio::test]
    async fn parallel_tool_io_batch_attaches_task_metrics_to_each_execution() {
        let calls = vec![runtime_test_prepared_tool_call(2, "call-3", "rag.search")];
        let (_guard, cancel_token) =
            crate::application::ai::agent_service::AgentRuntimeRegistry::default()
                .register_run(1, 1);

        let result = execute_agent_tool_io_batch(
            ToolBatchExecutionMode::Parallel,
            calls,
            cancel_token,
            |prepared| async move { Ok(runtime_test_executed_tool_call(prepared)) },
        )
        .await
        .unwrap();

        let metrics = result[0].tool_io_metrics.as_ref().unwrap();
        assert_eq!(metrics.execution_mode, ToolBatchExecutionMode::Parallel);
        assert_eq!(metrics.task_runtime, "tokio_task");
        assert_eq!(metrics.supervisor, "agent_tool_io_task_supervisor");
        assert_eq!(metrics.batch_index, 2);
        assert_eq!(metrics.tool_code, "rag.search");
        assert_eq!(metrics.call_id, "call-3");
        assert_eq!(metrics.terminal_status, RunStatus::Succeeded);
        assert!(metrics.finished_at_ms >= metrics.started_at_ms);
    }
}
