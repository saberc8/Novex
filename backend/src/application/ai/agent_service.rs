use std::{
    collections::{BTreeSet, HashMap},
    future::Future,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use chrono::{NaiveDate, NaiveDateTime, Utc};
use novex_agent::{plan_react_run_with_memory, AgentIntent, AgentLoopKind};
use novex_agent_protocol::{AgentTurnItem, ToolObservationStatus};
use novex_agent_runtime::{
    parse_model_turn_output, AgentRemoteCompactionRequest, AgentRuntimeBudget, AgentRuntimeState,
    ParsedModelTurnOutput, StreamingModelTurnParseStatus, StreamingModelTurnParser,
};
use novex_ai_core::{validate_run_transition, RunEventKind, RunStatus, RunStepType, TaskBudget};
use novex_approval_review::{
    build_guardian_model_review_prompt, guardian_review_failure_decision,
    parse_guardian_model_assessment, review_tool_approval,
    review_tool_approval_with_model_assessment, GuardianApprovalPolicy, GuardianDecisionSource,
    GuardianModelReviewRequest, GuardianReviewDecision, GuardianReviewFailureReason,
    GuardianReviewInput, GuardianReviewStatus, GuardianReviewedAction, GuardianRiskLevel,
    GuardianTranscriptEntry, GuardianTranscriptRole, GuardianUserAuthorization,
};
use novex_memory::{
    build_memory_context, MemoryAccessContext, MemoryContext, MemoryScope, MemoryScopeRef,
    MemorySnippet, MemoryWritePolicy,
};
use novex_model::ModelRoutePurpose;
use novex_tools::{
    agent_model_loop_tool_definitions, agent_model_loop_tool_executor_bindings,
    evaluate_tool_execution_policy, AgentToolExecution, ApprovalPolicy, ToolBatchPlan,
    ToolExecutionPolicyDecision, ToolExecutionPolicyInput, ToolExecutorBinding,
    ToolExecutorDispatchPlan, ToolExecutorRegistry, ToolExecutorRegistryError, ToolRiskLevel,
    ToolRouteError, ToolRouteErrorKind, ToolRouter,
};
use novex_trace::{TraceBundle, TraceEvent, TraceReplaySummary};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::sync::watch;

use super::agent_tool_executor::{
    agent_tool_kind, agent_tool_requires_github_connector_credential,
    agent_tool_requires_mcp_lookup, execute_agent_tool, MEDIA_IMAGE_TOOL_CODE,
};
use super::agent_tool_io_runtime::{execute_agent_tool_io_batch, AgentToolIoMetrics};
use crate::{
    application::ai::model_service::{
        ModelChatCommand, ModelChatCompactionMetadata, ModelChatMessage, ModelChatRequestMetadata,
        ModelChatResp, ModelChatStreamCall, ModelChatUsage, ModelProviderCallContext,
        ModelProviderCallLeaseCancelResp, ModelProviderStreamChunk, ModelProviderStreamEvent,
        ModelRetryPolicy, ModelRuntimeService,
    },
    application::system::{ensure_max_chars, format_datetime},
    infrastructure::persistence::{
        ai_agent_repository::{
            AgentQueueOutboxSaveRecord, AgentRolloutSaveRecord, AgentRunFilter,
            AgentRunQueueSaveRecord, AgentRunRecord, AgentRunSaveRecord, AgentRunStatusUpdate,
            AgentTraceSaveRecord, AgentTurnItemFilter, AgentTurnItemRecord,
            AgentTurnItemSaveRecord, AiAgentRepository, RunEventCursorFilter, RunEventFilter,
            RunEventRecord, RunEventSaveRecord, RunPauseSaveRecord, RunSaveRecord, RunStatusUpdate,
            RunStepSaveRecord,
        },
        ai_capability_repository::{
            AiCapabilityRepository, CapabilityFilter, CapabilityRecord, CapabilityResource,
            SkillResourceRecord, ToolAuditSaveRecord, ToolLookupRecord,
        },
        ai_media_repository::{AiMediaRepository, MediaAssetSaveRecord, MediaJobSaveRecord},
        ai_memory_repository::{AiMemoryRepository, MemoryFilter, MemoryRecord},
    },
    shared::{
        error::AppError,
        id::next_id,
        pagination::{PageQuery, PageResult, DEFAULT_PAGE},
    },
};

const DEFAULT_TENANT_ID: i64 = 1;
const DEFAULT_AGENT_PAGE_SIZE: u64 = 20;
const DEFAULT_EVENT_PAGE_SIZE: u64 = 100;
const DEFAULT_EVENT_STREAM_BATCH_SIZE: u64 = 50;
const MAX_EVENT_STREAM_BATCH_SIZE: u64 = 200;
const DEFAULT_EVENT_STREAM_POLL_MS: u64 = 1000;
const MIN_EVENT_STREAM_POLL_MS: u64 = 250;
const MAX_EVENT_STREAM_POLL_MS: u64 = 5000;
const DEFAULT_EVENT_STREAM_MAX_IDLE_MS: u64 = 30_000;
const MAX_EVENT_STREAM_MAX_IDLE_MS: u64 = 300_000;
const MAX_TRACE_REPLAY_EVENTS: i64 = 1000;
const GITHUB_CONNECTOR_CODE: &str = "github.default";
const AGENT_TOOL_IO_TIMEOUT: Duration = Duration::from_secs(45);
const GUARDIAN_REVIEW_TIMEOUT: Duration = Duration::from_secs(90);
const MODEL_LOOP_PERSISTENT_CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(250);
const MAX_AGENT_MEMORY_SNIPPETS: usize = 6;
const MAX_AGENT_MEMORY_CANDIDATES: i64 = 32;
const CODE_AGENT_MODEL_ROUTE_ID: &str = "runtime.llm.code_agent";
const CODE_AGENT_ROUTE_PURPOSE: &str = "code_agent";
const AGENT_RUN_EXECUTION_MODE_INLINE: &str = "inline";
const AGENT_RUN_EXECUTION_MODE_QUEUED: &str = "queued";
const DEFAULT_AGENT_QUEUE_MAX_ATTEMPTS: i32 = 3;

#[derive(Debug, Clone)]
struct RecordedToolExecution {
    audit_id: i64,
    step_id: i64,
    execution: AgentToolExecution,
    terminal_status: RunStatus,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedAgentToolCall {
    pub(super) batch_index: usize,
    pub(super) call_id: String,
    pub(super) tool: ToolLookupRecord,
    pub(super) arguments: Value,
    pub(super) executor_binding: Option<ToolExecutorBinding>,
    pub(super) concurrency_policy: Value,
    pub(super) timeout: Duration,
}

#[derive(Debug, Clone)]
pub(super) struct ExecutedAgentToolCall {
    pub(super) prepared: PreparedAgentToolCall,
    pub(super) execution: AgentToolExecution,
    pub(super) terminal_status: RunStatus,
    pub(super) tool_io_metrics: Option<AgentToolIoMetrics>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelLoopCancelCheck {
    Continue,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ModelLoopFutureAwait<T> {
    Completed(T),
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelLoopProviderCompletionReason {
    ProviderCompleted,
    StreamedToolCallDetected,
}

#[derive(Debug, Clone)]
struct ModelLoopProviderCompletion<T> {
    response: Option<T>,
    streamed_tool_call_output: Option<ParsedModelTurnOutput>,
    completion_reason: ModelLoopProviderCompletionReason,
    provider_call_lease_id: Option<i64>,
    provider_response_id: Option<String>,
    provider_response_status: Option<String>,
}

#[derive(Debug, Clone)]
struct ModelLoopContextCompactionOutcome {
    summary: String,
    strategy: String,
    status: String,
    cancelled: bool,
    model_payload: Option<Value>,
    error_payload: Option<Value>,
    error_message: Option<String>,
}

#[derive(Debug, Clone)]
struct MediaPersistenceRecords {
    asset: Option<MediaAssetSaveRecord>,
    job: MediaJobSaveRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AgentRunKey {
    tenant_id: i64,
    run_id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRuntimeTaskKind {
    ModelLoop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRuntimeRunStatus {
    Running,
    Cancelling,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRunSnapshot {
    pub tenant_id: i64,
    pub run_id: i64,
    pub task_kind: AgentRuntimeTaskKind,
    pub status: AgentRuntimeRunStatus,
    pub cancel_requested: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRuntimeCancelSignal {
    pub sent: bool,
    pub active_before_cancel: bool,
    pub snapshot: Option<AgentRuntimeRunSnapshot>,
}

#[derive(Debug)]
struct AgentRuntimeRunState {
    sender: watch::Sender<bool>,
    task_kind: AgentRuntimeTaskKind,
    status: AgentRuntimeRunStatus,
    started_at: Instant,
    cancel_requested: bool,
}

impl AgentRuntimeRunState {
    fn new(sender: watch::Sender<bool>, task_kind: AgentRuntimeTaskKind) -> Self {
        Self {
            sender,
            task_kind,
            status: AgentRuntimeRunStatus::Running,
            started_at: Instant::now(),
            cancel_requested: false,
        }
    }

    fn snapshot(&self, key: AgentRunKey) -> AgentRuntimeRunSnapshot {
        AgentRuntimeRunSnapshot {
            tenant_id: key.tenant_id,
            run_id: key.run_id,
            task_kind: self.task_kind,
            status: self.status,
            cancel_requested: self.cancel_requested,
            elapsed_ms: self.started_at.elapsed().as_millis() as u64,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AgentRuntimeRegistry {
    inner: Arc<Mutex<HashMap<AgentRunKey, AgentRuntimeRunState>>>,
}

#[derive(Debug)]
pub struct ActiveAgentRunGuard {
    key: AgentRunKey,
    registry: AgentRuntimeRegistry,
}

#[derive(Debug, Clone)]
pub struct AgentRunCancellationToken {
    receiver: watch::Receiver<bool>,
}

impl AgentRuntimeRegistry {
    pub fn register_run(
        &self,
        tenant_id: i64,
        run_id: i64,
    ) -> (ActiveAgentRunGuard, AgentRunCancellationToken) {
        self.register_run_with_kind(tenant_id, run_id, AgentRuntimeTaskKind::ModelLoop)
    }

    pub fn register_run_with_kind(
        &self,
        tenant_id: i64,
        run_id: i64,
        task_kind: AgentRuntimeTaskKind,
    ) -> (ActiveAgentRunGuard, AgentRunCancellationToken) {
        let key = AgentRunKey { tenant_id, run_id };
        let (sender, receiver) = watch::channel(false);
        let mut inner = self.inner.lock().unwrap();
        if let Some(previous) = inner.insert(key, AgentRuntimeRunState::new(sender, task_kind)) {
            let _ = previous.sender.send(true);
        }
        (
            ActiveAgentRunGuard {
                key,
                registry: self.clone(),
            },
            AgentRunCancellationToken { receiver },
        )
    }

    pub fn cancel_run(&self, tenant_id: i64, run_id: i64) -> bool {
        self.cancel_run_signal(tenant_id, run_id).sent
    }

    pub fn cancel_run_signal(&self, tenant_id: i64, run_id: i64) -> AgentRuntimeCancelSignal {
        let key = AgentRunKey { tenant_id, run_id };
        let mut inner = self.inner.lock().unwrap();
        let Some(state) = inner.get_mut(&key) else {
            return AgentRuntimeCancelSignal {
                sent: false,
                active_before_cancel: false,
                snapshot: None,
            };
        };

        state.status = AgentRuntimeRunStatus::Cancelling;
        state.cancel_requested = true;
        let sent = state.sender.send(true).is_ok();
        AgentRuntimeCancelSignal {
            sent,
            active_before_cancel: true,
            snapshot: Some(state.snapshot(key)),
        }
    }

    pub fn active_run_snapshots(&self) -> Vec<AgentRuntimeRunSnapshot> {
        let mut snapshots = self
            .inner
            .lock()
            .unwrap()
            .iter()
            .map(|(key, state)| state.snapshot(*key))
            .collect::<Vec<_>>();
        snapshots.sort_by_key(|snapshot| (snapshot.tenant_id, snapshot.run_id));
        snapshots
    }

    fn unregister_run(&self, key: AgentRunKey) {
        self.inner.lock().unwrap().remove(&key);
    }
}

impl Drop for ActiveAgentRunGuard {
    fn drop(&mut self) {
        self.registry.unregister_run(self.key);
    }
}

impl AgentRunCancellationToken {
    pub fn is_cancelled(&self) -> bool {
        *self.receiver.borrow()
    }

    pub async fn cancelled(mut self) {
        if self.is_cancelled() {
            return;
        }
        while self.receiver.changed().await.is_ok() {
            if self.is_cancelled() {
                return;
            }
        }
        std::future::pending::<()>().await;
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWorkbenchContext {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub dataset_id: Option<i64>,
    #[serde(default)]
    pub document_ids: Vec<i64>,
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub skill_codes: Vec<String>,
    #[serde(default)]
    pub mcp_tool_codes: Vec<String>,
    #[serde(default)]
    pub web_search_enabled: bool,
    #[serde(default)]
    pub route_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunCommand {
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub runtime_mode: Option<String>,
    #[serde(default)]
    pub execution_mode: Option<String>,
    #[serde(default)]
    pub model_route_id: Option<String>,
    #[serde(default)]
    pub auto_approve: bool,
    #[serde(default)]
    pub budget: TaskBudget,
    #[serde(default)]
    pub workbench_context: Option<AgentWorkbenchContext>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunResumeCommand {
    #[serde(default)]
    pub approved: bool,
    #[serde(default)]
    pub input: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_agent_size")]
    pub size: u64,
    #[serde(default)]
    pub status: Option<String>,
}

impl Default for AgentRunQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            size: DEFAULT_AGENT_PAGE_SIZE,
            status: None,
        }
    }
}

impl AgentRunQuery {
    pub fn page_query(&self) -> PageQuery {
        PageQuery {
            page: self.page,
            size: self.size,
        }
        .normalized()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunEventQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_event_size")]
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunEventStreamQuery {
    #[serde(default)]
    pub after_sequence_no: i64,
    pub batch_size: Option<u64>,
    pub poll_ms: Option<u64>,
    pub max_idle_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRunEventStreamSettings {
    pub after_sequence_no: i64,
    pub batch_size: i64,
    pub poll_ms: u64,
    pub max_idle_ms: u64,
}

impl Default for AgentRunEventStreamQuery {
    fn default() -> Self {
        Self {
            after_sequence_no: 0,
            batch_size: None,
            poll_ms: None,
            max_idle_ms: None,
        }
    }
}

impl AgentRunEventStreamQuery {
    pub fn settings(&self) -> AgentRunEventStreamSettings {
        AgentRunEventStreamSettings {
            after_sequence_no: self.after_sequence_no.max(0),
            batch_size: self
                .batch_size
                .unwrap_or(DEFAULT_EVENT_STREAM_BATCH_SIZE)
                .clamp(1, MAX_EVENT_STREAM_BATCH_SIZE) as i64,
            poll_ms: self
                .poll_ms
                .unwrap_or(DEFAULT_EVENT_STREAM_POLL_MS)
                .clamp(MIN_EVENT_STREAM_POLL_MS, MAX_EVENT_STREAM_POLL_MS),
            max_idle_ms: self
                .max_idle_ms
                .unwrap_or(DEFAULT_EVENT_STREAM_MAX_IDLE_MS)
                .clamp(1, MAX_EVENT_STREAM_MAX_IDLE_MS),
        }
    }
}

impl Default for AgentRunEventQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            size: DEFAULT_EVENT_PAGE_SIZE,
        }
    }
}

impl AgentRunEventQuery {
    pub fn page_query(&self) -> PageQuery {
        PageQuery {
            page: self.page,
            size: self.size,
        }
        .normalized()
    }
}

struct AgentStatusUpdate<'a> {
    user_id: i64,
    run_id: i64,
    status: String,
    output_payload: Value,
    final_output: Option<&'a str>,
    pause_reason: Option<&'a str>,
    finished: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunResp {
    pub run_id: i64,
    pub trace_id: String,
    pub status: String,
    pub intent: String,
    pub loop_kind: String,
    pub selected_tool_code: Option<String>,
    pub pause_reason: Option<String>,
    pub final_output: Option<String>,
    pub task_budget: TaskBudget,
    pub create_time: String,
    pub update_time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunEventResp {
    pub id: i64,
    pub run_id: i64,
    pub step_id: Option<i64>,
    pub event_type: String,
    pub sequence_no: i64,
    pub status: String,
    pub payload: Value,
    pub create_time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTraceReplayResp {
    pub trace_id: String,
    pub events: Vec<TraceEvent>,
    pub summary: TraceReplaySummary,
    #[serde(default)]
    pub turn_items: Vec<AgentTurnItem>,
}

#[derive(Debug, Clone)]
pub struct AgentService {
    tenant_id: i64,
    repo: AiAgentRepository,
    capability_repo: AiCapabilityRepository,
    media_repo: AiMediaRepository,
    memory_repo: AiMemoryRepository,
    model_runtime: ModelRuntimeService,
    agent_runtime: AgentRuntimeRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentPlanSummary {
    intent: String,
    loop_kind: String,
    selected_tool_code: Option<String>,
    requires_approval: bool,
    pause_reason: Option<String>,
    initial_status: String,
    task_budget: TaskBudget,
    memory_context: MemoryContext,
}

impl AgentService {
    pub fn new(db: PgPool) -> Self {
        Self::for_tenant(db, DEFAULT_TENANT_ID)
    }

    pub fn for_tenant(db: PgPool, tenant_id: i64) -> Self {
        Self::for_tenant_with_runtime(db, tenant_id, AgentRuntimeRegistry::default())
    }

    pub fn for_tenant_with_runtime(
        db: PgPool,
        tenant_id: i64,
        agent_runtime: AgentRuntimeRegistry,
    ) -> Self {
        Self {
            tenant_id,
            repo: AiAgentRepository::new(db.clone()),
            capability_repo: AiCapabilityRepository::new(db.clone()),
            media_repo: AiMediaRepository::new(db.clone()),
            memory_repo: AiMemoryRepository::new(db.clone()),
            model_runtime: ModelRuntimeService::for_tenant(db.clone(), tenant_id),
            agent_runtime,
        }
    }

    pub async fn create_run(
        &self,
        user_id: i64,
        command: AgentRunCommand,
    ) -> Result<AgentRunResp, AppError> {
        let command = normalize_agent_run_command(command)?;
        if command.execution_mode.as_deref() == Some(AGENT_RUN_EXECUTION_MODE_QUEUED) {
            return self.create_queued_run(user_id, command).await;
        }
        if command.runtime_mode.as_deref() == Some("model_loop") {
            return self.create_model_loop_run(user_id, command).await;
        }
        let memory_context = self.load_agent_memory_context(user_id).await?;
        let mut plan = build_agent_plan(&command, memory_context)?;
        let selected_tool = if let Some(tool_code) = plan.selected_tool_code.as_deref() {
            let Some(tool) = self
                .capability_repo
                .find_tool_by_code(self.tenant_id, tool_code)
                .await?
            else {
                return Err(AppError::NotFound);
            };
            let policy = agent_tool_policy_decision(&tool, command.auto_approve);
            plan.requires_approval = policy.requires_approval;
            plan.pause_reason = policy.pause_reason;
            Some(tool)
        } else {
            None
        };
        let run_id = next_id();
        let trace_id = format!("agent-{run_id}");
        let now = Utc::now().naive_utc();

        self.create_run_records(user_id, run_id, &trace_id, &command, &plan, now)
            .await?;
        let input_item = novex_agent_protocol::AgentTurnItem::user_message(command.input.as_str());
        let mut input_payload = agent_turn_item_event_payload(&input_item);
        if let Some(object) = input_payload.as_object_mut() {
            object.insert("input".to_owned(), json!(&command.input));
        }
        self.append_event(
            user_id,
            run_id,
            None,
            RunEventKind::InputReceived,
            run_status_code(RunStatus::Running),
            input_payload,
        )
        .await?;
        self.execute_deterministic_plan(user_id, run_id, command, plan, selected_tool, now)
            .await?;

        self.get_run(run_id).await
    }

    async fn execute_deterministic_plan(
        &self,
        user_id: i64,
        run_id: i64,
        command: AgentRunCommand,
        plan: AgentPlanSummary,
        selected_tool: Option<ToolLookupRecord>,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        let initial_status = if plan.requires_approval {
            run_status_code(RunStatus::WaitingApproval)
        } else {
            run_status_code(RunStatus::Running)
        };
        self.append_event(
            user_id,
            run_id,
            None,
            RunEventKind::IntentRouted,
            run_status_code(RunStatus::Running),
            json!({
                "intent": plan.intent,
                "loopKind": plan.loop_kind,
                "selectedToolCode": plan.selected_tool_code
            }),
        )
        .await?;
        self.append_event(
            user_id,
            run_id,
            None,
            RunEventKind::Thought,
            run_status_code(RunStatus::Running),
            json!({ "message": "deterministic ReAct thought prepared" }),
        )
        .await?;
        self.record_retrieval_context(user_id, run_id, &command.input, &plan.memory_context)
            .await?;

        if let Some(tool) = selected_tool {
            if plan.requires_approval {
                let tool_arguments = json!({ "input": command.input.clone() });
                let guardian_review_decision = self
                    .guardian_review_decision_for_tool_policy(
                        &command.input,
                        None,
                        &tool,
                        tool_arguments.clone(),
                        command.auto_approve,
                    )
                    .await;
                let guardian_review =
                    guardian_review_payload_from_decision(&guardian_review_decision);
                if guardian_auto_approval_allows_execution(&guardian_review_decision) {
                    self.append_event(
                        user_id,
                        run_id,
                        None,
                        RunEventKind::ActionSelected,
                        run_status_code(RunStatus::Running),
                        json!({
                            "toolCode": tool.code,
                            "riskLevel": tool.risk_level,
                            "approvalMode": "guardian_auto_approved",
                            "guardianAutoApproved": true,
                            "guardianReview": guardian_review
                        }),
                    )
                    .await?;
                    self.execute_tool_and_finish(user_id, run_id, &tool, tool_arguments)
                        .await?;
                } else {
                    let guardian_review_override = Some(guardian_review);
                    self.pause_for_approval(
                        user_id,
                        run_id,
                        &tool,
                        &command.input,
                        None,
                        tool_arguments,
                        command.auto_approve,
                        guardian_review_override,
                        now,
                    )
                    .await?;
                    self.update_status(AgentStatusUpdate {
                        user_id,
                        run_id,
                        status: initial_status,
                        output_payload: Value::Null,
                        final_output: None,
                        pause_reason: Some("approval"),
                        finished: false,
                    })
                    .await?;
                    self.append_event(
                        user_id,
                        run_id,
                        None,
                        RunEventKind::StatusChanged,
                        run_status_code(RunStatus::WaitingApproval),
                        json!({ "status": run_status_code(RunStatus::WaitingApproval) }),
                    )
                    .await?;
                    self.refresh_trace_snapshot(user_id, run_id, Value::Null)
                        .await?;
                }
            } else {
                self.execute_tool_and_finish(
                    user_id,
                    run_id,
                    &tool,
                    json!({ "input": command.input }),
                )
                .await?;
            }
        } else {
            let final_output = final_output_for_intent(&plan.intent);
            self.finish_without_tool(user_id, run_id, &final_output)
                .await?;
            self.refresh_trace_snapshot(user_id, run_id, Value::Null)
                .await?;
        }
        Ok(())
    }

    async fn create_queued_run(
        &self,
        user_id: i64,
        command: AgentRunCommand,
    ) -> Result<AgentRunResp, AppError> {
        let memory_context = self.load_agent_memory_context(user_id).await?;
        let mut plan = build_agent_plan(&command, memory_context)?;
        if command.runtime_mode.as_deref() == Some("model_loop") {
            plan.loop_kind = "model_loop".to_owned();
            plan.selected_tool_code = None;
            plan.requires_approval = false;
            plan.pause_reason = None;
        } else if let Some(tool_code) = plan.selected_tool_code.as_deref() {
            let Some(tool) = self
                .capability_repo
                .find_tool_by_code(self.tenant_id, tool_code)
                .await?
            else {
                return Err(AppError::NotFound);
            };
            let policy = agent_tool_policy_decision(&tool, command.auto_approve);
            plan.requires_approval = policy.requires_approval;
            plan.pause_reason = policy.pause_reason;
        }

        let run_id = next_id();
        let trace_id = format!("agent-{run_id}");
        let now = Utc::now().naive_utc();
        self.create_run_records_with_status(
            user_id,
            run_id,
            &trace_id,
            &command,
            &plan,
            RunStatus::Queued,
            now,
        )
        .await?;

        let input_item = novex_agent_protocol::AgentTurnItem::user_message(command.input.as_str());
        let mut input_payload = agent_turn_item_event_payload(&input_item);
        if let Some(object) = input_payload.as_object_mut() {
            object.insert("input".to_owned(), json!(&command.input));
            object.insert("executionMode".to_owned(), json!("queued"));
            if let Some(runtime_mode) = command.runtime_mode.as_deref() {
                object.insert("runtimeMode".to_owned(), json!(runtime_mode));
            }
        }
        self.append_event(
            user_id,
            run_id,
            None,
            RunEventKind::InputReceived,
            run_status_code(RunStatus::Queued),
            input_payload,
        )
        .await?;
        self.append_event(
            user_id,
            run_id,
            None,
            RunEventKind::StatusChanged,
            run_status_code(RunStatus::Queued),
            json!({
                "status": run_status_code(RunStatus::Queued),
                "executionMode": "queued",
                "runtimeMode": command.runtime_mode
            }),
        )
        .await?;
        let queue_id = next_id();
        let queue_payload = json!({
            "command": agent_run_command_payload(&command),
            "executionMode": "queued",
            "source": "agent.create_run"
        });
        let queue_record = AgentRunQueueSaveRecord {
            id: queue_id,
            tenant_id: self.tenant_id,
            run_id,
            priority: 0,
            max_attempts: DEFAULT_AGENT_QUEUE_MAX_ATTEMPTS,
            payload: queue_payload.clone(),
            user_id,
            now,
        };
        self.repo
            .enqueue_agent_run_with_outbox(
                &queue_record,
                &AgentQueueOutboxSaveRecord {
                    id: next_id(),
                    tenant_id: self.tenant_id,
                    queue_id,
                    run_id,
                    event_type: "agent.run.queued".to_owned(),
                    max_attempts: DEFAULT_AGENT_QUEUE_MAX_ATTEMPTS,
                    payload: json!({
                        "source": "agent.create_run",
                        "executionMode": "queued"
                    }),
                    status: 1,
                    attempt_count: 0,
                    user_id,
                    now,
                },
            )
            .await?;
        self.refresh_trace_snapshot(
            user_id,
            run_id,
            json!({
                "executionMode": "queued",
                "runtimeMode": command.runtime_mode
            }),
        )
        .await?;

        self.get_run(run_id).await
    }

    pub async fn execute_queued_run(
        &self,
        user_id: i64,
        run_id: i64,
        queue_payload: Value,
    ) -> Result<AgentRunResp, AppError> {
        if let Some(resume_input) = agent_resume_input_from_queue_payload(&queue_payload)? {
            let run = self.get_run(run_id).await?;
            let Some(current_status) = parse_run_status_code(&run.status) else {
                return Err(AppError::conflict(format!("未知 Run 状态: {}", run.status)));
            };
            if current_status.is_terminal() || current_status == RunStatus::WaitingApproval {
                return Ok(run);
            }
            if current_status != RunStatus::Running {
                ensure_agent_run_transition(&run.status, RunStatus::Running)?;
                self.update_status(AgentStatusUpdate {
                    user_id,
                    run_id,
                    status: run_status_code(RunStatus::Running),
                    output_payload: Value::Null,
                    final_output: None,
                    pause_reason: None,
                    finished: false,
                })
                .await?;
                self.append_event(
                    user_id,
                    run_id,
                    None,
                    RunEventKind::StatusChanged,
                    run_status_code(RunStatus::Running),
                    json!({
                        "status": run_status_code(RunStatus::Running),
                        "executionMode": "queued",
                        "resumeQueued": true
                    }),
                )
                .await?;
            }
            self.execute_resumed_tool_and_finish(user_id, run_id, resume_input)
                .await?;
            return self.get_run(run_id).await;
        }

        let command = agent_run_command_from_queue_payload(queue_payload)?;
        let run = self.get_run(run_id).await?;
        let Some(current_status) = parse_run_status_code(&run.status) else {
            return Err(AppError::conflict(format!("未知 Run 状态: {}", run.status)));
        };
        if current_status.is_terminal() || current_status == RunStatus::WaitingApproval {
            return Ok(run);
        }
        if current_status != RunStatus::Running {
            ensure_agent_run_transition(&run.status, RunStatus::Running)?;
            self.update_status(AgentStatusUpdate {
                user_id,
                run_id,
                status: run_status_code(RunStatus::Running),
                output_payload: Value::Null,
                final_output: None,
                pause_reason: None,
                finished: false,
            })
            .await?;
            self.append_event(
                user_id,
                run_id,
                None,
                RunEventKind::StatusChanged,
                run_status_code(RunStatus::Running),
                json!({
                    "status": run_status_code(RunStatus::Running),
                    "executionMode": "queued"
                }),
            )
            .await?;
        }
        if command.runtime_mode.as_deref() == Some("model_loop") {
            return self
                .execute_model_loop_existing_run(user_id, run_id, command, false)
                .await;
        }

        let memory_context = self.load_agent_memory_context(user_id).await?;
        let mut plan = build_agent_plan(&command, memory_context)?;
        let selected_tool = if let Some(tool_code) = plan.selected_tool_code.as_deref() {
            let Some(tool) = self
                .capability_repo
                .find_tool_by_code(self.tenant_id, tool_code)
                .await?
            else {
                return Err(AppError::NotFound);
            };
            let policy = agent_tool_policy_decision(&tool, command.auto_approve);
            plan.requires_approval = policy.requires_approval;
            plan.pause_reason = policy.pause_reason;
            Some(tool)
        } else {
            None
        };
        self.execute_deterministic_plan(
            user_id,
            run_id,
            command,
            plan,
            selected_tool,
            Utc::now().naive_utc(),
        )
        .await?;

        self.get_run(run_id).await
    }

    async fn create_model_loop_run(
        &self,
        user_id: i64,
        command: AgentRunCommand,
    ) -> Result<AgentRunResp, AppError> {
        let memory_context = self.load_agent_memory_context(user_id).await?;
        let mut plan = build_agent_plan(&command, memory_context)?;
        plan.loop_kind = "model_loop".to_owned();
        plan.selected_tool_code = None;
        plan.requires_approval = false;
        plan.pause_reason = None;

        let run_id = next_id();
        let trace_id = format!("agent-{run_id}");
        let now = Utc::now().naive_utc();
        self.create_run_records(user_id, run_id, &trace_id, &command, &plan, now)
            .await?;
        self.execute_model_loop_existing_run(user_id, run_id, command, true)
            .await
    }

    async fn record_model_loop_input_event(
        &self,
        user_id: i64,
        run_id: i64,
        command: &AgentRunCommand,
        input_item: &AgentTurnItem,
    ) -> Result<(), AppError> {
        let mut input_payload = agent_turn_item_event_payload(input_item);
        if let Some(object) = input_payload.as_object_mut() {
            object.insert("input".to_owned(), json!(&command.input));
            object.insert("runtimeMode".to_owned(), json!("model_loop"));
            object.insert(
                "workbenchContext".to_owned(),
                json!(&command.workbench_context),
            );
        }
        self.append_event(
            user_id,
            run_id,
            None,
            RunEventKind::InputReceived,
            run_status_code(RunStatus::Running),
            input_payload,
        )
        .await
    }

    async fn resolve_agent_skill_context(
        &self,
        context: Option<&AgentWorkbenchContext>,
        question: &str,
    ) -> Result<Option<String>, AppError> {
        let Some(context) = context else {
            return Ok(None);
        };
        if context.skill_codes.is_empty() {
            return Ok(None);
        }

        let mut sections = Vec::new();
        for skill_code in &context.skill_codes {
            let mut records = self
                .capability_repo
                .list(
                    CapabilityResource::Skill,
                    &CapabilityFilter {
                        tenant_id: self.tenant_id,
                        status: Some(1),
                        kind: Some(skill_code.as_str()),
                        limit: 1,
                        offset: 0,
                    },
                )
                .await?;
            let Some(record) = records.pop() else {
                return Err(AppError::bad_request(format!(
                    "Skill `{skill_code}` 不存在或已停用"
                )));
            };
            let resources = self
                .capability_repo
                .list_skill_resources(self.tenant_id, record.id, None)
                .await?;
            sections.push(agent_skill_context_for_record(
                &record, &resources, question,
            ));
        }

        Ok((!sections.is_empty())
            .then(|| preview_chars(&sections.join("\n\n---\n\n"), AGENT_SKILL_CONTEXT_CHARS)))
    }

    async fn execute_model_loop_existing_run(
        &self,
        user_id: i64,
        run_id: i64,
        command: AgentRunCommand,
        record_input_event: bool,
    ) -> Result<AgentRunResp, AppError> {
        let model_retry_policy: ModelRetryPolicy = self
            .model_runtime
            .retry_policy_for_purpose_with_route_id(
                ModelRoutePurpose::CodeAgent,
                command.model_route_id.as_deref(),
            )
            .await?;
        let (_active_run_guard, cancel_token) =
            self.agent_runtime.register_run(self.tenant_id, run_id);

        let input_item = novex_agent_protocol::AgentTurnItem::user_message(command.input.as_str());
        let mut runtime_state = AgentRuntimeState::with_budget(
            run_id.to_string(),
            agent_runtime_budget_from_task_budget(command.budget),
        );
        runtime_state.push_item(input_item.clone());
        if record_input_event {
            self.record_model_loop_input_event(user_id, run_id, &command, &input_item)
                .await?;
        }

        let tool_router = build_model_loop_tool_router().map_err(tool_route_error_to_app_error)?;
        let executor_registry = build_model_loop_tool_executor_registry()
            .map_err(tool_executor_registry_error_to_app_error)?;
        let tool_codes = tool_router.tool_codes();
        let skill_context = self
            .resolve_agent_skill_context(command.workbench_context.as_ref(), &command.input)
            .await?;
        let mut last_tool_terminal_status = RunStatus::Succeeded;

        for _turn_index in 0..runtime_state.budget.max_turns {
            if self
                .check_model_loop_cancelled(user_id, run_id, "before_model_call")
                .await?
                == ModelLoopCancelCheck::Cancelled
            {
                return self.get_run(run_id).await;
            }

            let mut model_response = None;
            let mut streamed_tool_call_output = None;
            for attempt in 1..=model_retry_policy.max_attempts() {
                let model_call_started = Instant::now();
                let provider_attempt_kind = if attempt == 1 { "primary" } else { "retry" };
                let messages = build_model_loop_messages_from_history(
                    &command.input,
                    &tool_codes,
                    command.workbench_context.as_ref(),
                    skill_context.as_deref(),
                    &runtime_state.items,
                );
                let model_stream_call = self
                    .model_runtime
                    .chat_completion_stream_for_purpose(
                        ModelRoutePurpose::CodeAgent,
                        ModelChatCommand {
                            route_id: command.model_route_id.clone(),
                            messages: messages.clone(),
                            temperature: Some(0.2),
                            max_tokens: Some(1024),
                            provider_call_context: Some(ModelProviderCallContext {
                                run_id: Some(run_id),
                                source: "agent.model_loop".to_owned(),
                                route_purpose: Some(ModelRoutePurpose::CodeAgent),
                                attempt_kind: provider_attempt_kind.to_owned(),
                            }),
                            ..ModelChatCommand::default()
                        },
                    )
                    .await?;
                let model_call_result =
                    await_model_loop_stream_call_or_cancelled_with_delta_events(
                        cancel_token.clone(),
                        self.wait_for_model_loop_persistent_cancel(run_id),
                        "model_call",
                        self,
                        user_id,
                        run_id,
                        model_stream_call,
                    )
                    .await;
                match model_call_result {
                    Ok(ModelLoopFutureAwait::Completed(completion)) => {
                        let completion_reason = completion.completion_reason;
                        let provider_call_lease_id = completion.provider_call_lease_id;
                        let provider_response_id = completion.provider_response_id;
                        let provider_response_status = completion.provider_response_status;
                        streamed_tool_call_output = completion.streamed_tool_call_output;
                        if matches!(
                            completion_reason,
                            ModelLoopProviderCompletionReason::StreamedToolCallDetected
                        ) && streamed_tool_call_output.is_none()
                        {
                            return Err(AppError::bad_request(
                                "Agent 模型输出解析失败: streamed tool-call early stop missing parsed output",
                            ));
                        }
                        if matches!(
                            completion_reason,
                            ModelLoopProviderCompletionReason::StreamedToolCallDetected
                        ) {
                            self.try_cancel_streamed_provider_call(
                                user_id,
                                run_id,
                                provider_call_lease_id,
                                provider_response_id.as_deref(),
                            )
                            .await?;
                        }
                        let _stream_provider_response_metadata_present = provider_call_lease_id
                            .is_some()
                            || provider_response_id.is_some()
                            || provider_response_status.is_some();
                        model_response = completion.response;
                        break;
                    }
                    Ok(ModelLoopFutureAwait::Cancelled) => {
                        if self
                            .check_model_loop_cancelled(user_id, run_id, "model_call")
                            .await?
                            == ModelLoopCancelCheck::Continue
                        {
                            self.finish_model_loop_cancelled(
                                user_id,
                                run_id,
                                &run_status_code(RunStatus::Cancelling),
                                "model_call",
                            )
                            .await?;
                        }
                        return self.get_run(run_id).await;
                    }
                    Err(err) => {
                        let latency_ms = model_call_started.elapsed().as_millis();
                        let will_retry = model_inference_error_should_retry(&err)
                            && attempt < model_retry_policy.max_attempts();
                        let error_payload = model_inference_error_attempt_event_payload(
                            &err,
                            latency_ms,
                            attempt,
                            model_retry_policy.max_attempts(),
                            will_retry,
                        );
                        self.append_event(
                            user_id,
                            run_id,
                            None,
                            RunEventKind::Thought,
                            if will_retry {
                                run_status_code(RunStatus::Running)
                            } else {
                                run_status_code(RunStatus::Failed)
                            },
                            error_payload,
                        )
                        .await?;
                        if will_retry {
                            tokio::time::sleep(model_inference_retry_delay(attempt)).await;
                            continue;
                        }
                        let error_message = model_inference_error_message(&err);
                        self.append_event(
                            user_id,
                            run_id,
                            None,
                            RunEventKind::Error,
                            run_status_code(RunStatus::Failed),
                            json!({
                                "runtimeMode": "model_loop",
                                "stopReason": "model_call_failed",
                                "message": error_message.clone()
                            }),
                        )
                        .await?;
                        self.finish_model_loop_run(
                            user_id,
                            run_id,
                            None,
                            RunStatus::Failed,
                            &error_message,
                            json!({
                                "answer": error_message.clone(),
                                "runtimeMode": "model_loop",
                                "stopReason": "model_call_failed"
                            }),
                            agent_turn_item_event_payload(&AgentTurnItem::FinalAnswer {
                                content: error_message.clone(),
                            }),
                        )
                        .await?;
                        self.refresh_trace_snapshot(
                            user_id,
                            run_id,
                            json!({
                                "runtimeMode": "model_loop",
                                "stopReason": "model_call_failed"
                            }),
                        )
                        .await?;
                        return self.get_run(run_id).await;
                    }
                }
            }
            if let Some(model_response) = model_response.as_ref() {
                self.append_event(
                    user_id,
                    run_id,
                    None,
                    RunEventKind::Thought,
                    run_status_code(RunStatus::Running),
                    model_inference_event_payload(model_response),
                )
                .await?;
            }

            let parsed = model_loop_parse_turn_output(
                model_response.as_ref(),
                streamed_tool_call_output.as_ref(),
            )?;
            let parsed_items = parsed.items.clone();
            let parsed_payload = agent_turn_item_event_payload(&parsed.item);
            if self
                .check_model_loop_cancelled(user_id, run_id, "after_model_call")
                .await?
                == ModelLoopCancelCheck::Cancelled
            {
                return self.get_run(run_id).await;
            }

            match parsed.item {
                novex_agent_protocol::AgentTurnItem::FinalAnswer { content } => {
                    runtime_state.push_item(AgentTurnItem::FinalAnswer {
                        content: content.clone(),
                    });
                    self.finish_model_loop_run(
                        user_id,
                        run_id,
                        None,
                        last_tool_terminal_status,
                        &content,
                        json!({ "answer": content.clone(), "runtimeMode": "model_loop" }),
                        parsed_payload,
                    )
                    .await?;
                    self.refresh_trace_snapshot(
                        user_id,
                        run_id,
                        json!({ "runtimeMode": "model_loop" }),
                    )
                    .await?;
                    return self.get_run(run_id).await;
                }
                AgentTurnItem::ToolCall {
                    call_id: _,
                    tool_code: _,
                    arguments: _,
                } => {
                    let tool_call_items = parsed_items
                        .into_iter()
                        .filter_map(|item| match item {
                            AgentTurnItem::ToolCall {
                                call_id,
                                tool_code,
                                arguments,
                            } => Some((call_id, tool_code, arguments)),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let tool_call_count = tool_call_items.len();
                    let requested_batch_payload = tool_call_items
                        .iter()
                        .map(|(call_id, tool_code, arguments)| {
                            json!({
                                "callId": call_id,
                                "toolCode": tool_code,
                                "arguments": arguments,
                            })
                        })
                        .collect::<Vec<_>>();
                    let can_execute_requested_tool_calls = if tool_call_count == 1 {
                        runtime_state.can_execute_tool_call()
                    } else {
                        runtime_state.can_execute_tool_calls(tool_call_count)
                    };
                    if !can_execute_requested_tool_calls {
                        let final_output = format!(
                            "Tool call budget exhausted before executing requested batch of {tool_call_count} tool calls."
                        );
                        let final_payload =
                            agent_turn_item_event_payload(&AgentTurnItem::FinalAnswer {
                                content: final_output.clone(),
                            });
                        self.append_event(
                            user_id,
                            run_id,
                                None,
                                RunEventKind::ActionSelected,
                                run_status_code(RunStatus::Failed),
                                json!({
                                    "requestedToolCallCount": tool_call_count,
                                    "remainingToolCallBudget": runtime_state.remaining_tool_call_budget(),
                                    "toolCallBatch": requested_batch_payload,
                                    "runtimeMode": "model_loop",
                                    "stopReason": "tool_call_budget_exhausted"
                                }),
                            )
                            .await?;
                        self.finish_model_loop_run(
                            user_id,
                            run_id,
                            None,
                            RunStatus::Failed,
                            &final_output,
                            json!({
                                "answer": final_output.clone(),
                                "runtimeMode": "model_loop",
                                "stopReason": "tool_call_budget_exhausted"
                            }),
                            final_payload,
                        )
                        .await?;
                        self.refresh_trace_snapshot(
                            user_id,
                            run_id,
                            json!({
                                "runtimeMode": "model_loop",
                                "stopReason": "tool_call_budget_exhausted"
                            }),
                        )
                        .await?;
                        return self.get_run(run_id).await;
                    }

                    let mut routed_calls = Vec::with_capacity(tool_call_count);
                    for (call_id, tool_code, arguments) in tool_call_items {
                        match tool_router.route_tool_call(&call_id, &tool_code, arguments.clone()) {
                            Ok(routed_call) => routed_calls.push(routed_call),
                            Err(err) => {
                                let stop_reason = tool_route_stop_reason(err.kind);
                                let final_output = tool_route_failure_message(&err);
                                let final_payload =
                                    agent_turn_item_event_payload(&AgentTurnItem::FinalAnswer {
                                        content: final_output.clone(),
                                    });
                                self.append_event(
                                    user_id,
                                    run_id,
                                    None,
                                    RunEventKind::ActionSelected,
                                    run_status_code(RunStatus::Failed),
                                    json!({
                                        "toolCode": tool_code,
                                        "arguments": arguments,
                                        "toolCallBatch": requested_batch_payload,
                                        "runtimeMode": "model_loop",
                                        "stopReason": stop_reason,
                                        "toolRouteError": err
                                    }),
                                )
                                .await?;
                                self.finish_model_loop_run(
                                    user_id,
                                    run_id,
                                    None,
                                    RunStatus::Failed,
                                    &final_output,
                                    json!({
                                        "answer": final_output.clone(),
                                        "runtimeMode": "model_loop",
                                        "stopReason": stop_reason
                                    }),
                                    final_payload,
                                )
                                .await?;
                                self.refresh_trace_snapshot(
                                    user_id,
                                    run_id,
                                    json!({
                                        "runtimeMode": "model_loop",
                                        "stopReason": stop_reason
                                    }),
                                )
                                .await?;
                                return self.get_run(run_id).await;
                            }
                        }
                    }

                    let batch_plan = ToolBatchPlan::from_routed_calls(routed_calls);
                    let batch_execution_mode = batch_plan.mode;
                    let batch_execution_mode_payload =
                        serde_json::to_value(batch_execution_mode).unwrap_or(Value::Null);
                    let serial_reason = batch_plan.serial_reason.clone();
                    let tool_call_batch_payload = batch_plan
                        .calls
                        .iter()
                        .map(|call| {
                            json!({
                                "callId": call.call_id,
                                "toolCode": call.tool.code,
                                "arguments": call.arguments,
                                "concurrencyPolicy": call.tool.concurrency,
                            })
                        })
                        .collect::<Vec<_>>();
                    let batch_size = batch_plan.calls.len();
                    let mut prepared_calls = Vec::with_capacity(batch_size);
                    let mut guardian_auto_approved_calls = HashMap::new();

                    for (batch_index, routed_call) in batch_plan.calls.into_iter().enumerate() {
                        let call_id = routed_call.call_id;
                        let concurrency_policy_payload =
                            serde_json::to_value(&routed_call.tool.concurrency)
                                .unwrap_or(Value::Null);
                        let executor_binding = executor_registry
                            .executor_for(&routed_call.tool.code)
                            .map_err(tool_executor_registry_error_to_app_error)?
                            .clone();
                        let executor_binding_payload =
                            model_loop_tool_executor_binding_payload(Some(&executor_binding));
                        let tool_code = routed_call.tool.code;
                        let arguments = routed_call.arguments;
                        let Some(tool) = self
                            .capability_repo
                            .find_tool_by_code(self.tenant_id, &tool_code)
                            .await?
                        else {
                            return Err(AppError::NotFound);
                        };
                        let batch_policy = agent_tool_policy_decision(&tool, command.auto_approve);
                        let prepared_call = PreparedAgentToolCall {
                            batch_index,
                            call_id: call_id.clone(),
                            tool,
                            arguments: arguments.clone(),
                            executor_binding: Some(executor_binding.clone()),
                            concurrency_policy: concurrency_policy_payload.clone(),
                            timeout: AGENT_TOOL_IO_TIMEOUT,
                        };
                        if batch_policy.requires_approval {
                            let guardian_review_decision = self
                                .guardian_review_decision_for_tool_policy(
                                    &command.input,
                                    Some(&runtime_state.items),
                                    &prepared_call.tool,
                                    prepared_call.arguments.clone(),
                                    command.auto_approve,
                                )
                                .await;
                            let guardian_review =
                                guardian_review_payload_from_decision(&guardian_review_decision);
                            if guardian_auto_approval_allows_execution(&guardian_review_decision) {
                                guardian_auto_approved_calls
                                    .insert(prepared_call.call_id.clone(), guardian_review);
                                prepared_calls.push(prepared_call);
                                continue;
                            }
                            let mut action_payload = agent_turn_item_event_payload(
                                &AgentTurnItem::tool_call(call_id, tool_code.clone(), arguments),
                            );
                            if let Some(object) = action_payload.as_object_mut() {
                                object.insert("runtimeMode".to_owned(), json!("model_loop"));
                                object.insert(
                                    "concurrencyPolicy".to_owned(),
                                    concurrency_policy_payload,
                                );
                                object
                                    .insert("executorBinding".to_owned(), executor_binding_payload);
                                object.insert(
                                    "batchExecutionMode".to_owned(),
                                    batch_execution_mode_payload.clone(),
                                );
                                object.insert(
                                    "serialReason".to_owned(),
                                    json!(serial_reason.clone()),
                                );
                                object.insert(
                                    "toolCallBatch".to_owned(),
                                    Value::Array(tool_call_batch_payload.clone()),
                                );
                                object.insert("toolCallBatchIndex".to_owned(), json!(batch_index));
                                object.insert("toolCallBatchSize".to_owned(), json!(batch_size));
                                object.insert("guardianReview".to_owned(), guardian_review.clone());
                            }
                            self.append_event(
                                user_id,
                                run_id,
                                None,
                                RunEventKind::ActionSelected,
                                run_status_code(RunStatus::Running),
                                action_payload,
                            )
                            .await?;
                            ensure_agent_run_transition(
                                &run_status_code(RunStatus::Running),
                                RunStatus::WaitingApproval,
                            )?;
                            self.update_status(AgentStatusUpdate {
                                user_id,
                                run_id,
                                status: run_status_code(RunStatus::WaitingApproval),
                                output_payload: json!({ "toolCode": prepared_call.tool.code }),
                                final_output: None,
                                pause_reason: batch_policy.pause_reason.as_deref(),
                                finished: false,
                            })
                            .await?;
                            let guardian_review_override = Some(guardian_review);
                            let now = Utc::now().naive_utc();
                            self.pause_for_approval(
                                user_id,
                                run_id,
                                &prepared_call.tool,
                                &command.input,
                                Some(&runtime_state.items),
                                prepared_call.arguments.clone(),
                                command.auto_approve,
                                guardian_review_override,
                                now,
                            )
                            .await?;
                            self.refresh_trace_snapshot(
                                user_id,
                                run_id,
                                json!({ "runtimeMode": "model_loop", "pauseReason": "approval" }),
                            )
                            .await?;
                            return self.get_run(run_id).await;
                        }
                        prepared_calls.push(prepared_call);
                    }

                    let mut last_recorded_step_id = None;

                    for prepared_call in &prepared_calls {
                        runtime_state.push_item(AgentTurnItem::tool_call(
                            prepared_call.call_id.clone(),
                            prepared_call.tool.code.clone(),
                            prepared_call.arguments.clone(),
                        ));
                        let mut action_payload =
                            agent_turn_item_event_payload(&AgentTurnItem::tool_call(
                                prepared_call.call_id.clone(),
                                prepared_call.tool.code.clone(),
                                prepared_call.arguments.clone(),
                            ));
                        if let Some(object) = action_payload.as_object_mut() {
                            object.insert("runtimeMode".to_owned(), json!("model_loop"));
                            object.insert(
                                "concurrencyPolicy".to_owned(),
                                prepared_call.concurrency_policy.clone(),
                            );
                            object.insert(
                                "executorBinding".to_owned(),
                                model_loop_tool_executor_binding_payload(
                                    prepared_call.executor_binding.as_ref(),
                                ),
                            );
                            object.insert(
                                "batchExecutionMode".to_owned(),
                                batch_execution_mode_payload.clone(),
                            );
                            object.insert("serialReason".to_owned(), json!(serial_reason.clone()));
                            object.insert(
                                "toolCallBatch".to_owned(),
                                Value::Array(tool_call_batch_payload.clone()),
                            );
                            object.insert(
                                "toolCallBatchIndex".to_owned(),
                                json!(prepared_call.batch_index),
                            );
                            object.insert("toolCallBatchSize".to_owned(), json!(batch_size));
                            if let Some(guardian_review) =
                                guardian_auto_approved_calls.get(&prepared_call.call_id)
                            {
                                object.insert("guardianReview".to_owned(), guardian_review.clone());
                                object.insert("guardianAutoApproved".to_owned(), json!(true));
                                object.insert(
                                    "approvalMode".to_owned(),
                                    json!("guardian_auto_approved"),
                                );
                            }
                        }
                        self.append_event(
                            user_id,
                            run_id,
                            None,
                            RunEventKind::ActionSelected,
                            run_status_code(RunStatus::Running),
                            action_payload,
                        )
                        .await?;
                    }

                    if self
                        .check_model_loop_cancelled(user_id, run_id, "before_tool_batch")
                        .await?
                        == ModelLoopCancelCheck::Cancelled
                    {
                        return self.get_run(run_id).await;
                    }

                    let tool_io_service = self.clone();
                    let executed_calls = execute_agent_tool_io_batch(
                        batch_execution_mode,
                        prepared_calls,
                        cancel_token.clone(),
                        move |prepared| {
                            let tool_io_service = tool_io_service.clone();
                            async move {
                                tool_io_service
                                    .execute_agent_tool_io(user_id, prepared)
                                    .await
                            }
                        },
                    )
                    .await?;
                    for executed_call in executed_calls {
                        let prepared = executed_call.prepared.clone();
                        let executed_terminal_status = executed_call.terminal_status;
                        let recorded = self
                            .record_agent_tool_execution(
                                user_id,
                                run_id,
                                &prepared,
                                executed_call.execution,
                            )
                            .await?;
                        last_tool_terminal_status = executed_terminal_status;
                        last_recorded_step_id = Some(recorded.step_id);
                        let observation_status =
                            tool_observation_status_for_execution(&recorded.execution);
                        let observation_item = AgentTurnItem::tool_observation(
                            &prepared.call_id,
                            observation_status,
                            recorded.execution.response_payload.clone(),
                        );
                        runtime_state.push_item(observation_item.clone());
                        let mut observation_payload =
                            agent_turn_item_event_payload(&observation_item);
                        if let Some(object) = observation_payload.as_object_mut() {
                            object.insert("toolCode".to_owned(), json!(&prepared.tool.code));
                            object.insert("auditId".to_owned(), json!(recorded.audit_id));
                            object.insert("dryRun".to_owned(), json!(recorded.execution.dry_run));
                            object.insert("runtimeMode".to_owned(), json!("model_loop"));
                            if let Some(tool_io_metrics) = executed_call.tool_io_metrics.as_ref() {
                                object.insert(
                                    "toolIoTask".to_owned(),
                                    tool_io_metrics_payload(tool_io_metrics),
                                );
                            }
                        }
                        self.append_event(
                            user_id,
                            run_id,
                            Some(recorded.step_id),
                            RunEventKind::ToolCalled,
                            run_status_code(RunStatus::Running),
                            json!({
                                "toolCode": prepared.tool.code,
                                "arguments": prepared.arguments.clone(),
                                "auditId": recorded.audit_id,
                                "dryRun": recorded.execution.dry_run,
                                "runtimeMode": "model_loop"
                            }),
                        )
                        .await?;
                        self.append_event(
                            user_id,
                            run_id,
                            Some(recorded.step_id),
                            RunEventKind::Observation,
                            run_status_code(RunStatus::Running),
                            observation_payload,
                        )
                        .await?;
                    }

                    if self
                        .check_model_loop_cancelled(user_id, run_id, "after_tool_batch")
                        .await?
                        == ModelLoopCancelCheck::Cancelled
                    {
                        return self.get_run(run_id).await;
                    }

                    if runtime_state.should_compact_context() {
                        if let Some(deterministic_summary) =
                            runtime_state.compaction_candidate_summary()
                        {
                            let remote_compaction_request =
                                runtime_state.remote_compaction_request(tool_codes.clone());
                            let compaction_outcome = self
                                .model_loop_context_compaction_outcome(
                                    cancel_token.clone(),
                                    run_id,
                                    &command.input,
                                    &deterministic_summary,
                                    &tool_codes,
                                    remote_compaction_request.as_ref(),
                                    command.model_route_id.as_deref(),
                                )
                                .await?;
                            if compaction_outcome.cancelled {
                                if self
                                    .check_model_loop_cancelled(
                                        user_id,
                                        run_id,
                                        "context_compaction",
                                    )
                                    .await?
                                    == ModelLoopCancelCheck::Continue
                                {
                                    self.finish_model_loop_cancelled(
                                        user_id,
                                        run_id,
                                        &run_status_code(RunStatus::Cancelling),
                                        "context_compaction",
                                    )
                                    .await?;
                                }
                                return self.get_run(run_id).await;
                            }
                            let Some(compaction) = runtime_state
                                .compact_context_with_summary(compaction_outcome.summary.clone())
                            else {
                                continue;
                            };
                            let compaction_item = AgentTurnItem::ContextCompaction {
                                summary: compaction.summary.clone(),
                            };
                            let mut compaction_payload =
                                agent_turn_item_event_payload(&compaction_item);
                            if let Some(object) = compaction_payload.as_object_mut() {
                                object.insert("runtimeMode".to_owned(), json!("model_loop"));
                                object.insert(
                                    "compactionWindowId".to_owned(),
                                    json!(compaction.window_id),
                                );
                                object.insert(
                                    "retainedItemCount".to_owned(),
                                    json!(compaction.retained_item_count),
                                );
                                object.insert(
                                    "compactedItemCount".to_owned(),
                                    json!(compaction.compacted_item_count),
                                );
                                object.insert(
                                    "compactionStrategy".to_owned(),
                                    json!(&compaction_outcome.strategy),
                                );
                                object.insert(
                                    "compactionStatus".to_owned(),
                                    json!(&compaction_outcome.status),
                                );
                                object.insert(
                                    "compactionImplementation".to_owned(),
                                    json!("responses_compaction_v2"),
                                );
                                if let Some(remote_request) = &remote_compaction_request {
                                    let remote_payload =
                                        serde_json::to_value(remote_request).unwrap_or(Value::Null);
                                    object.insert("remoteCompaction".to_owned(), remote_payload);
                                    if let Some(model_request_metadata) =
                                        model_chat_request_metadata_for_remote_compaction(Some(
                                            remote_request,
                                        ))
                                    {
                                        object.insert(
                                            "modelRequestMetadata".to_owned(),
                                            serde_json::to_value(model_request_metadata)
                                                .unwrap_or(Value::Null),
                                        );
                                        object.insert(
                                            "compactionTransport".to_owned(),
                                            json!("provider_metadata_envelope"),
                                        );
                                    }
                                } else {
                                    object.insert(
                                        "compactionTransport".to_owned(),
                                        json!("prompt_adapter"),
                                    );
                                }
                                if let Some(model_payload) = &compaction_outcome.model_payload {
                                    object
                                        .insert("modelInference".to_owned(), model_payload.clone());
                                }
                                if let Some(error_payload) = &compaction_outcome.error_payload {
                                    object.insert("modelError".to_owned(), error_payload.clone());
                                }
                                if let Some(error_message) = &compaction_outcome.error_message {
                                    object.insert("errorMessage".to_owned(), json!(error_message));
                                }
                            }
                            self.append_event(
                                user_id,
                                run_id,
                                last_recorded_step_id,
                                RunEventKind::Observation,
                                run_status_code(RunStatus::Running),
                                compaction_payload,
                            )
                            .await?;
                            if self
                                .check_model_loop_cancelled(user_id, run_id, "before_next_turn")
                                .await?
                                == ModelLoopCancelCheck::Cancelled
                            {
                                return self.get_run(run_id).await;
                            }
                            continue;
                        }
                    }

                    if self
                        .check_model_loop_cancelled(user_id, run_id, "before_next_turn")
                        .await?
                        == ModelLoopCancelCheck::Cancelled
                    {
                        return self.get_run(run_id).await;
                    }

                    continue;
                }
                _ => {
                    let fallback_answer = model_response
                        .as_ref()
                        .map(|response| response.answer.clone())
                        .unwrap_or_else(|| parsed_payload.to_string());
                    runtime_state.push_item(parsed.item);
                    self.finish_model_loop_run(
                        user_id,
                        run_id,
                        None,
                        last_tool_terminal_status,
                        &fallback_answer,
                        json!({ "answer": fallback_answer.clone(), "runtimeMode": "model_loop" }),
                        parsed_payload,
                    )
                    .await?;
                    self.refresh_trace_snapshot(
                        user_id,
                        run_id,
                        json!({ "runtimeMode": "model_loop" }),
                    )
                    .await?;
                    return self.get_run(run_id).await;
                }
            }
        }

        let final_output = "Agent model loop stopped because the turn budget was exhausted.";
        self.finish_model_loop_run(
            user_id,
            run_id,
            None,
            RunStatus::Failed,
            final_output,
            json!({
                "answer": final_output,
                "runtimeMode": "model_loop",
                "stopReason": "turn_budget_exhausted"
            }),
            agent_turn_item_event_payload(&AgentTurnItem::FinalAnswer {
                content: final_output.to_owned(),
            }),
        )
        .await?;
        self.refresh_trace_snapshot(
            user_id,
            run_id,
            json!({
                "runtimeMode": "model_loop",
                "stopReason": "turn_budget_exhausted"
            }),
        )
        .await?;
        self.get_run(run_id).await
    }

    async fn try_cancel_streamed_provider_call(
        &self,
        user_id: i64,
        run_id: i64,
        provider_call_lease_id: Option<i64>,
        provider_response_id: Option<&str>,
    ) -> Result<(), AppError> {
        let Some(provider_call_lease_id) = provider_call_lease_id else {
            return Ok(());
        };
        let Some(provider_response_id) = provider_response_id else {
            return Ok(());
        };

        match self
            .model_runtime
            .cancel_provider_call_lease_with_response_metadata(
                user_id,
                provider_call_lease_id,
                Some(provider_response_id),
            )
            .await
        {
            Ok(cancel) => {
                self.append_event(
                    user_id,
                    run_id,
                    None,
                    RunEventKind::Thought,
                    run_status_code(RunStatus::Running),
                    provider_native_cancel_event_payload(&cancel),
                )
                .await?;
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    run_id,
                    provider_call_lease_id,
                    provider_response_id,
                    "failed to dispatch streamed provider native cancel"
                );
                self.append_event(
                    user_id,
                    run_id,
                    None,
                    RunEventKind::Thought,
                    run_status_code(RunStatus::Running),
                    provider_native_cancel_error_event_payload(
                        provider_call_lease_id,
                        Some(provider_response_id),
                        &err,
                    ),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn model_loop_context_compaction_outcome(
        &self,
        cancel_token: AgentRunCancellationToken,
        run_id: i64,
        original_input: &str,
        deterministic_summary: &str,
        tool_codes: &[String],
        remote_compaction_request: Option<&AgentRemoteCompactionRequest>,
        model_route_id: Option<&str>,
    ) -> Result<ModelLoopContextCompactionOutcome, AppError> {
        let messages = build_model_loop_remote_context_compaction_messages(
            original_input,
            deterministic_summary,
            tool_codes,
            remote_compaction_request,
        );
        let request_metadata =
            model_chat_request_metadata_for_remote_compaction(remote_compaction_request);
        let started = Instant::now();
        let result = await_model_loop_provider_future_or_cancelled(
            cancel_token,
            self.wait_for_model_loop_persistent_cancel(run_id),
            "context_compaction",
            self.model_runtime.chat_completion_for_purpose(
                ModelRoutePurpose::CodeAgent,
                ModelChatCommand {
                    route_id: model_route_id.map(str::to_owned),
                    messages,
                    temperature: Some(0.1),
                    max_tokens: Some(512),
                    request_metadata,
                    provider_call_context: Some(ModelProviderCallContext {
                        run_id: Some(run_id),
                        source: "agent.context_compaction".to_owned(),
                        route_purpose: Some(ModelRoutePurpose::CodeAgent),
                        attempt_kind: "primary".to_owned(),
                    }),
                    ..ModelChatCommand::default()
                },
            ),
        )
        .await;

        match result {
            Ok(ModelLoopFutureAwait::Completed(response)) => {
                let summary = model_loop_context_compaction_summary_from_response(&response.answer);
                Ok(ModelLoopContextCompactionOutcome {
                    summary: if summary.is_empty() {
                        deterministic_summary.to_owned()
                    } else {
                        summary
                    },
                    strategy: "model".to_owned(),
                    status: "succeeded".to_owned(),
                    cancelled: false,
                    model_payload: model_inference_event_payload(&response)
                        .get("item")
                        .cloned(),
                    error_payload: None,
                    error_message: None,
                })
            }
            Ok(ModelLoopFutureAwait::Cancelled) => Ok(ModelLoopContextCompactionOutcome {
                summary: deterministic_summary.to_owned(),
                strategy: "model".to_owned(),
                status: "cancelled".to_owned(),
                cancelled: true,
                model_payload: None,
                error_payload: None,
                error_message: Some("context compaction cancelled".to_owned()),
            }),
            Err(err) => Ok(ModelLoopContextCompactionOutcome {
                summary: deterministic_summary.to_owned(),
                strategy: "deterministic_fallback".to_owned(),
                status: "fallback_used".to_owned(),
                cancelled: false,
                model_payload: None,
                error_payload: model_inference_error_event_payload(
                    &err,
                    started.elapsed().as_millis(),
                )
                .get("item")
                .cloned(),
                error_message: Some(model_inference_error_message(&err)),
            }),
        }
    }

    pub async fn list_runs(
        &self,
        query: AgentRunQuery,
    ) -> Result<PageResult<AgentRunResp>, AppError> {
        let page = query.page_query();
        let filter = AgentRunFilter {
            tenant_id: self.tenant_id,
            status: query.status.as_deref(),
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_runs(&filter).await?;
        let list = self
            .repo
            .list_runs(&filter)
            .await?
            .into_iter()
            .map(AgentRunResp::from)
            .collect();
        Ok(PageResult::new(list, total))
    }

    pub async fn get_run(&self, run_id: i64) -> Result<AgentRunResp, AppError> {
        let Some(record) = self.repo.find_run(self.tenant_id, run_id).await? else {
            return Err(AppError::NotFound);
        };
        Ok(AgentRunResp::from(record))
    }

    pub async fn list_events(
        &self,
        run_id: i64,
        query: AgentRunEventQuery,
    ) -> Result<PageResult<AgentRunEventResp>, AppError> {
        let page = query.page_query();
        let filter = RunEventFilter {
            tenant_id: self.tenant_id,
            run_id,
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_events(&filter).await?;
        let list = self
            .repo
            .list_events(&filter)
            .await?
            .into_iter()
            .map(AgentRunEventResp::from)
            .collect();
        Ok(PageResult::new(list, total))
    }

    pub async fn list_events_after_sequence(
        &self,
        run_id: i64,
        after_sequence_no: i64,
        limit: i64,
    ) -> Result<Vec<AgentRunEventResp>, AppError> {
        let filter = RunEventCursorFilter {
            tenant_id: self.tenant_id,
            run_id,
            after_sequence_no: after_sequence_no.max(0),
            limit: limit.clamp(1, MAX_EVENT_STREAM_BATCH_SIZE as i64),
        };
        Ok(self
            .repo
            .list_events_after_sequence(&filter)
            .await?
            .into_iter()
            .map(AgentRunEventResp::from)
            .collect())
    }

    pub async fn is_run_terminal(&self, run_id: i64) -> Result<bool, AppError> {
        let Some(record) = self.repo.find_run(self.tenant_id, run_id).await? else {
            return Err(AppError::NotFound);
        };
        Ok(parse_run_status_code(&record.status).is_some_and(|status| status.is_terminal()))
    }

    pub async fn get_run_trace(&self, run_id: i64) -> Result<AgentTraceReplayResp, AppError> {
        let Some(run) = self.repo.find_run(self.tenant_id, run_id).await? else {
            return Err(AppError::NotFound);
        };
        let turn_items = self.load_model_loop_turn_item_history(run_id).await?;
        if let Some(rollout) = self
            .repo
            .find_rollout_by_run_id(self.tenant_id, run_id)
            .await?
        {
            if let Ok(bundle) = serde_json::from_value::<TraceBundle>(rollout.event_bundle) {
                return Ok(AgentTraceReplayResp::from_bundle_with_turn_items(
                    bundle, turn_items,
                ));
            }
        }
        let filter = RunEventFilter {
            tenant_id: self.tenant_id,
            run_id,
            limit: MAX_TRACE_REPLAY_EVENTS,
            offset: 0,
        };
        let events = self.repo.list_events(&filter).await?;

        Ok(AgentTraceReplayResp::from_bundle_with_turn_items(
            agent_events_to_trace_bundle(run.trace_id, events),
            turn_items,
        ))
    }

    pub async fn resume_run(
        &self,
        user_id: i64,
        run_id: i64,
        command: AgentRunResumeCommand,
    ) -> Result<AgentRunResp, AppError> {
        if !command.approved {
            return Err(AppError::bad_request("审批恢复必须显式通过"));
        }
        let run = self.get_run(run_id).await?;
        ensure_agent_run_transition(&run.status, RunStatus::Resuming)?;
        let Some(pause) = self.repo.find_active_pause(self.tenant_id, run_id).await? else {
            return Err(AppError::NotFound);
        };
        let now = Utc::now().naive_utc();
        self.repo
            .complete_pause(
                self.tenant_id,
                pause.id,
                "resumed",
                &json!({ "approved": true, "input": command.input }),
                user_id,
                now,
            )
            .await?;
        self.update_status(AgentStatusUpdate {
            user_id,
            run_id,
            status: run_status_code(RunStatus::Resuming),
            output_payload: Value::Null,
            final_output: None,
            pause_reason: None,
            finished: false,
        })
        .await?;
        self.append_event(
            user_id,
            run_id,
            pause.step_id,
            RunEventKind::Resumed,
            run_status_code(RunStatus::Resuming),
            json!({ "pauseReason": pause.pause_reason }),
        )
        .await?;
        let resume_queue_payload = agent_resume_queue_payload(&command);
        let requeued = self
            .repo
            .requeue_agent_run_for_resume_with_outbox(
                self.tenant_id,
                run_id,
                &resume_queue_payload,
                &AgentQueueOutboxSaveRecord {
                    id: next_id(),
                    tenant_id: self.tenant_id,
                    queue_id: 0,
                    run_id,
                    event_type: "agent.run.resumed".to_owned(),
                    max_attempts: DEFAULT_AGENT_QUEUE_MAX_ATTEMPTS,
                    payload: json!({
                        "source": "agent.resume_run",
                        "executionMode": "queued"
                    }),
                    status: 1,
                    attempt_count: 0,
                    user_id,
                    now,
                },
                user_id,
                now,
            )
            .await?;
        if requeued > 0 {
            self.append_event(
                user_id,
                run_id,
                None,
                RunEventKind::StatusChanged,
                run_status_code(RunStatus::Resuming),
                json!({
                    "status": run_status_code(RunStatus::Resuming),
                    "executionMode": "queued",
                    "resumeQueued": true
                }),
            )
            .await?;
            self.refresh_trace_snapshot(
                user_id,
                run_id,
                json!({
                    "resumed": true,
                    "executionMode": "queued",
                    "resumeQueued": true
                }),
            )
            .await?;
            return self.get_run(run_id).await;
        }
        ensure_agent_run_transition(&run_status_code(RunStatus::Resuming), RunStatus::Running)?;
        self.update_status(AgentStatusUpdate {
            user_id,
            run_id,
            status: run_status_code(RunStatus::Running),
            output_payload: Value::Null,
            final_output: None,
            pause_reason: None,
            finished: false,
        })
        .await?;
        self.append_event(
            user_id,
            run_id,
            None,
            RunEventKind::StatusChanged,
            run_status_code(RunStatus::Running),
            json!({ "status": run_status_code(RunStatus::Running) }),
        )
        .await?;
        self.execute_resumed_tool_and_finish(user_id, run_id, command.input)
            .await?;
        self.get_run(run_id).await
    }

    async fn execute_resumed_tool_and_finish(
        &self,
        user_id: i64,
        run_id: i64,
        resume_input: Value,
    ) -> Result<(), AppError> {
        let run = self.get_run(run_id).await?;
        let Some(tool_code) = run.selected_tool_code.as_deref() else {
            return Err(AppError::bad_request("恢复 Run 缺少工具上下文"));
        };
        let Some(tool) = self
            .capability_repo
            .find_tool_by_code(self.tenant_id, tool_code)
            .await?
        else {
            return Err(AppError::NotFound);
        };

        self.execute_tool_and_finish(user_id, run_id, &tool, resume_input)
            .await?;
        self.refresh_trace_snapshot(user_id, run_id, json!({ "resumed": true }))
            .await
    }

    pub async fn cancel_run(&self, user_id: i64, run_id: i64) -> Result<AgentRunResp, AppError> {
        let run = self.get_run(run_id).await?;
        ensure_agent_run_transition(&run.status, RunStatus::Cancelling)?;
        self.update_status(AgentStatusUpdate {
            user_id,
            run_id,
            status: run_status_code(RunStatus::Cancelling),
            output_payload: Value::Null,
            final_output: None,
            pause_reason: run.pause_reason.as_deref(),
            finished: false,
        })
        .await?;
        self.append_event(
            user_id,
            run_id,
            None,
            RunEventKind::CancelRequested,
            run_status_code(RunStatus::Cancelling),
            json!({ "requestedBy": user_id }),
        )
        .await?;
        let runtime_cancel_signal = self.agent_runtime.cancel_run_signal(self.tenant_id, run_id);
        let runtime_cancel_payload = runtime_cancelled_event_payload(runtime_cancel_signal);
        let now = Utc::now().naive_utc();
        self.repo
            .cancel_active_pauses(self.tenant_id, run_id, user_id, now)
            .await?;
        self.repo
            .cancel_agent_run_queue_for_run(self.tenant_id, run_id, user_id, now)
            .await?;
        ensure_agent_run_transition(
            &run_status_code(RunStatus::Cancelling),
            RunStatus::Cancelled,
        )?;
        self.update_status(AgentStatusUpdate {
            user_id,
            run_id,
            status: run_status_code(RunStatus::Cancelled),
            output_payload: json!({ "cancelled": true }),
            final_output: None,
            pause_reason: None,
            finished: true,
        })
        .await?;
        self.append_event(
            user_id,
            run_id,
            None,
            RunEventKind::Cancelled,
            run_status_code(RunStatus::Cancelled),
            runtime_cancel_payload,
        )
        .await?;
        self.refresh_trace_snapshot(user_id, run_id, json!({ "cancelled": true }))
            .await?;
        self.get_run(run_id).await
    }

    async fn check_model_loop_cancelled(
        &self,
        user_id: i64,
        run_id: i64,
        stage: &str,
    ) -> Result<ModelLoopCancelCheck, AppError> {
        let Some(run) = self.repo.find_run(self.tenant_id, run_id).await? else {
            return Err(AppError::NotFound);
        };
        if !model_loop_cancel_requested(&run.status) {
            return Ok(ModelLoopCancelCheck::Continue);
        }

        self.finish_model_loop_cancelled(user_id, run_id, &run.status, stage)
            .await?;
        Ok(ModelLoopCancelCheck::Cancelled)
    }

    async fn wait_for_model_loop_persistent_cancel(&self, run_id: i64) -> Result<(), AppError> {
        loop {
            tokio::time::sleep(MODEL_LOOP_PERSISTENT_CANCEL_POLL_INTERVAL).await;
            let Some(run) = self.repo.find_run(self.tenant_id, run_id).await? else {
                return Err(AppError::NotFound);
            };
            if model_loop_cancel_requested(&run.status) {
                return Ok(());
            }
        }
    }

    async fn finish_model_loop_cancelled(
        &self,
        user_id: i64,
        run_id: i64,
        current_status: &str,
        stage: &str,
    ) -> Result<(), AppError> {
        let payload = model_loop_external_cancel_payload(stage);
        if parse_run_status_code(current_status) == Some(RunStatus::Cancelling) {
            ensure_agent_run_transition(current_status, RunStatus::Cancelled)?;
            self.update_status(AgentStatusUpdate {
                user_id,
                run_id,
                status: run_status_code(RunStatus::Cancelled),
                output_payload: payload.clone(),
                final_output: None,
                pause_reason: None,
                finished: true,
            })
            .await?;
            self.append_event(
                user_id,
                run_id,
                None,
                RunEventKind::Cancelled,
                run_status_code(RunStatus::Cancelled),
                payload.clone(),
            )
            .await?;
        }
        self.refresh_trace_snapshot(user_id, run_id, payload)
            .await?;
        Ok(())
    }

    async fn create_run_records(
        &self,
        user_id: i64,
        run_id: i64,
        trace_id: &str,
        command: &AgentRunCommand,
        plan: &AgentPlanSummary,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        self.create_run_records_with_status(
            user_id,
            run_id,
            trace_id,
            command,
            plan,
            RunStatus::Running,
            now,
        )
        .await
    }

    async fn create_run_records_with_status(
        &self,
        user_id: i64,
        run_id: i64,
        trace_id: &str,
        command: &AgentRunCommand,
        plan: &AgentPlanSummary,
        initial_status: RunStatus,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        let status = run_status_code(initial_status);
        self.repo
            .create_run(&RunSaveRecord {
                id: run_id,
                tenant_id: self.tenant_id,
                run_type: "agent".to_owned(),
                status: status.clone(),
                source_type: "admin".to_owned(),
                source_id: Some(user_id.to_string()),
                trace_id: trace_id.to_owned(),
                input_payload: agent_run_command_payload(command),
                output_payload: Value::Null,
                budget_policy: serde_json::to_value(plan.task_budget).unwrap_or(Value::Null),
                created_by: user_id,
                started_at: (initial_status != RunStatus::Queued).then_some(now),
                user_id,
                now,
            })
            .await?;
        self.repo
            .create_agent_run(&AgentRunSaveRecord {
                id: next_id(),
                tenant_id: self.tenant_id,
                run_id,
                intent: plan.intent.clone(),
                loop_kind: plan.loop_kind.clone(),
                selected_tool_code: plan.selected_tool_code.clone(),
                status,
                pause_reason: None,
                task_budget: serde_json::to_value(plan.task_budget).unwrap_or(Value::Null),
                metadata: json!({
                    "milestone": "M3",
                    "poc": true,
                    "memorySnippetCount": plan.memory_context.snippets.len()
                }),
                user_id,
                now,
            })
            .await?;
        self.repo
            .create_agent_trace(&AgentTraceSaveRecord {
                id: next_id(),
                tenant_id: self.tenant_id,
                run_id,
                trace_id: trace_id.to_owned(),
                event_snapshot: json!([]),
                model_route_snapshot: json!({ "mode": "deterministic", "model": "none" }),
                tool_snapshot: json!({}),
                metadata: json!({
                    "milestone": "M3",
                    "memorySnippetCount": plan.memory_context.snippets.len()
                }),
                user_id,
                now,
            })
            .await
    }

    async fn pause_for_approval(
        &self,
        user_id: i64,
        run_id: i64,
        tool: &ToolLookupRecord,
        input: &str,
        runtime_items: Option<&[AgentTurnItem]>,
        tool_arguments: Value,
        auto_approved: bool,
        guardian_review_override: Option<Value>,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        let step_id = next_id();
        let guardian_review = match guardian_review_override {
            Some(review) => review,
            None => {
                self.guardian_review_payload_for_tool_policy(
                    input,
                    runtime_items,
                    tool,
                    tool_arguments,
                    auto_approved,
                )
                .await
            }
        };
        self.repo
            .create_step(&RunStepSaveRecord {
                id: step_id,
                tenant_id: self.tenant_id,
                run_id,
                parent_step_id: None,
                step_type: step_type_code(RunStepType::Approval),
                status: run_status_code(RunStatus::WaitingApproval),
                sequence_no: self
                    .repo
                    .next_event_sequence(self.tenant_id, run_id)
                    .await?,
                input_payload: json!({ "toolCode": tool.code, "input": input }),
                output_payload: Value::Null,
                tool_call_audit_id: None,
                user_id,
                now,
            })
            .await?;
        self.append_event(
            user_id,
            run_id,
            Some(step_id),
            RunEventKind::ActionSelected,
            run_status_code(RunStatus::Running),
            json!({
                "toolCode": tool.code,
                "riskLevel": tool.risk_level,
                "guardianReview": guardian_review.clone()
            }),
        )
        .await?;
        self.append_event(
            user_id,
            run_id,
            Some(step_id),
            RunEventKind::ApprovalRequested,
            run_status_code(RunStatus::WaitingApproval),
            json!({
                "toolCode": tool.code,
                "permissionCode": tool.permission_code,
                "guardianReview": guardian_review
            }),
        )
        .await?;
        self.repo
            .create_pause(&RunPauseSaveRecord {
                id: next_id(),
                tenant_id: self.tenant_id,
                run_id,
                step_id: Some(step_id),
                pause_reason: "approval".to_owned(),
                requested_input_schema: json!({
                    "type": "object",
                    "required": ["approved"],
                    "properties": { "approved": { "type": "boolean" } }
                }),
                resume_token_hash: None,
                user_id,
                now,
            })
            .await?;
        self.append_event(
            user_id,
            run_id,
            Some(step_id),
            RunEventKind::Paused,
            run_status_code(RunStatus::WaitingApproval),
            json!({ "pauseReason": "approval" }),
        )
        .await
    }

    async fn guardian_review_payload_for_tool_policy(
        &self,
        input: &str,
        runtime_items: Option<&[AgentTurnItem]>,
        tool: &ToolLookupRecord,
        arguments: Value,
        auto_approved: bool,
    ) -> Value {
        serde_json::to_value(
            self.guardian_review_decision_for_tool_policy(
                input,
                runtime_items,
                tool,
                arguments,
                auto_approved,
            )
            .await,
        )
        .unwrap_or(Value::Null)
    }

    async fn guardian_review_decision_for_tool_policy(
        &self,
        input: &str,
        runtime_items: Option<&[AgentTurnItem]>,
        tool: &ToolLookupRecord,
        arguments: Value,
        auto_approved: bool,
    ) -> GuardianReviewDecision {
        let review_input =
            guardian_review_input_for_tool_policy(tool, auto_approved, auto_approved);
        if !auto_approved {
            return review_tool_approval(review_input);
        }

        let request = guardian_model_review_request_for_tool(input, runtime_items, tool, arguments);
        let prompt_messages = match build_guardian_model_review_prompt(&request) {
            Ok(messages) => messages,
            Err(err) => {
                return guardian_review_failure_decision(
                    review_input,
                    GuardianReviewFailureReason::Parse,
                    format!("guardian review prompt build failed: {err}"),
                );
            }
        };
        let messages = prompt_messages
            .into_iter()
            .map(|message| ModelChatMessage {
                role: message.role,
                content: message.content,
            })
            .collect();
        let started = Instant::now();
        let result = tokio::time::timeout(
            GUARDIAN_REVIEW_TIMEOUT,
            self.model_runtime.chat_completion_for_purpose(
                ModelRoutePurpose::GuardianReview,
                ModelChatCommand {
                    messages,
                    temperature: Some(0.0),
                    max_tokens: Some(512),
                    ..ModelChatCommand::default()
                },
            ),
        )
        .await;

        match result {
            Err(_) => guardian_review_failure_decision(
                review_input,
                GuardianReviewFailureReason::Timeout,
                "guardian review timed out",
            ),
            Ok(Err(err)) => guardian_review_failure_decision(
                review_input,
                GuardianReviewFailureReason::Session,
                format!("guardian review model call failed: {err}"),
            ),
            Ok(Ok(response)) => match parse_guardian_model_assessment(&response.answer) {
                Ok(assessment) => guardian_review_decision_with_model_metadata(
                    review_tool_approval_with_model_assessment(review_input, assessment),
                    &response,
                    started.elapsed().as_millis(),
                ),
                Err(err) => guardian_review_decision_with_model_metadata(
                    guardian_review_failure_decision(
                        review_input,
                        err.kind,
                        format!("guardian review parse failed: {}", err.message),
                    ),
                    &response,
                    started.elapsed().as_millis(),
                ),
            },
        }
    }

    async fn record_retrieval_context(
        &self,
        user_id: i64,
        run_id: i64,
        input: &str,
        memory_context: &MemoryContext,
    ) -> Result<(), AppError> {
        let step_id = next_id();
        let output_payload = agent_context_retrieval_payload(input, memory_context);
        self.repo
            .create_step(&RunStepSaveRecord {
                id: step_id,
                tenant_id: self.tenant_id,
                run_id,
                parent_step_id: None,
                step_type: step_type_code(RunStepType::Retrieval),
                status: run_status_code(RunStatus::Succeeded),
                sequence_no: self
                    .repo
                    .next_event_sequence(self.tenant_id, run_id)
                    .await?,
                input_payload: json!({ "query": input }),
                output_payload: output_payload.clone(),
                tool_call_audit_id: None,
                user_id,
                now: Utc::now().naive_utc(),
            })
            .await?;
        self.append_event(
            user_id,
            run_id,
            Some(step_id),
            RunEventKind::Retrieval,
            run_status_code(RunStatus::Succeeded),
            output_payload,
        )
        .await
    }

    async fn load_agent_memory_context(&self, user_id: i64) -> Result<MemoryContext, AppError> {
        let records = self
            .memory_repo
            .list_memories(&MemoryFilter {
                tenant_id: self.tenant_id,
                scope_type: None,
                scope_id: None,
                limit: MAX_AGENT_MEMORY_CANDIDATES,
                offset: 0,
            })
            .await?;

        Ok(agent_memory_context_from_records(
            self.tenant_id,
            user_id,
            records,
        ))
    }

    async fn execute_and_record_tool_call(
        &self,
        user_id: i64,
        run_id: i64,
        tool: &ToolLookupRecord,
        input: Value,
    ) -> Result<RecordedToolExecution, AppError> {
        let prepared = PreparedAgentToolCall {
            batch_index: 0,
            call_id: "single-tool-call".to_owned(),
            tool: tool.clone(),
            arguments: input,
            executor_binding: None,
            concurrency_policy: Value::Null,
            timeout: AGENT_TOOL_IO_TIMEOUT,
        };
        let executed = self.execute_agent_tool_io(user_id, prepared).await?;
        self.record_agent_tool_execution(user_id, run_id, &executed.prepared, executed.execution)
            .await
    }

    async fn execute_agent_tool_io(
        &self,
        user_id: i64,
        prepared: PreparedAgentToolCall,
    ) -> Result<ExecutedAgentToolCall, AppError> {
        let tool = &prepared.tool;
        let executor_dispatch = prepared
            .executor_binding
            .as_ref()
            .map(ToolExecutorDispatchPlan::from_binding);
        let tool_kind = agent_tool_kind(tool);
        let connector_credential = if agent_tool_requires_github_connector_credential(
            &tool.code,
            executor_dispatch.as_ref(),
        ) {
            self.capability_repo
                .find_connector_credential(self.tenant_id, GITHUB_CONNECTOR_CODE, user_id)
                .await?
        } else {
            None
        };
        let mcp_tool = if agent_tool_requires_mcp_lookup(tool_kind, executor_dispatch.as_ref()) {
            self.capability_repo
                .find_mcp_tool_for_execution(self.tenant_id, &tool.code)
                .await?
        } else {
            None
        };
        let execution = execute_agent_tool(
            tool,
            &prepared.arguments,
            connector_credential.as_ref(),
            mcp_tool.as_ref(),
            executor_dispatch.as_ref(),
            Some(&self.model_runtime),
        )
        .await;
        let terminal_status = if execution.cancelled_status() {
            RunStatus::Cancelled
        } else if execution.succeeded_status() {
            RunStatus::Succeeded
        } else {
            RunStatus::Failed
        };
        Ok(ExecutedAgentToolCall {
            prepared,
            execution,
            terminal_status,
            tool_io_metrics: None,
        })
    }

    async fn record_agent_tool_execution(
        &self,
        user_id: i64,
        run_id: i64,
        prepared: &PreparedAgentToolCall,
        execution: AgentToolExecution,
    ) -> Result<RecordedToolExecution, AppError> {
        let now = Utc::now().naive_utc();
        let audit_id = next_id();
        let tool = &prepared.tool;
        let input = prepared.arguments.clone();
        let terminal_status = if execution.succeeded_status() {
            RunStatus::Succeeded
        } else {
            RunStatus::Failed
        };
        let step_status = run_status_code(terminal_status);
        self.capability_repo
            .create_tool_call_audit(&ToolAuditSaveRecord {
                id: audit_id,
                tenant_id: self.tenant_id,
                tool_id: tool.id,
                tool_code: tool.code.clone(),
                caller_kind: "agent_run".to_owned(),
                caller_id: Some(run_id),
                request_payload: json!({
                    "runId": run_id,
                    "toolCode": tool.code,
                    "executorBinding": model_loop_tool_executor_binding_payload(
                        prepared.executor_binding.as_ref()
                    ),
                    "input": input.clone()
                }),
                response_payload: execution.response_payload.clone(),
                status: execution.status.clone(),
                dry_run: execution.dry_run,
                risk_level: tool.risk_level,
                permission_code: tool.permission_code.clone(),
                error_message: execution.error_message.clone(),
                user_id,
                now,
            })
            .await?;
        if tool.code == MEDIA_IMAGE_TOOL_CODE {
            self.record_media_tool_result(user_id, run_id, audit_id, &execution, now)
                .await?;
        }
        let step_id = next_id();
        self.repo
            .create_step(&RunStepSaveRecord {
                id: step_id,
                tenant_id: self.tenant_id,
                run_id,
                parent_step_id: None,
                step_type: step_type_code(RunStepType::ToolCall),
                status: step_status.clone(),
                sequence_no: self
                    .repo
                    .next_event_sequence(self.tenant_id, run_id)
                    .await?,
                input_payload: input.clone(),
                output_payload: execution.response_payload.clone(),
                tool_call_audit_id: Some(audit_id),
                user_id,
                now,
            })
            .await?;
        Ok(RecordedToolExecution {
            audit_id,
            step_id,
            execution,
            terminal_status,
        })
    }

    async fn execute_tool_and_finish(
        &self,
        user_id: i64,
        run_id: i64,
        tool: &ToolLookupRecord,
        input: Value,
    ) -> Result<(), AppError> {
        let recorded = self
            .execute_and_record_tool_call(user_id, run_id, tool, input)
            .await?;
        ensure_agent_run_transition(
            &run_status_code(RunStatus::Running),
            recorded.terminal_status,
        )?;
        let step_status = run_status_code(recorded.terminal_status);
        let final_output = recorded.execution.final_output.clone();
        self.append_event(
            user_id,
            run_id,
            Some(recorded.step_id),
            RunEventKind::ToolCalled,
            run_status_code(RunStatus::Running),
            json!({ "toolCode": tool.code, "auditId": recorded.audit_id }),
        )
        .await?;
        self.append_event(
            user_id,
            run_id,
            Some(recorded.step_id),
            RunEventKind::Observation,
            run_status_code(RunStatus::Running),
            recorded.execution.response_payload.clone(),
        )
        .await?;
        self.update_status(AgentStatusUpdate {
            user_id,
            run_id,
            status: step_status.clone(),
            output_payload: json!({ "answer": final_output.clone(), "auditId": recorded.audit_id }),
            final_output: Some(&final_output),
            pause_reason: None,
            finished: true,
        })
        .await?;
        self.append_event(
            user_id,
            run_id,
            Some(recorded.step_id),
            RunEventKind::StatusChanged,
            step_status.clone(),
            json!({ "status": step_status, "toolCode": tool.code }),
        )
        .await?;
        self.append_event(
            user_id,
            run_id,
            Some(recorded.step_id),
            RunEventKind::FinalOutput,
            step_status,
            json!({ "answer": final_output }),
        )
        .await?;
        self.refresh_trace_snapshot(
            user_id,
            run_id,
            json!({ "toolCode": tool.code, "auditId": recorded.audit_id }),
        )
        .await
    }

    async fn record_media_tool_result(
        &self,
        user_id: i64,
        run_id: i64,
        audit_id: i64,
        execution: &AgentToolExecution,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        let Some(records) = media_records_from_tool_execution(
            self.tenant_id,
            run_id,
            user_id,
            audit_id,
            execution,
            now,
        ) else {
            return Ok(());
        };
        self.media_repo
            .create_media_result(records.asset.as_ref(), &records.job)
            .await
    }

    async fn finish_model_loop_run(
        &self,
        user_id: i64,
        run_id: i64,
        step_id: Option<i64>,
        final_status: RunStatus,
        final_output: &str,
        output_payload: Value,
        final_payload: Value,
    ) -> Result<(), AppError> {
        ensure_agent_run_transition(&run_status_code(RunStatus::Running), final_status)?;
        let final_status_code = run_status_code(final_status);
        self.update_status(AgentStatusUpdate {
            user_id,
            run_id,
            status: final_status_code.clone(),
            output_payload,
            final_output: Some(final_output),
            pause_reason: None,
            finished: true,
        })
        .await?;
        self.append_event(
            user_id,
            run_id,
            step_id,
            RunEventKind::StatusChanged,
            final_status_code.clone(),
            json!({
                "status": final_status_code.clone(),
                "runtimeMode": "model_loop"
            }),
        )
        .await?;
        self.append_event(
            user_id,
            run_id,
            step_id,
            RunEventKind::FinalOutput,
            final_status_code,
            final_payload,
        )
        .await
    }

    async fn finish_without_tool(
        &self,
        user_id: i64,
        run_id: i64,
        final_output: &str,
    ) -> Result<(), AppError> {
        self.update_status(AgentStatusUpdate {
            user_id,
            run_id,
            status: run_status_code(RunStatus::Succeeded),
            output_payload: json!({ "answer": final_output }),
            final_output: Some(final_output),
            pause_reason: None,
            finished: true,
        })
        .await?;
        self.append_event(
            user_id,
            run_id,
            None,
            RunEventKind::StatusChanged,
            run_status_code(RunStatus::Succeeded),
            json!({ "status": run_status_code(RunStatus::Succeeded) }),
        )
        .await?;
        self.append_event(
            user_id,
            run_id,
            None,
            RunEventKind::FinalOutput,
            run_status_code(RunStatus::Succeeded),
            json!({ "answer": final_output }),
        )
        .await
    }

    async fn update_status(&self, update: AgentStatusUpdate<'_>) -> Result<(), AppError> {
        let now = Utc::now().naive_utc();
        self.repo
            .update_run_status(&RunStatusUpdate {
                tenant_id: self.tenant_id,
                run_id: update.run_id,
                status: &update.status,
                output_payload: &update.output_payload,
                finished: update.finished,
                user_id: update.user_id,
                now,
            })
            .await?;
        self.repo
            .update_agent_run_status(&AgentRunStatusUpdate {
                tenant_id: self.tenant_id,
                run_id: update.run_id,
                status: &update.status,
                final_output: update.final_output,
                pause_reason: update.pause_reason,
                user_id: update.user_id,
                now,
            })
            .await
    }

    async fn load_model_loop_turn_item_history(
        &self,
        run_id: i64,
    ) -> Result<Vec<AgentTurnItem>, AppError> {
        let records = self
            .repo
            .list_turn_items(&AgentTurnItemFilter {
                tenant_id: self.tenant_id,
                run_id,
                limit: MAX_TRACE_REPLAY_EVENTS,
                offset: 0,
            })
            .await?;
        records
            .into_iter()
            .map(agent_turn_item_from_record)
            .collect()
    }

    async fn append_event(
        &self,
        user_id: i64,
        run_id: i64,
        step_id: Option<i64>,
        event_type: RunEventKind,
        status: String,
        payload: Value,
    ) -> Result<(), AppError> {
        let sequence_no = self
            .repo
            .next_event_sequence(self.tenant_id, run_id)
            .await?;
        let event_id = next_id();
        let now = Utc::now().naive_utc();
        let turn_item_record = agent_turn_item_save_record_from_event_payload(
            self.tenant_id,
            run_id,
            step_id,
            event_id,
            sequence_no,
            &payload,
            user_id,
            now,
        );
        let event_record = RunEventSaveRecord {
            id: event_id,
            tenant_id: self.tenant_id,
            run_id,
            step_id,
            event_type: event_kind_code(event_type),
            sequence_no,
            status,
            payload,
            user_id,
            now,
        };
        self.repo
            .create_event_with_turn_item(&event_record, turn_item_record.as_ref())
            .await
    }

    async fn refresh_trace_snapshot(
        &self,
        user_id: i64,
        run_id: i64,
        tool_snapshot: Value,
    ) -> Result<(), AppError> {
        let filter = RunEventFilter {
            tenant_id: self.tenant_id,
            run_id,
            limit: DEFAULT_EVENT_PAGE_SIZE as i64,
            offset: 0,
        };
        let events = self.repo.list_events(&filter).await?;
        let trace_id = format!("agent-{run_id}");
        let bundle = agent_events_to_trace_bundle(&trace_id, events.clone());
        let event_snapshot = agent_trace_snapshot_payload_for_bundle(&events, &bundle);
        let summary = bundle.replay_summary();
        let now = Utc::now().naive_utc();
        self.repo
            .update_trace_snapshot(
                self.tenant_id,
                run_id,
                &event_snapshot,
                &tool_snapshot,
                user_id,
                now,
            )
            .await?;
        self.repo
            .upsert_rollout_bundle(&AgentRolloutSaveRecord {
                id: next_id(),
                tenant_id: self.tenant_id,
                run_id,
                trace_id,
                event_bundle: serde_json::to_value(&bundle).unwrap_or_else(|_| json!({})),
                summary_payload: serde_json::to_value(&summary).unwrap_or_else(|_| json!({})),
                source: "agent_run".to_owned(),
                user_id,
                now,
            })
            .await
    }
}

#[allow(dead_code)]
async fn await_model_loop_future_or_cancelled<F, T>(
    cancel_token: AgentRunCancellationToken,
    _stage: &str,
    future: F,
) -> Result<ModelLoopFutureAwait<T>, AppError>
where
    F: Future<Output = Result<T, AppError>>,
{
    tokio::select! {
        biased;
        _ = cancel_token.cancelled() => Ok(ModelLoopFutureAwait::Cancelled),
        result = future => result.map(ModelLoopFutureAwait::Completed),
    }
}

async fn await_model_loop_provider_future_or_cancelled<F, C, T>(
    cancel_token: AgentRunCancellationToken,
    persistent_cancel: C,
    _stage: &str,
    future: F,
) -> Result<ModelLoopFutureAwait<T>, AppError>
where
    F: Future<Output = Result<T, AppError>>,
    C: Future<Output = Result<(), AppError>>,
{
    tokio::select! {
        biased;
        _ = cancel_token.cancelled() => Ok(ModelLoopFutureAwait::Cancelled),
        persistent = persistent_cancel => {
            persistent?;
            Ok(ModelLoopFutureAwait::Cancelled)
        }
        result = future => result.map(ModelLoopFutureAwait::Completed),
    }
}

#[derive(Debug, Clone)]
struct ModelLoopProviderStreamState {
    tool_call_parser: StreamingModelTurnParser,
    tool_call_detected: bool,
    tool_call_parser_disabled: bool,
    detected_tool_call_output: Option<ParsedModelTurnOutput>,
    provider_call_lease_id: Option<i64>,
    provider_response_id: Option<String>,
    provider_response_status: Option<String>,
}

impl ModelLoopProviderStreamState {
    fn new() -> Self {
        Self {
            tool_call_parser: StreamingModelTurnParser::new(),
            tool_call_detected: false,
            tool_call_parser_disabled: false,
            detected_tool_call_output: None,
            provider_call_lease_id: None,
            provider_response_id: None,
            provider_response_status: None,
        }
    }

    fn observe_tool_call(&mut self, event: &ModelProviderStreamEvent) -> Option<Value> {
        self.observe_provider_response_metadata(event);

        if self.tool_call_detected || self.tool_call_parser_disabled {
            return None;
        }

        match self.tool_call_parser.push_delta(&event.chunk.content) {
            Ok(StreamingModelTurnParseStatus::Pending) => None,
            Ok(StreamingModelTurnParseStatus::Ready(parsed)) => {
                self.tool_call_detected = true;
                self.detected_tool_call_output = Some(parsed.clone());
                Some(model_stream_tool_call_event_payload(event, &parsed))
            }
            Err(_) => {
                self.tool_call_parser_disabled = true;
                None
            }
        }
    }

    fn detected_tool_call_output(&self) -> Option<ParsedModelTurnOutput> {
        self.detected_tool_call_output.clone()
    }

    fn observe_provider_response_metadata(&mut self, event: &ModelProviderStreamEvent) {
        if let Some(provider_call_lease_id) = event.provider_call_lease_id {
            self.provider_call_lease_id = Some(provider_call_lease_id);
        }
        if let Some(provider_response_id) = event.provider_response_id.as_ref() {
            self.provider_response_id = Some(provider_response_id.clone());
        }
        if let Some(provider_response_status) = event.provider_response_status.as_ref() {
            self.provider_response_status = Some(provider_response_status.clone());
        }
    }

    fn provider_response_id(&self) -> Option<String> {
        self.provider_response_id.clone()
    }

    fn provider_call_lease_id(&self) -> Option<i64> {
        self.provider_call_lease_id
    }

    fn provider_response_status(&self) -> Option<String> {
        self.provider_response_status.clone()
    }
}

fn model_loop_streamed_tool_call_completion<T>(
    stream_state: &ModelLoopProviderStreamState,
) -> Option<ModelLoopProviderCompletion<T>> {
    stream_state
        .detected_tool_call_output()
        .map(|streamed_tool_call_output| ModelLoopProviderCompletion {
            response: None,
            streamed_tool_call_output: Some(streamed_tool_call_output),
            completion_reason: ModelLoopProviderCompletionReason::StreamedToolCallDetected,
            provider_call_lease_id: stream_state.provider_call_lease_id(),
            provider_response_id: stream_state.provider_response_id(),
            provider_response_status: stream_state.provider_response_status(),
        })
}

async fn await_model_loop_stream_call_or_cancelled_with_delta_events<C>(
    cancel_token: AgentRunCancellationToken,
    persistent_cancel: C,
    _stage: &str,
    service: &AgentService,
    user_id: i64,
    run_id: i64,
    model_stream_call: ModelChatStreamCall,
) -> Result<ModelLoopFutureAwait<ModelLoopProviderCompletion<ModelChatResp>>, AppError>
where
    C: Future<Output = Result<(), AppError>>,
{
    let ModelChatStreamCall {
        lifecycle: _lifecycle,
        transport,
        events: mut provider_stream_receiver,
    } = model_stream_call;
    let future = transport.wait();
    tokio::pin!(future);
    tokio::pin!(persistent_cancel);
    let mut stream_closed = false;
    let mut stream_state = ModelLoopProviderStreamState::new();

    loop {
        tokio::select! {
            biased;
            _ = cancel_token.clone().cancelled() => return Ok(ModelLoopFutureAwait::Cancelled),
            persistent = &mut persistent_cancel => {
                persistent?;
                return Ok(ModelLoopFutureAwait::Cancelled);
            }
            maybe_event = provider_stream_receiver.recv(), if !stream_closed => {
                if let Some(event) = maybe_event {
                    drain_model_delta_events(service, user_id, run_id, &mut stream_state, event).await?;
                    if let Some(completion) = model_loop_streamed_tool_call_completion::<ModelChatResp>(&stream_state) {
                        return Ok(ModelLoopFutureAwait::Completed(completion));
                    }
                } else {
                    stream_closed = true;
                }
            }
            result = &mut future => {
                while let Ok(event) = provider_stream_receiver.try_recv() {
                    drain_model_delta_events(service, user_id, run_id, &mut stream_state, event).await?;
                }
                return result.map(|response| {
                    ModelLoopFutureAwait::Completed(ModelLoopProviderCompletion {
                        response: Some(response),
                        streamed_tool_call_output: stream_state.detected_tool_call_output(),
                        completion_reason: ModelLoopProviderCompletionReason::ProviderCompleted,
                        provider_call_lease_id: stream_state.provider_call_lease_id(),
                        provider_response_id: stream_state.provider_response_id(),
                        provider_response_status: stream_state.provider_response_status(),
                    })
                });
            }
        }
    }
}

async fn drain_model_delta_events(
    service: &AgentService,
    user_id: i64,
    run_id: i64,
    stream_state: &mut ModelLoopProviderStreamState,
    event: ModelProviderStreamEvent,
) -> Result<(), AppError> {
    let tool_call_payload = stream_state.observe_tool_call(&event);
    service
        .append_event(
            user_id,
            run_id,
            None,
            RunEventKind::Thought,
            run_status_code(RunStatus::Running),
            model_delta_event_payload_from_stream_event(&event),
        )
        .await?;
    if let Some(tool_call_payload) = tool_call_payload {
        service
            .append_event(
                user_id,
                run_id,
                None,
                RunEventKind::Thought,
                run_status_code(RunStatus::Running),
                tool_call_payload,
            )
            .await?;
    }
    Ok(())
}

fn model_loop_parse_turn_output(
    response: Option<&ModelChatResp>,
    streamed_tool_call_output: Option<&ParsedModelTurnOutput>,
) -> Result<ParsedModelTurnOutput, AppError> {
    if let Some(streamed_tool_call_output) = streamed_tool_call_output {
        return Ok(streamed_tool_call_output.clone());
    }

    let Some(response) = response else {
        return Err(AppError::bad_request(
            "Agent 模型输出解析失败: provider response missing",
        ));
    };

    parse_model_turn_output(&response.answer)
        .map_err(|err| AppError::bad_request(format!("Agent 模型输出解析失败: {}", err.message)))
}

fn media_records_from_tool_execution(
    tenant_id: i64,
    run_id: i64,
    user_id: i64,
    audit_id: i64,
    execution: &AgentToolExecution,
    now: NaiveDateTime,
) -> Option<MediaPersistenceRecords> {
    let tool_code = execution
        .response_payload
        .get("toolCode")
        .and_then(Value::as_str)
        .unwrap_or(MEDIA_IMAGE_TOOL_CODE);
    if tool_code != MEDIA_IMAGE_TOOL_CODE {
        return None;
    }

    let provider = non_empty_json_string(execution.response_payload.get("provider"))
        .unwrap_or_else(|| "right-code-draw".to_owned());
    let request_payload = execution
        .response_payload
        .get("requestPayload")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let prompt = non_empty_json_string(request_payload.get("prompt"))
        .or_else(|| non_empty_json_string(execution.response_payload.get("prompt")))
        .unwrap_or_else(|| "Novex generated image".to_owned());
    let asset_url = non_empty_json_string(execution.response_payload.get("assetUrl"));
    let provider_asset_id =
        non_empty_json_string(execution.response_payload.get("providerAssetId"));
    let model_route = non_empty_json_string(execution.response_payload.get("modelRoute"));

    let asset = asset_url.map(|asset_url| {
        let id = next_id();
        MediaAssetSaveRecord {
            id,
            tenant_id,
            asset_uid: format!("media-{id}"),
            asset_kind: "image".to_owned(),
            provider: provider.clone(),
            provider_asset_id,
            asset_url: Some(asset_url),
            storage_ref: None,
            mime_type: None,
            width: None,
            height: None,
            metadata: json!({
                "toolCode": MEDIA_IMAGE_TOOL_CODE,
                "runId": run_id,
                "auditId": audit_id,
            }),
            user_id,
            now,
        }
    });

    let job = MediaJobSaveRecord {
        id: next_id(),
        tenant_id,
        trace_id: Some(format!("agent-{run_id}")),
        run_id: Some(run_id),
        tool_call_audit_id: Some(audit_id),
        tool_code: MEDIA_IMAGE_TOOL_CODE.to_owned(),
        provider,
        model_route: (!execution.dry_run).then_some(model_route).flatten(),
        prompt,
        request_payload,
        response_payload: execution.response_payload.clone(),
        asset_id: asset.as_ref().map(|asset| asset.id),
        status: execution.status.clone(),
        dry_run: execution.dry_run,
        cost: None,
        latency_ms: None,
        policy_result: json!({
            "riskLevel": "medium",
            "approval": "required_before_external_call",
        }),
        error_message: execution.error_message.clone(),
        user_id,
        now,
    };

    Some(MediaPersistenceRecords { asset, job })
}

fn non_empty_json_string(value: Option<&Value>) -> Option<String> {
    value?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn agent_tool_policy_decision(
    tool: &ToolLookupRecord,
    auto_approved: bool,
) -> ToolExecutionPolicyDecision {
    evaluate_tool_execution_policy(ToolExecutionPolicyInput {
        tool_code: tool.code.clone(),
        risk_level: tool_risk_level(tool.risk_level),
        approval_policy: tool_approval_policy(tool.approval_policy),
        permission_code: tool.permission_code.clone(),
        auto_approved,
    })
}

#[allow(dead_code)]
fn guardian_review_for_tool_policy(
    tool: &ToolLookupRecord,
    auto_approved: bool,
) -> GuardianReviewDecision {
    review_tool_approval(guardian_review_input_for_tool_policy(
        tool,
        auto_approved,
        false,
    ))
}

fn guardian_review_input_for_tool_policy(
    tool: &ToolLookupRecord,
    auto_approved: bool,
    reviewer_enabled: bool,
) -> GuardianReviewInput {
    GuardianReviewInput {
        tool_code: tool.code.clone(),
        risk_level: guardian_risk_level(tool.risk_level),
        approval_policy: guardian_approval_policy(tool.approval_policy),
        user_authorization: if auto_approved {
            GuardianUserAuthorization::Implicit
        } else {
            GuardianUserAuthorization::Missing
        },
        auto_approved,
        reviewer_enabled,
    }
}

fn guardian_model_review_request_for_tool(
    input: &str,
    runtime_items: Option<&[AgentTurnItem]>,
    tool: &ToolLookupRecord,
    arguments: Value,
) -> GuardianModelReviewRequest {
    GuardianModelReviewRequest {
        transcript: guardian_transcript_entries(input, runtime_items),
        reviewed_action: GuardianReviewedAction {
            tool_code: tool.code.clone(),
            arguments,
            permission_code: tool.permission_code.clone(),
        },
        retry_reason: None,
    }
}

fn guardian_auto_approval_allows_execution(decision: &GuardianReviewDecision) -> bool {
    matches!(decision.source, GuardianDecisionSource::Guardian)
        && matches!(decision.review_status, GuardianReviewStatus::Reviewed)
        && decision.can_execute
}

fn guardian_review_payload_from_decision(decision: &GuardianReviewDecision) -> Value {
    serde_json::to_value(decision).unwrap_or(Value::Null)
}

fn guardian_transcript_entries(
    input: &str,
    runtime_items: Option<&[AgentTurnItem]>,
) -> Vec<GuardianTranscriptEntry> {
    let mut entries = runtime_items
        .unwrap_or_default()
        .iter()
        .filter_map(guardian_transcript_entry_from_turn_item)
        .collect::<Vec<_>>();
    if entries.is_empty() {
        entries.push(GuardianTranscriptEntry {
            role: GuardianTranscriptRole::User,
            content: input.to_owned(),
        });
    }
    entries
}

fn guardian_transcript_entry_from_turn_item(
    item: &AgentTurnItem,
) -> Option<GuardianTranscriptEntry> {
    match item {
        AgentTurnItem::UserMessage { content } => Some(GuardianTranscriptEntry {
            role: GuardianTranscriptRole::User,
            content: content.clone(),
        }),
        AgentTurnItem::AssistantMessage { content } | AgentTurnItem::FinalAnswer { content } => {
            Some(GuardianTranscriptEntry {
                role: GuardianTranscriptRole::Assistant,
                content: content.clone(),
            })
        }
        AgentTurnItem::Reasoning { summary } | AgentTurnItem::ContextCompaction { summary } => {
            Some(GuardianTranscriptEntry {
                role: GuardianTranscriptRole::Assistant,
                content: summary.clone(),
            })
        }
        AgentTurnItem::ToolCall {
            tool_code,
            arguments,
            ..
        } => Some(GuardianTranscriptEntry {
            role: GuardianTranscriptRole::Tool,
            content: format!("tool_call {tool_code} arguments: {arguments}"),
        }),
        AgentTurnItem::ToolObservation { status, output, .. } => Some(GuardianTranscriptEntry {
            role: GuardianTranscriptRole::Tool,
            content: format!("tool_observation status: {status:?} output: {output}"),
        }),
    }
}

fn guardian_review_decision_with_model_metadata(
    mut decision: GuardianReviewDecision,
    response: &ModelChatResp,
    latency_ms: u128,
) -> GuardianReviewDecision {
    decision.model_route_id = Some(response.route_id.clone());
    decision.model_provider = Some(response.provider.clone());
    decision.model_name = response.model.clone();
    decision.review_latency_ms = Some(latency_ms);
    decision
}

fn tool_risk_level(value: i16) -> ToolRiskLevel {
    match value {
        value if value <= 1 => ToolRiskLevel::Low,
        2 => ToolRiskLevel::Medium,
        _ => ToolRiskLevel::High,
    }
}

fn tool_approval_policy(value: i16) -> ApprovalPolicy {
    match value {
        0 => ApprovalPolicy::Never,
        3 => ApprovalPolicy::Always,
        _ => ApprovalPolicy::OnRisk,
    }
}

fn guardian_risk_level(value: i16) -> GuardianRiskLevel {
    match value {
        value if value <= 1 => GuardianRiskLevel::Low,
        2 => GuardianRiskLevel::Medium,
        _ => GuardianRiskLevel::High,
    }
}

fn guardian_approval_policy(value: i16) -> GuardianApprovalPolicy {
    match value {
        0 => GuardianApprovalPolicy::Never,
        3 => GuardianApprovalPolicy::Always,
        _ => GuardianApprovalPolicy::OnRisk,
    }
}

pub fn normalize_agent_run_command(
    mut command: AgentRunCommand,
) -> Result<AgentRunCommand, AppError> {
    command.input = command.input.trim().to_owned();
    if command.input.is_empty() {
        return Err(AppError::bad_request("Agent 输入不能为空"));
    }
    ensure_max_chars("Agent 输入", &command.input, 4000)?;
    command.runtime_mode = normalize_agent_runtime_mode(command.runtime_mode)?;
    command.execution_mode = Some(normalize_agent_execution_mode(command.execution_mode)?);
    command.model_route_id = normalize_optional_agent_model_route_id(command.model_route_id)?;
    command.budget = novex_ai_core::normalize_task_budget(command.budget)
        .map_err(|err| AppError::bad_request(format!("任务预算超出限制: {}", err.field)))?;
    command.workbench_context = normalize_agent_workbench_context(command.workbench_context);
    Ok(command)
}

fn normalize_optional_agent_model_route_id(
    route_id: Option<String>,
) -> Result<Option<String>, AppError> {
    let route_id = route_id
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(route_id) = &route_id {
        ensure_max_chars("模型路由", route_id, 128)?;
    }
    Ok(route_id)
}

fn normalize_agent_runtime_mode(runtime_mode: Option<String>) -> Result<Option<String>, AppError> {
    let Some(runtime_mode) = runtime_mode
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    if runtime_mode == "model_loop" {
        return Ok(Some(runtime_mode));
    }
    Err(AppError::bad_request("Agent runtimeMode 不支持"))
}

fn normalize_agent_execution_mode(execution_mode: Option<String>) -> Result<String, AppError> {
    let Some(execution_mode) = execution_mode
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return Ok(AGENT_RUN_EXECUTION_MODE_INLINE.to_owned());
    };
    if matches!(
        execution_mode.as_str(),
        AGENT_RUN_EXECUTION_MODE_INLINE | AGENT_RUN_EXECUTION_MODE_QUEUED
    ) {
        return Ok(execution_mode);
    }
    Err(AppError::bad_request("Agent executionMode 不支持"))
}

const WORKBENCH_CONTEXT_MAX_IDS: usize = 16;
const WORKBENCH_CONTEXT_MAX_CODES: usize = 16;
const AGENT_SKILL_CONTEXT_CHARS: usize = 8000;
const AGENT_SKILL_MD_CHARS: usize = 2400;
const AGENT_SKILL_METADATA_CHARS: usize = 1600;
const AGENT_SKILL_REFERENCE_CHARS: usize = 1200;
const AGENT_SKILL_REFERENCE_LIMIT: usize = 3;

fn normalize_agent_workbench_context(
    context: Option<AgentWorkbenchContext>,
) -> Option<AgentWorkbenchContext> {
    let mut context = context?;
    let mode = context.mode.trim();
    context.mode = if mode.is_empty() {
        "agent".to_owned()
    } else {
        mode.to_owned()
    };
    context.document_ids =
        normalized_positive_i64_list(context.document_ids, WORKBENCH_CONTEXT_MAX_IDS);
    context.file_ids = normalized_positive_i64_list(context.file_ids, WORKBENCH_CONTEXT_MAX_IDS);
    context.skill_codes = normalized_code_list(context.skill_codes, WORKBENCH_CONTEXT_MAX_CODES);
    context.mcp_tool_codes =
        normalized_code_list(context.mcp_tool_codes, WORKBENCH_CONTEXT_MAX_CODES);
    context.route_id = context
        .route_id
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());

    let has_context = context.dataset_id.is_some()
        || !context.document_ids.is_empty()
        || !context.file_ids.is_empty()
        || !context.skill_codes.is_empty()
        || !context.mcp_tool_codes.is_empty()
        || context.web_search_enabled
        || context.route_id.is_some();

    has_context.then_some(context)
}

fn normalized_positive_i64_list(values: Vec<i64>, limit: usize) -> Vec<i64> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| *value > 0)
        .filter(|value| seen.insert(*value))
        .take(limit)
        .collect()
}

fn normalized_code_list(values: Vec<String>, limit: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .take(limit)
        .collect()
}

fn agent_run_command_payload(command: &AgentRunCommand) -> Value {
    json!({
        "input": command.input,
        "runtimeMode": command.runtime_mode,
        "executionMode": command.execution_mode,
        "modelRouteId": command.model_route_id,
        "autoApprove": command.auto_approve,
        "budget": command.budget,
        "workbenchContext": command.workbench_context
    })
}

fn agent_run_command_from_queue_payload(payload: Value) -> Result<AgentRunCommand, AppError> {
    let command_payload = payload.get("command").cloned().unwrap_or(payload);
    let command = serde_json::from_value::<AgentRunCommand>(command_payload)
        .map_err(|_| AppError::bad_request("Agent queued command payload 无效"))?;
    normalize_agent_run_command(command)
}

fn agent_resume_queue_payload(command: &AgentRunResumeCommand) -> Value {
    json!({
        "source": "agent.resume_run",
        "executionMode": "queued",
        "resume": {
            "approved": command.approved,
            "input": command.input
        }
    })
}

fn agent_resume_input_from_queue_payload(payload: &Value) -> Result<Option<Value>, AppError> {
    if payload.get("source").and_then(Value::as_str) != Some("agent.resume_run") {
        return Ok(None);
    }
    let Some(resume) = payload.get("resume") else {
        return Err(AppError::bad_request("Agent resume queue payload 无效"));
    };
    if resume.get("approved").and_then(Value::as_bool) != Some(true) {
        return Err(AppError::bad_request(
            "Agent resume queue payload 未通过审批",
        ));
    }
    Ok(Some(resume.get("input").cloned().unwrap_or(Value::Null)))
}

fn build_model_loop_tool_router() -> Result<ToolRouter, ToolRouteError> {
    ToolRouter::from_definitions(agent_model_loop_tool_definitions())
}

fn build_model_loop_tool_executor_registry(
) -> Result<ToolExecutorRegistry, ToolExecutorRegistryError> {
    ToolExecutorRegistry::from_bindings(agent_model_loop_tool_executor_bindings())
}

fn model_loop_tool_executor_binding_payload(binding: Option<&ToolExecutorBinding>) -> Value {
    binding
        .and_then(|binding| serde_json::to_value(binding).ok())
        .unwrap_or(Value::Null)
}

fn build_model_loop_system_prompt(tool_codes: &[String]) -> String {
    build_model_loop_system_prompt_for_date(tool_codes, Utc::now().date_naive())
}

fn build_model_loop_system_prompt_for_date(
    tool_codes: &[String],
    current_date: NaiveDate,
) -> String {
    format!(
        "You are Novex Agent Runtime. Current date: {current_date}. Treat relative dates like today, tomorrow, and yesterday against this runtime date unless the user specifies another timezone. You may answer directly or request tool calls while staying within the run budget. Available tools: {}. After each tool observation, decide whether another tool call is necessary or produce the final answer. To call one tool, reply with compact JSON exactly like {{\"type\":\"tool_call\",\"callId\":\"call-1\",\"toolCode\":\"rag.search\",\"arguments\":{{\"query\":\"...\"}}}}. To call multiple independent tools in the same turn, reply with compact JSON exactly like {{\"type\":\"tool_calls\",\"calls\":[{{\"callId\":\"call-1\",\"toolCode\":\"rag.search\",\"arguments\":{{\"query\":\"...\"}}}},{{\"callId\":\"call-2\",\"toolCode\":\"github.repo.read\",\"arguments\":{{\"repository\":\"org/repo\",\"path\":\"README.md\"}}}}]}}. Otherwise reply with the final answer. Never request a tool outside the available tools or after the tool-call budget is exhausted.",
        tool_codes.join(", ")
    )
}

fn build_model_loop_system_prompt_with_context(
    tool_codes: &[String],
    context: Option<&AgentWorkbenchContext>,
    skill_context: Option<&str>,
) -> String {
    let mut prompt = build_model_loop_system_prompt(tool_codes);
    if let Some(context) = context {
        let mut lines = Vec::new();
        lines.push("Workbench context:".to_owned());
        if let Some(dataset_id) = context.dataset_id {
            lines.push(format!(
                "Use rag.search with datasetId {dataset_id} for file-grounded questions."
            ));
        }
        if !context.document_ids.is_empty() {
            lines.push(format!(
                "Selected document ids: {}.",
                join_i64_values(&context.document_ids)
            ));
        }
        if !context.file_ids.is_empty() {
            lines.push(format!(
                "Selected file ids: {}.",
                join_i64_values(&context.file_ids)
            ));
        }
        if !context.skill_codes.is_empty() {
            lines.push(format!(
                "Selected skill codes: {}.",
                context.skill_codes.join(", ")
            ));
        }
        if !context.mcp_tool_codes.is_empty() {
            lines.push(format!(
                "Selected MCP tool codes: {}.",
                context.mcp_tool_codes.join(", ")
            ));
        }
        if context.web_search_enabled {
            lines.push(
                "Web search is enabled; web.search may be used for fresh external facts."
                    .to_owned(),
            );
        } else {
            lines.push("Web search is disabled for this run.".to_owned());
        }
        prompt.push(' ');
        prompt.push_str(&lines.join(" "));
    }
    if let Some(skill_context) = skill_context
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        prompt.push(' ');
        prompt.push_str("Loaded skill instructions:\n");
        prompt.push_str(skill_context);
    }
    prompt
}

fn agent_skill_context_for_record(
    record: &CapabilityRecord,
    resources: &[SkillResourceRecord],
    question: &str,
) -> String {
    let mut parts = Vec::new();
    parts.push(format!("Skill: {} ({})", record.name, record.code));

    let skill_md = resources
        .iter()
        .find(|resource| {
            resource.resource_type == "skill_md"
                || resource.relative_path.eq_ignore_ascii_case("SKILL.md")
        })
        .and_then(|resource| resource.content_text.as_deref())
        .or_else(|| record.metadata.get("skillMd").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(skill_md) = skill_md {
        parts.push(preview_chars(skill_md, AGENT_SKILL_MD_CHARS));
    }

    let metadata_instruction = agent_skill_metadata_instruction(record);
    if !metadata_instruction.is_empty() {
        parts.push(metadata_instruction);
    }

    if let Some(reference_instruction) = agent_skill_reference_instruction_for_question(
        question,
        resources,
        AGENT_SKILL_REFERENCE_LIMIT,
    ) {
        parts.push(reference_instruction);
    }

    preview_chars(&parts.join("\n\n"), AGENT_SKILL_CONTEXT_CHARS)
}

fn agent_skill_metadata_instruction(record: &CapabilityRecord) -> String {
    let mut parts = Vec::new();
    push_json_instruction(&mut parts, &record.metadata, "systemPrompt");
    push_json_instruction(&mut parts, &record.metadata, "instruction");
    push_json_instruction(&mut parts, &record.metadata, "instructions");
    push_json_instruction(&mut parts, &record.metadata, "prompt");
    push_json_instruction(&mut parts, &record.metadata, "promptRules");
    push_json_instruction(&mut parts, &record.metadata, "rules");
    if parts.is_empty() && !record.description.trim().is_empty() {
        parts.push(record.description.trim().to_owned());
    }
    preview_chars(&parts.join("\n"), AGENT_SKILL_METADATA_CHARS)
}

fn push_json_instruction(parts: &mut Vec<String>, metadata: &Value, key: &str) {
    match metadata.get(key) {
        Some(Value::String(value)) => {
            let value = value.trim();
            if !value.is_empty() {
                parts.push(value.to_owned());
            }
        }
        Some(Value::Array(values)) => {
            for value in values {
                if let Some(value) = value
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    parts.push(format!("- {value}"));
                }
            }
        }
        _ => {}
    }
}

fn agent_skill_reference_instruction_for_question(
    question: &str,
    resources: &[SkillResourceRecord],
    limit: usize,
) -> Option<String> {
    if limit == 0 || question.trim().is_empty() {
        return None;
    }
    let chunks = resources
        .iter()
        .filter(|resource| resource.resource_type == "reference")
        .filter_map(|resource| {
            let content_text = resource.content_text.as_deref()?.trim();
            if content_text.is_empty() {
                return None;
            }
            Some(agent_skill_reference_chunk(resource, content_text))
        })
        .collect::<Vec<_>>();
    if chunks.is_empty() {
        return None;
    }
    let hits = novex_rag::keyword_retrieve(question, &chunks, limit);
    if hits.is_empty() {
        return None;
    }

    let mut instruction =
        "Relevant Skill References:\nUse these skill-local references only when they help the current run. Do not execute imported scripts or claim unavailable tools ran."
            .to_owned();
    let mut appended = false;
    for hit in hits
        .into_iter()
        .filter(|hit| agent_skill_reference_has_relevant_overlap(question, &hit.chunk.text))
    {
        appended = true;
        let path = hit
            .chunk
            .metadata
            .source_file_name
            .as_deref()
            .unwrap_or(&hit.chunk.chunk_id);
        instruction.push_str("\n\n[");
        instruction.push_str(path);
        instruction.push_str("]\n");
        instruction.push_str(&preview_chars(&hit.chunk.text, AGENT_SKILL_REFERENCE_CHARS));
    }
    appended.then(|| preview_chars(&instruction, AGENT_SKILL_CONTEXT_CHARS))
}

fn agent_skill_reference_has_relevant_overlap(question: &str, text: &str) -> bool {
    let query_terms = agent_skill_terms(question);
    if query_terms.is_empty() {
        return false;
    }
    let text_terms = agent_skill_terms(text);
    query_terms.iter().any(|term| text_terms.contains(term))
}

fn agent_skill_terms(text: &str) -> BTreeSet<String> {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .map(str::trim)
        .filter(|term| term.len() >= 3)
        .map(|term| term.to_ascii_lowercase())
        .map(|term| {
            if term.len() > 4 && term.ends_with('s') {
                term.trim_end_matches('s').to_owned()
            } else {
                term
            }
        })
        .filter(|term| !AGENT_SKILL_STOP_WORDS.contains(&term.as_str()))
        .collect()
}

const AGENT_SKILL_STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "can", "you", "your", "after", "before", "within",
    "from", "into", "onto", "are", "was", "were", "has", "have", "had", "requires", "required",
];

fn agent_skill_reference_chunk(
    resource: &SkillResourceRecord,
    content_text: &str,
) -> novex_rag::DocumentChunk {
    let mut metadata = novex_rag::ChunkMetadata::default();
    metadata.source_file_name = Some(resource.relative_path.clone());
    metadata.source_title = Some(resource.relative_path.clone());
    metadata.source_content_type = Some(resource.mime_type.clone());
    novex_rag::DocumentChunk {
        document_id: format!("skill-reference:{}", resource.skill_id),
        chunk_id: resource.relative_path.clone(),
        chunk_index: 0,
        text: content_text.to_owned(),
        semantic_search_text: format!("{}\n{}", resource.relative_path, content_text),
        token_count: content_text.split_whitespace().count(),
        citation: novex_rag::CitationRef {
            document_id: format!("skill-reference:{}", resource.skill_id),
            chunk_id: resource.relative_path.clone(),
            page_no: None,
            section_path: vec![],
        },
        metadata,
    }
}

fn preview_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut preview = text
        .chars()
        .take(limit.saturating_sub(3))
        .collect::<String>();
    preview.push_str("...");
    preview
}

fn join_i64_values(values: &[i64]) -> String {
    values
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn agent_runtime_budget_from_task_budget(budget: TaskBudget) -> AgentRuntimeBudget {
    AgentRuntimeBudget {
        max_turns: budget.max_steps.unwrap_or(novex_ai_core::DEFAULT_MAX_STEPS) as usize,
        max_tool_calls: budget
            .max_tool_calls
            .unwrap_or(novex_ai_core::DEFAULT_MAX_TOOL_CALLS) as usize,
        compact_after_observations: Some(2),
    }
}

fn tool_observation_status_for_execution(execution: &AgentToolExecution) -> ToolObservationStatus {
    if execution.succeeded_status() {
        return ToolObservationStatus::Succeeded;
    }
    if execution.cancelled_status() {
        return ToolObservationStatus::Cancelled;
    }
    ToolObservationStatus::Failed
}

fn tool_io_metrics_payload(metrics: &AgentToolIoMetrics) -> Value {
    metrics.payload()
}

fn model_loop_cancel_requested(status: &str) -> bool {
    matches!(
        parse_run_status_code(status),
        Some(RunStatus::Cancelling | RunStatus::Cancelled)
    )
}

fn model_loop_external_cancel_payload(stage: &str) -> Value {
    json!({
        "cancelled": true,
        "cancelReason": "external_cancel",
        "cancelStage": stage,
        "runtimeMode": "model_loop",
    })
}

fn runtime_cancelled_event_payload(signal: AgentRuntimeCancelSignal) -> Value {
    let supervisor = signal
        .snapshot
        .map(|snapshot| {
            let mut value = serde_json::to_value(snapshot).unwrap_or_else(|_| json!({}));
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    "activeBeforeCancel".to_owned(),
                    json!(signal.active_before_cancel),
                );
            }
            value
        })
        .unwrap_or_else(|| {
            json!({
                "activeBeforeCancel": signal.active_before_cancel,
            })
        });

    json!({
        "cancelled": true,
        "runtimeSignalSent": signal.sent,
        "runtimeSupervisor": supervisor,
    })
}

fn provider_native_cancel_event_payload(cancel: &ModelProviderCallLeaseCancelResp) -> Value {
    let native = &cancel.native_cancel;
    let mut item = json!({
        "type": "provider_native_cancel",
        "providerCallLeaseId": cancel.lease_id,
        "status": cancel.status,
        "attempted": native.attempted,
        "supported": native.supported,
        "provider": native.provider,
        "message": native.message,
    });
    if let Some(object) = item.as_object_mut() {
        if let Some(provider_response_id) = native.provider_response_id.as_deref() {
            object.insert("providerResponseId".to_owned(), json!(provider_response_id));
        }
        if let Some(endpoint) = native.endpoint.as_deref() {
            object.insert("endpoint".to_owned(), json!(endpoint));
        }
        if let Some(http_status) = native.http_status {
            object.insert("httpStatus".to_owned(), json!(http_status));
        }
    }

    json!({
        "runtimeMode": "model_loop",
        "item": item
    })
}

fn provider_native_cancel_error_event_payload(
    provider_call_lease_id: i64,
    provider_response_id: Option<&str>,
    error: &AppError,
) -> Value {
    let mut item = json!({
        "type": "provider_native_cancel_error",
        "providerCallLeaseId": provider_call_lease_id,
        "message": model_inference_error_message(error),
    });
    if let Some(object) = item.as_object_mut() {
        if let Some(provider_response_id) = provider_response_id {
            object.insert("providerResponseId".to_owned(), json!(provider_response_id));
        }
    }

    json!({
        "runtimeMode": "model_loop",
        "item": item
    })
}

fn model_inference_event_payload(response: &ModelChatResp) -> Value {
    let fallback_route_id = response
        .provider_attempts
        .iter()
        .find(|attempt| attempt.attempt_kind == "fallback" && attempt.status == "succeeded")
        .map(|attempt| attempt.route_id.clone());
    let circuit_open = response
        .provider_attempts
        .iter()
        .any(|attempt| attempt.error_kind.as_deref() == Some("circuit_open"));
    let mut item = json!({
        "type": "model_inference",
        "routeId": &response.route_id,
        "provider": &response.provider,
        "model": &response.model,
        "latencyMs": u128_to_i64(response.latency_ms),
        "usage": &response.usage,
        "costCents": response.cost_cents,
    });
    if let Some(object) = item.as_object_mut() {
        if !response.provider_attempts.is_empty() {
            object.insert(
                "providerAttempts".to_owned(),
                json!(&response.provider_attempts),
            );
        }
        if let Some(fallback_route_id) = fallback_route_id {
            object.insert("fallbackUsed".to_owned(), json!(true));
            object.insert("fallbackRouteId".to_owned(), json!(fallback_route_id));
        }
        if let Some(provider_call_lease_id) = response.provider_call_lease_id {
            object.insert(
                "providerCallLeaseId".to_owned(),
                json!(provider_call_lease_id),
            );
        }
        if !response.provider_delta_chunks.is_empty() {
            object.insert("streaming".to_owned(), json!(true));
            object.insert(
                "deltaChunkCount".to_owned(),
                json!(response.provider_delta_chunks.len()),
            );
            object.insert(
                "deltaTextLength".to_owned(),
                json!(response
                    .provider_delta_chunks
                    .iter()
                    .map(|chunk| chunk.content.chars().count())
                    .sum::<usize>()),
            );
        }
        if circuit_open {
            object.insert("circuitOpen".to_owned(), json!(true));
        }
    }

    json!({
        "runtimeMode": "model_loop",
        "item": item
    })
}

fn model_delta_event_payload(response: &ModelChatResp, chunk: &ModelProviderStreamChunk) -> Value {
    let mut item = json!({
        "type": "model_delta",
        "source": "provider_stream",
        "routeId": &response.route_id,
        "provider": &response.provider,
        "model": &response.model,
        "deltaIndex": chunk.index,
        "content": &chunk.content,
    });
    if let Some(object) = item.as_object_mut() {
        if let Some(provider_event) = chunk.provider_event.as_deref() {
            object.insert("providerEvent".to_owned(), json!(provider_event));
        }
    }

    json!({
        "runtimeMode": "model_loop",
        "item": item
    })
}

fn model_delta_event_payload_from_stream_event(event: &ModelProviderStreamEvent) -> Value {
    let response = ModelChatResp {
        conversation_id: None,
        answer: String::new(),
        route_id: event.route_id.clone(),
        provider: event.provider.clone(),
        model: event.model.clone(),
        latency_ms: 0,
        usage: ModelChatUsage::default(),
        cost_cents: None,
        provider_attempts: vec![],
        provider_call_lease_id: event.provider_call_lease_id,
        provider_response_id: None,
        provider_response_status: None,
        provider_delta_chunks: vec![event.chunk.clone()],
    };
    let mut payload = model_delta_event_payload(&response, &event.chunk);
    if let Some(item) = payload.get_mut("item").and_then(Value::as_object_mut) {
        if let Some(provider_call_lease_id) = event.provider_call_lease_id {
            item.insert(
                "providerCallLeaseId".to_owned(),
                json!(provider_call_lease_id),
            );
        }
        if let Some(provider_response_id) = event.provider_response_id.as_deref() {
            item.insert("providerResponseId".to_owned(), json!(provider_response_id));
        }
        if let Some(provider_response_status) = event.provider_response_status.as_deref() {
            item.insert(
                "providerResponseStatus".to_owned(),
                json!(provider_response_status),
            );
        }
    }
    payload
}

fn model_stream_tool_call_event_payload(
    event: &ModelProviderStreamEvent,
    parsed: &ParsedModelTurnOutput,
) -> Value {
    let tool_calls = parsed
        .items
        .iter()
        .filter_map(|item| match item {
            AgentTurnItem::ToolCall {
                call_id,
                tool_code,
                arguments,
            } => Some(json!({
                "callId": call_id,
                "toolCode": tool_code,
                "arguments": arguments,
            })),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut payload = json!({
        "runtimeMode": "model_loop",
        "item": {
            "type": "model_stream_tool_call",
            "source": "provider_stream",
            "routeId": &event.route_id,
            "provider": &event.provider,
            "model": &event.model,
            "deltaIndex": event.chunk.index,
            "toolCallCount": tool_calls.len(),
            "toolCalls": tool_calls,
        }
    });
    if let Some(item) = payload.get_mut("item").and_then(Value::as_object_mut) {
        if let Some(provider_call_lease_id) = event.provider_call_lease_id {
            item.insert(
                "providerCallLeaseId".to_owned(),
                json!(provider_call_lease_id),
            );
        }
        if let Some(provider_response_id) = event.provider_response_id.as_deref() {
            item.insert("providerResponseId".to_owned(), json!(provider_response_id));
        }
        if let Some(provider_response_status) = event.provider_response_status.as_deref() {
            item.insert(
                "providerResponseStatus".to_owned(),
                json!(provider_response_status),
            );
        }
    }
    payload
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelInferenceErrorClass {
    kind: &'static str,
    http_status: Option<u16>,
    retryable: bool,
}

#[allow(dead_code)]
fn model_inference_error_event_payload(error: &AppError, latency_ms: u128) -> Value {
    model_inference_error_attempt_event_payload(error, latency_ms, 1, 1, false)
}

fn model_inference_error_attempt_event_payload(
    error: &AppError,
    latency_ms: u128,
    attempt: usize,
    max_attempts: usize,
    will_retry: bool,
) -> Value {
    let class = classify_model_inference_error(error);
    let mut item = json!({
        "type": "model_inference_error",
        "routeId": CODE_AGENT_MODEL_ROUTE_ID,
        "routePurpose": CODE_AGENT_ROUTE_PURPOSE,
        "attempt": attempt,
        "maxAttempts": max_attempts,
        "willRetry": will_retry,
        "retryable": class.retryable,
        "errorKind": class.kind,
        "message": model_inference_error_message(error),
        "latencyMs": u128_to_i64(latency_ms),
    });
    if let Some(http_status) = class.http_status {
        if let Some(object) = item.as_object_mut() {
            object.insert("httpStatus".to_owned(), json!(http_status));
        }
    }

    json!({
        "runtimeMode": "model_loop",
        "item": item
    })
}

fn model_inference_error_should_retry(error: &AppError) -> bool {
    classify_model_inference_error(error).retryable
}

fn model_inference_retry_delay(attempt: usize) -> Duration {
    Duration::from_millis((attempt as u64).saturating_mul(50).min(250))
}

fn classify_model_inference_error(error: &AppError) -> ModelInferenceErrorClass {
    let message = model_inference_error_message(error);
    let http_status = model_inference_http_status(&message);
    let kind = match error {
        AppError::BadRequest(_) if http_status.is_some() => "provider_http",
        AppError::BadRequest(_) if model_inference_error_is_timeout(&message) => "provider_timeout",
        AppError::BadRequest(_) => "invalid_model_request",
        AppError::Unauthorized => "unauthorized",
        AppError::Forbidden => "forbidden",
        AppError::NotFound => "not_found",
        AppError::Conflict(_) => "conflict",
        AppError::Sqlx(_) | AppError::Io(_) | AppError::Anyhow(_) => "provider_transport",
    };
    let retryable = match kind {
        "provider_http" => http_status.is_some_and(|status| status == 429 || status >= 500),
        "provider_timeout" | "provider_transport" => true,
        _ => false,
    };

    ModelInferenceErrorClass {
        kind,
        http_status,
        retryable,
    }
}

fn model_inference_error_message(error: &AppError) -> String {
    match error {
        AppError::BadRequest(message) | AppError::Conflict(message) => message.clone(),
        AppError::Unauthorized => "unauthorized model request".to_owned(),
        AppError::Forbidden => "forbidden model request".to_owned(),
        AppError::NotFound => "model route not found".to_owned(),
        AppError::Sqlx(_) | AppError::Io(_) | AppError::Anyhow(_) => {
            "model provider transport error".to_owned()
        }
    }
}

fn model_inference_http_status(message: &str) -> Option<u16> {
    let marker_index = message.find("HTTP")?;
    let digits = message[marker_index + "HTTP".len()..]
        .trim_start()
        .chars()
        .take_while(|char| char.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u16>().ok()
}

fn model_inference_error_is_timeout(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("timeout") || message.contains("timed out") || message.contains("超时")
}

fn u128_to_i64(value: u128) -> i64 {
    value.min(i64::MAX as u128) as i64
}

#[derive(Debug, Clone)]
struct ModelLoopToolCallProjection {
    call_id: String,
    tool_code: String,
    arguments: Value,
}

#[derive(Debug, Clone)]
struct ModelLoopToolObservationProjection {
    call_id: String,
    tool_code: Option<String>,
    status: ToolObservationStatus,
    output: Value,
}

fn build_model_loop_messages_from_history(
    original_input: &str,
    tool_codes: &[String],
    workbench_context: Option<&AgentWorkbenchContext>,
    skill_context: Option<&str>,
    history: &[AgentTurnItem],
) -> Vec<ModelChatMessage> {
    let mut messages = vec![ModelChatMessage {
        role: "system".to_owned(),
        content: build_model_loop_system_prompt_with_context(
            tool_codes,
            workbench_context,
            skill_context,
        ),
    }];
    let original_user_input = history
        .iter()
        .find_map(|item| match item {
            AgentTurnItem::UserMessage { content } => Some(content.as_str()),
            _ => None,
        })
        .unwrap_or(original_input);

    if let Some(compaction_index) = history
        .iter()
        .rposition(|item| matches!(item, AgentTurnItem::ContextCompaction { .. }))
    {
        messages.push(ModelChatMessage {
            role: "user".to_owned(),
            content: original_user_input.to_owned(),
        });
        append_model_loop_history_messages(&mut messages, &history[compaction_index..]);
        return messages;
    }

    if !history
        .iter()
        .any(|item| matches!(item, AgentTurnItem::UserMessage { .. }))
    {
        messages.push(ModelChatMessage {
            role: "user".to_owned(),
            content: original_user_input.to_owned(),
        });
    }
    append_model_loop_history_messages(&mut messages, history);
    messages
}

fn append_model_loop_history_messages(
    messages: &mut Vec<ModelChatMessage>,
    history: &[AgentTurnItem],
) {
    let mut tool_code_by_call_id = HashMap::new();
    let mut index = 0;

    while index < history.len() {
        match &history[index] {
            AgentTurnItem::UserMessage { content } => {
                messages.push(ModelChatMessage {
                    role: "user".to_owned(),
                    content: content.clone(),
                });
                index += 1;
            }
            AgentTurnItem::AssistantMessage { content }
            | AgentTurnItem::FinalAnswer { content } => {
                messages.push(ModelChatMessage {
                    role: "assistant".to_owned(),
                    content: content.clone(),
                });
                index += 1;
            }
            AgentTurnItem::Reasoning { summary } => {
                messages.push(ModelChatMessage {
                    role: "assistant".to_owned(),
                    content: format!("Reasoning summary:\n{summary}"),
                });
                index += 1;
            }
            AgentTurnItem::ToolCall { .. } => {
                let mut calls = Vec::new();
                while let Some(AgentTurnItem::ToolCall {
                    call_id,
                    tool_code,
                    arguments,
                }) = history.get(index)
                {
                    tool_code_by_call_id.insert(call_id.clone(), tool_code.clone());
                    calls.push(ModelLoopToolCallProjection {
                        call_id: call_id.clone(),
                        tool_code: tool_code.clone(),
                        arguments: arguments.clone(),
                    });
                    index += 1;
                }
                messages.push(ModelChatMessage {
                    role: "assistant".to_owned(),
                    content: serialize_model_loop_tool_calls(&calls),
                });
            }
            AgentTurnItem::ToolObservation { .. } => {
                let mut observations = Vec::new();
                while let Some(AgentTurnItem::ToolObservation {
                    call_id,
                    status,
                    output,
                }) = history.get(index)
                {
                    observations.push(ModelLoopToolObservationProjection {
                        call_id: call_id.clone(),
                        tool_code: tool_code_by_call_id.get(call_id).cloned(),
                        status: *status,
                        output: output.clone(),
                    });
                    index += 1;
                }
                messages.push(ModelChatMessage {
                    role: "user".to_owned(),
                    content: build_model_loop_observation_history_prompt(&observations),
                });
            }
            AgentTurnItem::ContextCompaction { summary } => {
                messages.push(ModelChatMessage {
                    role: "user".to_owned(),
                    content: build_model_loop_compaction_history_prompt(summary),
                });
                index += 1;
            }
        }
    }
}

fn serialize_model_loop_tool_calls(calls: &[ModelLoopToolCallProjection]) -> String {
    let payload = if calls.len() == 1 {
        let call = &calls[0];
        json!({
            "type": "tool_call",
            "callId": call.call_id,
            "toolCode": call.tool_code,
            "arguments": call.arguments,
        })
    } else {
        let calls = calls
            .iter()
            .map(|call| {
                json!({
                    "callId": call.call_id,
                    "toolCode": call.tool_code,
                    "arguments": call.arguments,
                })
            })
            .collect::<Vec<_>>();
        json!({
            "type": "tool_calls",
            "calls": calls,
        })
    };

    serde_json::to_string(&payload).unwrap_or_else(|_| "{\"type\":\"tool_calls\"}".to_owned())
}

fn build_model_loop_observation_history_prompt(
    observations: &[ModelLoopToolObservationProjection],
) -> String {
    if observations.len() == 1 {
        let observation = &observations[0];
        let tool_code = observation.tool_code.as_deref().unwrap_or("unknown_tool");
        return format!(
            "Tool observation for call `{}` (`{}`, status: {}):\n{}\nUse it to produce the final answer. If the observation is insufficient, say what is missing.",
            observation.call_id,
            tool_code,
            model_loop_tool_observation_status_code(observation.status),
            serde_json::to_string_pretty(&observation.output).unwrap_or_else(|_| "{}".to_owned())
        );
    }

    let payload = observations
        .iter()
        .map(|observation| {
            json!({
                "callId": observation.call_id,
                "toolCode": observation.tool_code.as_deref().unwrap_or("unknown_tool"),
                "status": model_loop_tool_observation_status_code(observation.status),
                "observation": observation.output,
            })
        })
        .collect::<Vec<_>>();
    format!(
        "The requested tool batch returned these observations:\n{}\nUse them to produce the final answer. If the observations are insufficient, say what is missing.",
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "[]".to_owned())
    )
}

fn model_loop_tool_observation_status_code(status: ToolObservationStatus) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{status:?}").to_ascii_lowercase())
}

fn build_model_loop_compaction_history_prompt(summary: &str) -> String {
    format!(
        "Prior agent context was compacted to keep the run inside the model context window:\n{summary}\nContinue from this compacted context. You may call another available tool if needed, otherwise produce the final answer."
    )
}

#[cfg(test)]
fn build_model_loop_context_compaction_messages(
    original_input: &str,
    deterministic_summary: &str,
    tool_codes: &[String],
) -> Vec<ModelChatMessage> {
    build_model_loop_remote_context_compaction_messages(
        original_input,
        deterministic_summary,
        tool_codes,
        None,
    )
}

fn build_model_loop_remote_context_compaction_messages(
    original_input: &str,
    deterministic_summary: &str,
    tool_codes: &[String],
    remote_compaction_request: Option<&AgentRemoteCompactionRequest>,
) -> Vec<ModelChatMessage> {
    let remote_metadata = remote_compaction_request
        .map(remote_compaction_prompt_metadata)
        .unwrap_or_else(|| {
            json!({
                "implementation": "responses_compaction_v2",
                "trigger": "auto",
                "reason": "observation_threshold",
                "phase": "model_loop_follow_up",
                "inputHistoryCount": Value::Null,
                "retainedHistoryCount": Value::Null,
            })
        });
    vec![
        ModelChatMessage {
            role: "system".to_owned(),
            content: "You are Novex Agent Context Compactor, acting as a remote compaction endpoint adapter. Rewrite prior agent context into a compact, factual summary for the next model turn. Preserve user intent, tool evidence, unresolved questions, citations, and decisions. Do not answer the user and do not request tools. Return either plain text or compact JSON like {\"summary\":\"...\"}.".to_owned(),
        },
        ModelChatMessage {
            role: "user".to_owned(),
            content: format!(
                "Original user request:\n{original_input}\n\nRemote compaction endpoint metadata:\n{}\n\nAvailable tools for the next turn:\n{}\n\nExisting deterministic summary candidate:\n{deterministic_summary}\n\nProduce the shortest useful continuation summary.",
                remote_metadata,
                tool_codes.join(", ")
            ),
        },
    ]
}

fn model_chat_request_metadata_for_remote_compaction(
    remote_compaction_request: Option<&AgentRemoteCompactionRequest>,
) -> Option<ModelChatRequestMetadata> {
    let request = remote_compaction_request?;
    Some(ModelChatRequestMetadata::remote_compaction(
        ModelChatCompactionMetadata {
            implementation: serialized_compaction_metadata_value(request.implementation),
            trigger: serialized_compaction_metadata_value(request.trigger),
            reason: serialized_compaction_metadata_value(request.reason),
            phase: serialized_compaction_metadata_value(request.phase),
            strategy: "memento".to_owned(),
            window_id: request.window_id,
            input_history_count: request.input_history.len(),
            retained_history_count: request.retained_history.len(),
            compacted_item_count: request.compacted_item_count,
            retained_item_count: request.retained_item_count,
            tool_codes: request.tool_codes.clone(),
        },
    ))
}

fn serialized_compaction_metadata_value<T: Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

fn remote_compaction_prompt_metadata(request: &AgentRemoteCompactionRequest) -> Value {
    json!({
        "implementation": request.implementation,
        "trigger": request.trigger,
        "reason": request.reason,
        "phase": request.phase,
        "windowId": request.window_id,
        "inputHistoryCount": request.input_history.len(),
        "retainedHistoryCount": request.retained_history.len(),
        "compactedItemCount": request.compacted_item_count,
        "retainedItemCount": request.retained_item_count,
        "toolCodes": request.tool_codes,
    })
}

fn model_loop_context_compaction_summary_from_response(answer: &str) -> String {
    let trimmed = answer.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(summary) = value
            .get("summary")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|summary| !summary.is_empty())
        {
            return summary.to_owned();
        }
    }
    trimmed.to_owned()
}

fn tool_route_error_to_app_error(err: ToolRouteError) -> AppError {
    AppError::bad_request(format!("Agent 工具路由初始化失败: {}", err.message))
}

fn tool_executor_registry_error_to_app_error(err: ToolExecutorRegistryError) -> AppError {
    AppError::Anyhow(anyhow::anyhow!(
        "Agent 工具执行器注册表初始化失败: {}",
        err.message
    ))
}

fn tool_route_stop_reason(kind: ToolRouteErrorKind) -> &'static str {
    match kind {
        ToolRouteErrorKind::UnknownTool => "unknown_tool",
        ToolRouteErrorKind::EmptyToolCode => "invalid_tool",
        ToolRouteErrorKind::DuplicateToolCode => "tool_router_error",
    }
}

fn tool_route_failure_message(err: &ToolRouteError) -> String {
    match err.kind {
        ToolRouteErrorKind::UnknownTool => format!(
            "Model requested unavailable tool `{}`.",
            err.tool_code.as_deref().unwrap_or("unknown")
        ),
        ToolRouteErrorKind::EmptyToolCode => "Model requested an empty tool code.".to_owned(),
        ToolRouteErrorKind::DuplicateToolCode => {
            format!("Tool router configuration error: {}", err.message)
        }
    }
}

fn build_agent_plan(
    command: &AgentRunCommand,
    memory_context: MemoryContext,
) -> Result<AgentPlanSummary, AppError> {
    let plan = plan_react_run_with_memory(&command.input, command.budget, memory_context)
        .map_err(|err| AppError::bad_request(format!("Agent 计划失败: {:?}", err)))?;
    if plan.selected_tool.is_some() && plan.budget.max_tool_calls.unwrap_or_default() == 0 {
        return Err(AppError::bad_request("工具调用预算不足"));
    }
    let selected_tool_code = plan.selected_tool.map(|tool| tool.code);
    let requires_approval = selected_tool_code
        .as_deref()
        .is_some_and(|code| code != "rag.search")
        && !command.auto_approve;
    Ok(AgentPlanSummary {
        intent: intent_code(plan.intent),
        loop_kind: loop_kind_code(plan.loop_kind),
        selected_tool_code,
        requires_approval,
        pause_reason: requires_approval.then(|| "approval".to_owned()),
        initial_status: if requires_approval {
            run_status_code(RunStatus::WaitingApproval)
        } else {
            run_status_code(RunStatus::Running)
        },
        task_budget: plan.budget,
        memory_context: plan.memory_context,
    })
}

fn agent_memory_context_from_records(
    tenant_id: i64,
    user_id: i64,
    records: Vec<MemoryRecord>,
) -> MemoryContext {
    let tenant_id = tenant_id.to_string();
    let user_id = user_id.to_string();
    let candidates = records
        .into_iter()
        .filter_map(|record| memory_snippet_from_record(&tenant_id, record))
        .collect::<Vec<_>>();

    build_memory_context(
        candidates,
        &MemoryAccessContext {
            tenant_id,
            subject_id: user_id.clone(),
            allowed_scopes: vec![MemoryScopeRef {
                scope: MemoryScope::User,
                scope_id: user_id,
            }],
            max_snippets: MAX_AGENT_MEMORY_SNIPPETS,
        },
    )
}

fn memory_snippet_from_record(tenant_id: &str, record: MemoryRecord) -> Option<MemorySnippet> {
    let key = memory_key_from_record(&record);
    Some(MemorySnippet {
        tenant_id: tenant_id.to_owned(),
        scope: memory_scope_from_str(&record.scope_type)?,
        scope_id: record.scope_id,
        key,
        content: record.content,
        write_policy: memory_write_policy_from_str(&record.write_policy)?,
    })
}

fn memory_scope_from_str(value: &str) -> Option<MemoryScope> {
    match value.trim() {
        "session" => Some(MemoryScope::Session),
        "user" => Some(MemoryScope::User),
        "org" => Some(MemoryScope::Org),
        "project" => Some(MemoryScope::Project),
        _ => None,
    }
}

fn memory_write_policy_from_str(value: &str) -> Option<MemoryWritePolicy> {
    match value.trim() {
        "disabled" => Some(MemoryWritePolicy::Disabled),
        "user_approved" => Some(MemoryWritePolicy::UserApproved),
        "automatic" => Some(MemoryWritePolicy::Automatic),
        _ => None,
    }
}

fn memory_key_from_record(record: &MemoryRecord) -> String {
    record
        .metadata
        .get("key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let summary = record.summary.trim();
            (!summary.is_empty()).then_some(summary)
        })
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("memory:{}", record.id))
}

fn agent_context_retrieval_payload(input: &str, memory_context: &MemoryContext) -> Value {
    json!({
        "retrievalKind": "agent_context",
        "query": input,
        "hitCount": memory_context.snippets.len(),
        "source": if memory_context.snippets.is_empty() { "run_input" } else { "ai_memory" },
        "memoryContext": serde_json::to_value(memory_context).unwrap_or_else(|_| json!({ "snippets": [] }))
    })
}

fn agent_turn_item_event_payload(item: &novex_agent_protocol::AgentTurnItem) -> Value {
    json!({
        "eventSource": "novex-agent-runtime",
        "item": serde_json::to_value(item).unwrap_or(Value::Null),
    })
}

fn agent_turn_item_save_record_from_event_payload(
    tenant_id: i64,
    run_id: i64,
    step_id: Option<i64>,
    source_event_id: i64,
    sequence_no: i64,
    payload: &Value,
    user_id: i64,
    now: NaiveDateTime,
) -> Option<AgentTurnItemSaveRecord> {
    if payload.get("eventSource").and_then(Value::as_str) != Some("novex-agent-runtime") {
        return None;
    }
    let item_payload = payload.get("item")?.clone();
    let item = serde_json::from_value::<AgentTurnItem>(item_payload.clone()).ok()?;

    Some(AgentTurnItemSaveRecord {
        id: next_id(),
        tenant_id,
        run_id,
        step_id,
        source_event_id,
        sequence_no,
        item_type: agent_turn_item_type_code(&item).to_owned(),
        call_id: agent_turn_item_call_id(&item),
        tool_code: agent_turn_item_tool_code(&item),
        item_payload,
        user_id,
        now,
    })
}

fn agent_turn_item_from_record(record: AgentTurnItemRecord) -> Result<AgentTurnItem, AppError> {
    serde_json::from_value::<AgentTurnItem>(record.item_payload).map_err(|err| {
        AppError::bad_request(format!("Agent turn item replay payload invalid: {err}"))
    })
}

fn agent_turn_item_type_code(item: &AgentTurnItem) -> &'static str {
    match item {
        AgentTurnItem::UserMessage { .. } => "user_message",
        AgentTurnItem::AssistantMessage { .. } => "assistant_message",
        AgentTurnItem::Reasoning { .. } => "reasoning",
        AgentTurnItem::ToolCall { .. } => "tool_call",
        AgentTurnItem::ToolObservation { .. } => "tool_observation",
        AgentTurnItem::FinalAnswer { .. } => "final_answer",
        AgentTurnItem::ContextCompaction { .. } => "context_compaction",
    }
}

fn agent_turn_item_call_id(item: &AgentTurnItem) -> Option<String> {
    item.call_id().map(ToOwned::to_owned)
}

fn agent_turn_item_tool_code(item: &AgentTurnItem) -> Option<String> {
    match item {
        AgentTurnItem::ToolCall { tool_code, .. } => Some(tool_code.clone()),
        _ => None,
    }
}

fn agent_events_to_trace_bundle(
    trace_id: impl Into<String>,
    events: Vec<RunEventRecord>,
) -> TraceBundle {
    let mut bundle = TraceBundle::new(trace_id);
    for event in events {
        if let Some(trace_event) = trace_event_from_run_event(&event) {
            bundle = bundle.with_event(trace_event);
        }
    }
    bundle
}

#[cfg(test)]
fn agent_trace_snapshot_payload(trace_id: &str, events: &[RunEventRecord]) -> Value {
    let bundle = agent_events_to_trace_bundle(trace_id, events.to_vec());
    agent_trace_snapshot_payload_for_bundle(events, &bundle)
}

fn agent_trace_snapshot_payload_for_bundle(
    events: &[RunEventRecord],
    bundle: &TraceBundle,
) -> Value {
    let event_snapshot = events
        .iter()
        .cloned()
        .map(AgentRunEventResp::from)
        .collect::<Vec<_>>();
    let summary = bundle.replay_summary();

    json!({
        "events": event_snapshot,
        "traceEvents": bundle.events.clone(),
        "summary": summary,
    })
}

fn trace_event_from_run_event(event: &RunEventRecord) -> Option<TraceEvent> {
    let sequence_no = trace_sequence_no(event.sequence_no);
    match event.event_type.as_str() {
        "input_received" => Some(TraceEvent::user_message(
            sequence_no,
            trace_payload_text(&event.payload, &["input", "content", "query"])
                .unwrap_or_else(|| trace_payload_fallback(&event.payload)),
        )),
        "thought"
            if trace_payload_item_type(&event.payload)
                .as_deref()
                .is_some_and(is_model_inference_trace_item) =>
        {
            Some(TraceEvent::inference(sequence_no, event.payload.clone()))
        }
        "thought" => Some(TraceEvent::assistant_message(
            sequence_no,
            trace_payload_text(&event.payload, &["message", "content", "summary"])
                .unwrap_or_else(|| trace_payload_fallback(&event.payload)),
        )),
        "tool_called" => Some(TraceEvent::tool_call(
            sequence_no,
            trace_call_id(event),
            trace_payload_text(&event.payload, &["toolCode", "tool_code"])
                .unwrap_or_else(|| "unknown".to_owned()),
        )),
        "retrieval" => Some(TraceEvent::retrieval(sequence_no, event.payload.clone())),
        "action_selected" => Some(TraceEvent::action_selected(
            sequence_no,
            event.payload.clone(),
        )),
        "observation"
            if trace_payload_item_type(&event.payload).as_deref() == Some("context_compaction") =>
        {
            Some(TraceEvent::context_compaction(
                sequence_no,
                event.payload.clone(),
            ))
        }
        "observation" => Some(trace_observation_event_from_run_event(sequence_no, event)),
        "approval_requested" => Some(TraceEvent {
            sequence_no,
            kind: novex_trace::TraceEventKind::ApprovalRequested,
            payload: event.payload.clone(),
        }),
        "final_output" => Some(TraceEvent::final_answer(
            sequence_no,
            trace_payload_text(&event.payload, &["answer", "content"])
                .unwrap_or_else(|| trace_payload_fallback(&event.payload)),
        )),
        "cancel_requested" | "cancelled" => {
            Some(TraceEvent::cancellation(sequence_no, event.payload.clone()))
        }
        "error" => Some(TraceEvent::error(
            sequence_no,
            trace_payload_text(&event.payload, &["message", "error"])
                .unwrap_or_else(|| trace_payload_fallback(&event.payload)),
        )),
        _ => None,
    }
}

fn is_model_inference_trace_item(item_type: &str) -> bool {
    matches!(
        item_type,
        "model_inference"
            | "model_inference_error"
            | "model_delta"
            | "model_stream_tool_call"
            | "provider_native_cancel"
            | "provider_native_cancel_error"
    )
}

fn trace_payload_item_type(payload: &Value) -> Option<String> {
    payload
        .get("item")
        .and_then(|item| item.get("type"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn trace_sequence_no(sequence_no: i64) -> i32 {
    sequence_no.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

fn trace_call_id(event: &RunEventRecord) -> String {
    trace_payload_text(&event.payload, &["callId", "call_id"])
        .or_else(|| event.step_id.map(|step_id| format!("step-{step_id}")))
        .unwrap_or_else(|| format!("call-{}", event.sequence_no))
}

fn trace_payload_text(payload: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        trace_value_text(payload.get(*key)).or_else(|| {
            payload
                .get("item")
                .and_then(|item| trace_value_text(item.get(*key)))
        })
    })
}

fn trace_value_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_owned())
        }
        Value::Null => None,
        value => Some(value.to_string()),
    }
}

fn trace_observation_output(payload: &Value) -> Value {
    payload
        .get("item")
        .and_then(|item| item.get("output"))
        .cloned()
        .or_else(|| payload.get("output").cloned())
        .unwrap_or_else(|| payload.clone())
}

fn trace_observation_event_from_run_event(sequence_no: i32, event: &RunEventRecord) -> TraceEvent {
    let mut trace_event = TraceEvent::observation(
        sequence_no,
        trace_call_id(event),
        trace_observation_output(&event.payload),
    );
    if let Some(object) = trace_event.payload.as_object_mut() {
        for key in ["toolCode", "auditId", "dryRun", "runtimeMode", "toolIoTask"] {
            if let Some(value) = event.payload.get(key) {
                object.insert(key.to_owned(), value.clone());
            }
        }
    }
    trace_event
}

fn trace_payload_fallback(payload: &Value) -> String {
    payload.to_string()
}

impl From<AgentRunRecord> for AgentRunResp {
    fn from(record: AgentRunRecord) -> Self {
        Self {
            run_id: record.run_id,
            trace_id: record.trace_id,
            status: record.status,
            intent: record.intent,
            loop_kind: record.loop_kind,
            selected_tool_code: record.selected_tool_code,
            pause_reason: record.pause_reason,
            final_output: record.final_output,
            task_budget: serde_json::from_value(record.task_budget).unwrap_or_default(),
            create_time: format_datetime(record.create_time),
            update_time: record.update_time.map(format_datetime),
        }
    }
}

impl From<RunEventRecord> for AgentRunEventResp {
    fn from(record: RunEventRecord) -> Self {
        Self {
            id: record.id,
            run_id: record.run_id,
            step_id: record.step_id,
            event_type: record.event_type,
            sequence_no: record.sequence_no,
            status: record.status,
            payload: record.payload,
            create_time: format_datetime(record.create_time),
        }
    }
}

impl From<TraceBundle> for AgentTraceReplayResp {
    fn from(bundle: TraceBundle) -> Self {
        Self::from_bundle_with_turn_items(bundle, Vec::new())
    }
}

impl AgentTraceReplayResp {
    fn from_bundle_with_turn_items(bundle: TraceBundle, turn_items: Vec<AgentTurnItem>) -> Self {
        let summary = bundle.replay_summary();
        Self {
            trace_id: bundle.trace_id,
            events: bundle.events,
            summary,
            turn_items,
        }
    }
}

fn run_status_code(status: RunStatus) -> String {
    match status {
        RunStatus::Queued => "queued",
        RunStatus::Running => "running",
        RunStatus::WaitingApproval => "waiting_approval",
        RunStatus::Paused => "paused",
        RunStatus::Resuming => "resuming",
        RunStatus::Cancelling => "cancelling",
        RunStatus::Cancelled => "cancelled",
        RunStatus::Failed => "failed",
        RunStatus::Succeeded => "succeeded",
    }
    .to_owned()
}

fn parse_run_status_code(status: &str) -> Option<RunStatus> {
    Some(match status {
        "queued" => RunStatus::Queued,
        "running" => RunStatus::Running,
        "waiting_approval" => RunStatus::WaitingApproval,
        "paused" => RunStatus::Paused,
        "resuming" => RunStatus::Resuming,
        "cancelling" => RunStatus::Cancelling,
        "cancelled" => RunStatus::Cancelled,
        "failed" => RunStatus::Failed,
        "succeeded" => RunStatus::Succeeded,
        _ => return None,
    })
}

fn ensure_agent_run_transition(from_status: &str, to: RunStatus) -> Result<(), AppError> {
    let Some(from) = parse_run_status_code(from_status) else {
        return Err(AppError::conflict(format!("未知 Run 状态: {from_status}")));
    };

    validate_run_transition(from, to).map_err(|err| {
        AppError::conflict(format!(
            "当前 Run 状态不允许流转: {} -> {}",
            run_status_code(err.from),
            run_status_code(err.to)
        ))
    })
}

fn step_type_code(step_type: RunStepType) -> String {
    match step_type {
        RunStepType::ModelCall => "model_call",
        RunStepType::Retrieval => "retrieval",
        RunStepType::Rerank => "rerank",
        RunStepType::ToolCall => "tool_call",
        RunStepType::Approval => "approval",
        RunStepType::HumanInput => "human_input",
        RunStepType::ConnectorSync => "connector_sync",
        RunStepType::MediaJob => "media_job",
    }
    .to_owned()
}

fn event_kind_code(kind: RunEventKind) -> String {
    match kind {
        RunEventKind::InputReceived => "input_received",
        RunEventKind::StatusChanged => "status_changed",
        RunEventKind::IntentRouted => "intent_routed",
        RunEventKind::Thought => "thought",
        RunEventKind::Retrieval => "retrieval",
        RunEventKind::ActionSelected => "action_selected",
        RunEventKind::ApprovalRequested => "approval_requested",
        RunEventKind::Paused => "paused",
        RunEventKind::Resumed => "resumed",
        RunEventKind::ToolCalled => "tool_called",
        RunEventKind::Observation => "observation",
        RunEventKind::FinalOutput => "final_output",
        RunEventKind::CancelRequested => "cancel_requested",
        RunEventKind::Cancelled => "cancelled",
        RunEventKind::Error => "error",
    }
    .to_owned()
}

fn intent_code(intent: AgentIntent) -> String {
    match intent {
        AgentIntent::Chat => "chat",
        AgentIntent::RagQuestion => "rag_question",
        AgentIntent::ToolTask => "tool_task",
        AgentIntent::CodeSearch => "code_search",
        AgentIntent::TrainingQuiz => "training_quiz",
        AgentIntent::HumanHandoff => "human_handoff",
    }
    .to_owned()
}

fn final_output_for_intent(intent: &str) -> String {
    if intent == "training_quiz" {
        return [
            "测验已生成：请根据培训资料回答 5 道题。",
            "1. 客户数据能否复制到个人网盘？",
            "2. 外发客户数据前需要完成什么审批？",
            "3. 新员工第一周应完成哪些安全培训？",
            "4. 发现权限异常或越权访问时应如何处理？",
            "5. 为什么客户数据处理需要保留访问审计？",
        ]
        .join("\n");
    }

    format!("Agent handled {intent} without tool execution.")
}

fn loop_kind_code(loop_kind: AgentLoopKind) -> String {
    match loop_kind {
        AgentLoopKind::ReAct => "react",
        AgentLoopKind::Planner => "planner",
        AgentLoopKind::SupervisorWorker => "supervisor_worker",
    }
    .to_owned()
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_agent_size() -> u64 {
    DEFAULT_AGENT_PAGE_SIZE
}

fn default_event_size() -> u64 {
    DEFAULT_EVENT_PAGE_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ai::model_service::{
        ModelChatUsage, ModelProviderAttempt, ModelProviderStreamChunk, ModelProviderStreamEvent,
    };
    use novex_ai_core::TaskBudget;
    use novex_approval_review::{
        GuardianDecisionSource, GuardianReviewFailureReason, GuardianReviewOutcome,
        GuardianReviewStatus,
    };
    use novex_tools::{
        feishu_message_text_from_tool_input, github_read_request_from_tool_input,
        github_search_request_from_tool_input, media_image_request_from_tool_input,
        ToolBatchExecutionMode,
    };
    use novex_trace::TraceEventKind;
    use sqlx::postgres::PgPoolOptions;

    fn test_prepared_tool_call(
        batch_index: usize,
        call_id: &str,
        tool_code: &str,
    ) -> PreparedAgentToolCall {
        PreparedAgentToolCall {
            batch_index,
            call_id: call_id.to_owned(),
            tool: ToolLookupRecord {
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
            concurrency_policy: Value::Null,
            timeout: AGENT_TOOL_IO_TIMEOUT,
        }
    }

    fn test_capability_record(
        id: i64,
        code: &str,
        name: &str,
        description: &str,
        metadata: Value,
    ) -> crate::infrastructure::persistence::ai_capability_repository::CapabilityRecord {
        crate::infrastructure::persistence::ai_capability_repository::CapabilityRecord {
            id,
            code: code.to_owned(),
            name: name.to_owned(),
            description: description.to_owned(),
            kind: code.to_owned(),
            status: 1,
            risk_level: None,
            metadata,
            create_time: Utc::now().naive_utc(),
        }
    }

    fn test_skill_resource_record(
        skill_id: i64,
        resource_type: &str,
        relative_path: &str,
        content_text: &str,
    ) -> crate::infrastructure::persistence::ai_capability_repository::SkillResourceRecord {
        crate::infrastructure::persistence::ai_capability_repository::SkillResourceRecord {
            id: next_id(),
            skill_id,
            resource_type: resource_type.to_owned(),
            relative_path: relative_path.to_owned(),
            mime_type: "text/markdown".to_owned(),
            content_text: Some(content_text.to_owned()),
            storage_ref: None,
            content_sha256: "test".to_owned(),
            size_bytes: content_text.len() as i64,
            metadata: Value::Null,
        }
    }

    fn test_executed_tool_call(prepared: PreparedAgentToolCall) -> ExecutedAgentToolCall {
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

    fn test_provider_attempt(
        attempt_kind: &str,
        route_id: &str,
        status: &str,
    ) -> ModelProviderAttempt {
        test_provider_attempt_with_error(
            attempt_kind,
            route_id,
            status,
            (status == "failed").then_some("provider_http"),
        )
    }

    fn test_provider_attempt_with_error(
        attempt_kind: &str,
        route_id: &str,
        status: &str,
        error_kind: Option<&str>,
    ) -> ModelProviderAttempt {
        ModelProviderAttempt {
            attempt_kind: attempt_kind.to_owned(),
            route_id: route_id.to_owned(),
            provider: "deep-seek".to_owned(),
            model: Some("deepseek-v4-flash".to_owned()),
            status: status.to_owned(),
            latency_ms: 12,
            error_kind: error_kind.map(str::to_owned),
            http_status: (status == "failed").then_some(502),
            message: error_kind.map(|kind| format!("provider attempt {kind}")),
        }
    }

    fn test_cancel_token() -> (ActiveAgentRunGuard, AgentRunCancellationToken) {
        AgentRuntimeRegistry::default().register_run(1, 1)
    }

    fn test_remote_compaction_request() -> novex_agent_runtime::AgentRemoteCompactionRequest {
        novex_agent_runtime::AgentRemoteCompactionRequest {
            window_id: 1,
            implementation:
                novex_agent_runtime::AgentRemoteCompactionImplementation::ResponsesCompactionV2,
            trigger: novex_agent_runtime::AgentCompactionTrigger::Auto,
            reason: novex_agent_runtime::AgentCompactionReason::ObservationThreshold,
            phase: novex_agent_runtime::AgentCompactionPhase::ModelLoopFollowUp,
            input_history: vec![
                AgentTurnItem::user_message("find refund policy"),
                AgentTurnItem::tool_observation(
                    "call-1",
                    ToolObservationStatus::Succeeded,
                    json!({"text":"refund within 7 days"}),
                ),
            ],
            retained_history: vec![AgentTurnItem::user_message("find refund policy")],
            tool_codes: vec!["rag.search".to_owned()],
            compacted_item_count: 2,
            retained_item_count: 1,
        }
    }

    #[tokio::test]
    async fn agent_service_can_be_bound_to_request_tenant() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let service = AgentService::for_tenant(db, 42);

        assert_eq!(service.tenant_id, 42);
    }

    #[tokio::test]
    async fn agent_runtime_registry_signals_registered_run_cancellation() {
        let registry = AgentRuntimeRegistry::default();
        let (_guard, token) = registry.register_run(42, 1001);

        assert!(!token.is_cancelled());
        assert!(registry.cancel_run(42, 1001));
        token.clone().cancelled().await;
        assert!(token.is_cancelled());
    }

    #[test]
    fn runtime_supervisor_snapshots_active_model_loop_runs() {
        let registry = AgentRuntimeRegistry::default();
        let (_guard, _token) =
            registry.register_run_with_kind(42, 1001, AgentRuntimeTaskKind::ModelLoop);

        let snapshots = registry.active_run_snapshots();

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].tenant_id, 42);
        assert_eq!(snapshots[0].run_id, 1001);
        assert_eq!(snapshots[0].task_kind, AgentRuntimeTaskKind::ModelLoop);
        assert_eq!(snapshots[0].status, AgentRuntimeRunStatus::Running);
        assert!(!snapshots[0].cancel_requested);
    }

    #[test]
    fn runtime_supervisor_guard_unregisters_runtime_snapshot_on_drop() {
        let registry = AgentRuntimeRegistry::default();
        let (guard, _token) = registry.register_run(42, 1001);
        assert_eq!(registry.active_run_snapshots().len(), 1);

        drop(guard);

        assert!(registry.active_run_snapshots().is_empty());
    }

    #[tokio::test]
    async fn runtime_supervisor_cancel_signal_marks_snapshot_cancelling() {
        let registry = AgentRuntimeRegistry::default();
        let (_guard, token) = registry.register_run(42, 1001);

        let signal = registry.cancel_run_signal(42, 1001);

        assert!(signal.sent);
        assert!(signal.active_before_cancel);
        assert_eq!(
            signal.snapshot.unwrap().status,
            AgentRuntimeRunStatus::Cancelling
        );
        token.clone().cancelled().await;
        assert!(token.is_cancelled());
    }

    #[test]
    fn runtime_supervisor_cancel_payload_includes_snapshot() {
        let registry = AgentRuntimeRegistry::default();
        let (_guard, _token) = registry.register_run(42, 1001);
        let signal = registry.cancel_run_signal(42, 1001);

        let payload = runtime_cancelled_event_payload(signal);

        assert_eq!(payload["cancelled"], true);
        assert_eq!(payload["runtimeSignalSent"], true);
        assert_eq!(payload["runtimeSupervisor"]["activeBeforeCancel"], true);
        assert_eq!(payload["runtimeSupervisor"]["taskKind"], "model_loop");
        assert_eq!(payload["runtimeSupervisor"]["status"], "cancelling");
        assert_eq!(payload["runtimeSupervisor"]["cancelRequested"], true);
    }

    #[test]
    fn runtime_supervisor_cancel_run_uses_signal_payload() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("cancel_run_signal"));
        assert!(source.contains("runtime_cancelled_event_payload"));
    }

    #[test]
    fn agent_runtime_registry_is_signalled_by_cancel_run() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("self.agent_runtime.cancel_run"));
    }

    #[test]
    fn agent_runtime_rejects_blank_run_input() {
        let err = normalize_agent_run_command(AgentRunCommand {
            input: "   ".to_owned(),
            runtime_mode: None,
            execution_mode: None,
            model_route_id: None,
            auto_approve: false,
            budget: TaskBudget::default(),
            workbench_context: None,
        })
        .unwrap_err();

        assert!(err.to_string().contains("Agent 输入不能为空"));
    }

    #[test]
    fn agent_run_command_accepts_queued_execution_mode() {
        let command: AgentRunCommand = serde_json::from_value(serde_json::json!({
            "input": "search policy",
            "executionMode": "queued"
        }))
        .unwrap();
        let command = normalize_agent_run_command(command).unwrap();

        assert_eq!(command.execution_mode.as_deref(), Some("queued"));
    }

    #[test]
    fn agent_run_command_defaults_to_inline_execution_mode() {
        let command = normalize_agent_run_command(AgentRunCommand {
            input: "search policy".to_owned(),
            runtime_mode: None,
            execution_mode: None,
            model_route_id: None,
            auto_approve: false,
            budget: TaskBudget::default(),
            workbench_context: None,
        })
        .unwrap();

        assert_eq!(command.execution_mode.as_deref(), Some("inline"));
    }

    #[test]
    fn agent_run_command_rejects_unknown_execution_mode() {
        let err = normalize_agent_run_command(AgentRunCommand {
            input: "search policy".to_owned(),
            runtime_mode: None,
            execution_mode: Some("fire_and_forget".to_owned()),
            model_route_id: None,
            auto_approve: false,
            budget: TaskBudget::default(),
            workbench_context: None,
        })
        .unwrap_err();

        assert!(err.to_string().contains("Agent executionMode 不支持"));
    }

    #[test]
    fn workbench_context_normalization_bounds_lists_and_trims_values() {
        let context = AgentWorkbenchContext {
            mode: " agent ".to_owned(),
            dataset_id: Some(42),
            document_ids: vec![1, 2, 2, 0, -5, 3],
            file_ids: vec![9, 9, 0, 10],
            skill_codes: vec![
                " support.refund ".to_owned(),
                "".to_owned(),
                "support.refund".to_owned(),
                " knowledge.writer ".to_owned(),
            ],
            mcp_tool_codes: (0..24)
                .map(|index| format!(" mcp.docs.search.{index} "))
                .collect(),
            web_search_enabled: true,
            route_id: Some(" runtime.llm.code_agent ".to_owned()),
        };

        let normalized = normalize_agent_workbench_context(Some(context)).expect("context present");

        assert_eq!(normalized.mode, "agent");
        assert_eq!(normalized.dataset_id, Some(42));
        assert_eq!(normalized.document_ids, vec![1, 2, 3]);
        assert_eq!(normalized.file_ids, vec![9, 10]);
        assert_eq!(
            normalized.skill_codes,
            vec!["support.refund".to_owned(), "knowledge.writer".to_owned()]
        );
        assert_eq!(normalized.mcp_tool_codes.len(), 16);
        assert_eq!(normalized.mcp_tool_codes[0], "mcp.docs.search.0");
        assert!(normalized.web_search_enabled);
        assert_eq!(
            normalized.route_id.as_deref(),
            Some("runtime.llm.code_agent")
        );
    }

    #[test]
    fn workbench_context_normalization_drops_empty_context() {
        let context = AgentWorkbenchContext::default();

        assert_eq!(normalize_agent_workbench_context(Some(context)), None);
    }

    #[test]
    fn agent_run_command_payload_preserves_workbench_context() {
        let command = AgentRunCommand {
            input: "What changed in the handbook?".to_owned(),
            runtime_mode: Some("model_loop".to_owned()),
            workbench_context: normalize_agent_workbench_context(Some(AgentWorkbenchContext {
                mode: "agent".to_owned(),
                dataset_id: Some(7),
                document_ids: vec![11],
                file_ids: vec![19],
                skill_codes: vec!["support.refund".to_owned()],
                mcp_tool_codes: vec!["mcp.docs.search".to_owned()],
                web_search_enabled: true,
                route_id: Some("runtime.llm.code_agent".to_owned()),
            })),
            ..AgentRunCommand::default()
        };

        let payload = agent_run_command_payload(&command);

        assert_eq!(payload["workbenchContext"]["mode"], "agent");
        assert_eq!(payload["workbenchContext"]["datasetId"], 7);
        assert_eq!(payload["workbenchContext"]["documentIds"], json!([11]));
        assert_eq!(payload["workbenchContext"]["fileIds"], json!([19]));
        assert_eq!(
            payload["workbenchContext"]["skillCodes"],
            json!(["support.refund"])
        );
        assert_eq!(
            payload["workbenchContext"]["mcpToolCodes"],
            json!(["mcp.docs.search"])
        );
        assert_eq!(payload["workbenchContext"]["webSearchEnabled"], true);
        assert_eq!(
            payload["workbenchContext"]["routeId"],
            "runtime.llm.code_agent"
        );
    }

    #[test]
    fn model_loop_system_prompt_includes_workbench_context_without_user_text_mutation() {
        let context = normalize_agent_workbench_context(Some(AgentWorkbenchContext {
            mode: "agent".to_owned(),
            dataset_id: Some(7),
            document_ids: vec![11, 12],
            file_ids: vec![19],
            skill_codes: vec!["support.refund".to_owned()],
            mcp_tool_codes: vec!["mcp.docs.search".to_owned()],
            web_search_enabled: true,
            route_id: Some("runtime.llm.code_agent".to_owned()),
        }));

        let prompt = build_model_loop_system_prompt_with_context(
            &[
                "rag.search".to_owned(),
                "web.search".to_owned(),
                "mcp.docs.search".to_owned(),
            ],
            context.as_ref(),
            None,
        );

        assert!(prompt.contains("Workbench context:"));
        assert!(prompt.contains("Use rag.search with datasetId 7"));
        assert!(prompt.contains("Selected skill codes: support.refund"));
        assert!(prompt.contains("Selected MCP tool codes: mcp.docs.search"));
        assert!(prompt.contains("Web search is enabled; web.search may be used"));
        assert!(!prompt.contains("What changed in the handbook?"));
    }

    #[test]
    fn model_loop_system_prompt_expands_selected_skill_instructions_and_references() {
        let context = normalize_agent_workbench_context(Some(AgentWorkbenchContext {
            mode: "agent".to_owned(),
            dataset_id: None,
            document_ids: vec![],
            file_ids: vec![],
            skill_codes: vec!["support.refund".to_owned()],
            mcp_tool_codes: vec![],
            web_search_enabled: false,
            route_id: None,
        }));
        let skill_context = "Skill: Refund support (support.refund)\nCheck refund windows before suggesting escalation.\n\nRelevant Skill References:\n[references/policy.md]\nRefunds within 7 days can be self-served.";

        let prompt = build_model_loop_system_prompt_with_context(
            &["rag.search".to_owned()],
            context.as_ref(),
            Some(skill_context),
        );

        assert!(prompt.contains("Loaded skill instructions:"));
        assert!(prompt.contains("Skill: Refund support (support.refund)"));
        assert!(prompt.contains("Check refund windows before suggesting escalation."));
        assert!(prompt.contains("references/policy.md"));
        assert!(prompt.contains("Refunds within 7 days can be self-served."));
    }

    #[test]
    fn agent_skill_context_uses_skill_md_and_relevant_references() {
        let skill = test_capability_record(
            7,
            "support.refund",
            "Refund support",
            "Handle refund requests carefully.",
            json!({
                "promptRules": ["Never promise a refund before checking eligibility."]
            }),
        );
        let resources = vec![
            test_skill_resource_record(
                7,
                "skill_md",
                "SKILL.md",
                "name: support.refund\n---\nAlways ask for the order id first.",
            ),
            test_skill_resource_record(
                7,
                "reference",
                "references/policy.md",
                "Refunds within 7 days can be self-served. Escalate damaged goods.",
            ),
            test_skill_resource_record(
                7,
                "reference",
                "references/unrelated.md",
                "Enterprise invoice setup requires an admin seat.",
            ),
        ];

        let context = agent_skill_context_for_record(
            &skill,
            &resources,
            "Can I refund an order after 3 days?",
        );

        assert!(context.contains("Skill: Refund support (support.refund)"));
        assert!(context.contains("Always ask for the order id first."));
        assert!(context.contains("Never promise a refund before checking eligibility."));
        assert!(context.contains("references/policy.md"));
        assert!(context.contains("Refunds within 7 days"));
        assert!(!context.contains("Enterprise invoice setup"));
    }

    #[test]
    fn model_loop_system_prompt_includes_runtime_current_date() {
        let prompt = build_model_loop_system_prompt_for_date(
            &["web.search".to_owned()],
            NaiveDate::from_ymd_opt(2026, 6, 19).unwrap(),
        );

        assert!(prompt.contains("Current date: 2026-06-19"));
        assert!(prompt.contains("Treat relative dates like today"));
    }

    #[test]
    fn agent_service_queued_run_creation_enqueues_and_returns_queued_status() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("create_queued_run"));
        assert!(source.contains("create_run_records_with_status"));
        assert!(source.contains("RunStatus::Queued"));
        assert!(source.contains("enqueue_agent_run"));
        assert!(source.contains("AgentRunQueueSaveRecord"));
        assert!(source.contains("\"executionMode\""));
        assert!(source.contains("RunEventKind::StatusChanged"));
    }

    #[test]
    fn agent_queue_outbox_service_writes_publish_intents_for_create_and_resume() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        let create_queued_run = &source[source.find("async fn create_queued_run").unwrap()
            ..source.find("pub async fn execute_queued_run").unwrap()];
        let resume_run = &source[source.find("pub async fn resume_run").unwrap()
            ..source.find("pub async fn cancel_run").unwrap()];

        assert!(source.contains("AgentQueueOutboxSaveRecord"));
        assert!(create_queued_run.contains("enqueue_agent_run_with_outbox"));
        assert!(create_queued_run.contains("\"agent.run.queued\""));
        assert!(create_queued_run.contains("\"source\": \"agent.create_run\""));
        assert!(resume_run.contains("requeue_agent_run_for_resume_with_outbox"));
        assert!(resume_run.contains("\"agent.run.resumed\""));
        assert!(resume_run.contains("\"source\": \"agent.resume_run\""));
    }

    #[test]
    fn agent_service_queued_execution_uses_existing_run_contract() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("execute_queued_run"));
        assert!(source.contains("agent_run_command_from_queue_payload"));
        assert!(source.contains("ensure_agent_run_transition(&run.status, RunStatus::Running)"));
        assert!(source.contains("execute_deterministic_plan"));
        assert!(source.contains("create_run_records_with_status"));
    }

    #[test]
    fn agent_queue_cancel_sync_service_updates_unclaimed_queue_rows() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        let cancel_run = &source[source.find("pub async fn cancel_run").unwrap()
            ..source.find("async fn check_model_loop_cancelled").unwrap()];

        assert!(cancel_run.contains("cancel_agent_run_queue_for_run"));
        assert!(cancel_run.contains("self.tenant_id"));
        assert!(cancel_run.contains("run_id"));
        assert!(cancel_run.contains("user_id"));
        assert!(cancel_run.contains("now"));
    }

    #[test]
    fn agent_queue_resume_requeue_service_returns_after_requeueing_resume() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        let resume_run = &source[source.find("pub async fn resume_run").unwrap()
            ..source.find("pub async fn cancel_run").unwrap()];
        let execute_queued_run = &source[source.find("pub async fn execute_queued_run").unwrap()
            ..source.find("async fn create_model_loop_run").unwrap()];

        assert!(source.contains("agent_resume_queue_payload"));
        assert!(source.contains("agent_resume_input_from_queue_payload"));
        assert!(source.contains("execute_resumed_tool_and_finish"));
        assert!(resume_run.contains("requeue_agent_run_for_resume"));
        assert!(resume_run.contains("\"resumeQueued\""));
        assert!(resume_run.contains("return self.get_run(run_id).await"));
        assert!(execute_queued_run.contains("agent_resume_input_from_queue_payload"));
        assert!(execute_queued_run.contains("execute_resumed_tool_and_finish"));
    }

    #[test]
    fn queued_model_loop_uses_existing_run_model_loop_executor() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let normalized_source = source.split_whitespace().collect::<String>();
        let create_queued_run = &source[source.find("async fn create_queued_run").unwrap()
            ..source.find("pub async fn execute_queued_run").unwrap()];

        assert!(!source.contains("Agent queued model_loop execution 暂未支持"));
        assert!(source.contains("execute_model_loop_existing_run"));
        assert!(source.contains("record_model_loop_input_event"));
        assert!(normalized_source
            .contains("execute_model_loop_existing_run(user_id,run_id,command,true)"));
        assert!(normalized_source
            .contains("execute_model_loop_existing_run(user_id,run_id,command,false)"));
        assert!(create_queued_run.contains("plan.loop_kind = \"model_loop\""));
        assert!(create_queued_run.contains("plan.selected_tool_code = None"));
    }

    #[test]
    fn agent_runtime_event_payload_preserves_turn_item_shape() {
        let item = novex_agent_protocol::AgentTurnItem::tool_call(
            "call-1",
            "rag.search",
            serde_json::json!({"query":"policy"}),
        );
        let payload = agent_turn_item_event_payload(&item);

        assert_eq!(payload["item"]["type"], "tool_call");
        assert_eq!(payload["item"]["callId"], "call-1");
        assert_eq!(payload["eventSource"], "novex-agent-runtime");
    }

    #[test]
    fn agent_turn_item_ledger_payload_round_trips_for_replay() {
        let now = Utc::now().naive_utc();
        let item = novex_agent_protocol::AgentTurnItem::tool_call(
            "call-1",
            "rag.search",
            serde_json::json!({"query":"policy"}),
        );
        let payload = agent_turn_item_event_payload(&item);

        let save_record =
            agent_turn_item_save_record_from_event_payload(1, 42, None, 99, 7, &payload, 8, now)
                .expect("turn item payload should produce a ledger record");

        assert_eq!(save_record.tenant_id, 1);
        assert_eq!(save_record.run_id, 42);
        assert_eq!(save_record.source_event_id, 99);
        assert_eq!(save_record.sequence_no, 7);
        assert_eq!(save_record.item_type, "tool_call");
        assert_eq!(save_record.call_id.as_deref(), Some("call-1"));
        assert_eq!(save_record.tool_code.as_deref(), Some("rag.search"));

        let replay_record =
            crate::infrastructure::persistence::ai_agent_repository::AgentTurnItemRecord {
                id: save_record.id,
                run_id: save_record.run_id,
                step_id: save_record.step_id,
                source_event_id: save_record.source_event_id,
                sequence_no: save_record.sequence_no,
                item_type: save_record.item_type,
                call_id: save_record.call_id,
                tool_code: save_record.tool_code,
                item_payload: save_record.item_payload,
                create_time: save_record.now,
            };

        let replayed = agent_turn_item_from_record(replay_record).expect("valid replay item");

        assert_eq!(replayed, item);
    }

    #[test]
    fn agent_service_response_item_ledger_source_contract_wires_append_and_replay() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let append_event = &source[source.find("async fn append_event").unwrap()
            ..source.find("async fn refresh_trace_snapshot").unwrap()];
        let get_run_trace = &source[source.find("pub async fn get_run_trace").unwrap()
            ..source.find("pub async fn resume_run").unwrap()];

        for needle in [
            "agent_turn_item_save_record_from_event_payload",
            "agent_turn_item_from_record",
            "load_model_loop_turn_item_history",
            "turn_items: Vec<AgentTurnItem>",
        ] {
            assert!(
                source.contains(needle),
                "{needle} missing from agent response item ledger service contract"
            );
        }
        assert!(append_event.contains("create_event_with_turn_item"));
        assert!(append_event.contains("agent_turn_item_save_record_from_event_payload"));
        assert!(get_run_trace.contains("load_model_loop_turn_item_history(run_id)"));
        assert!(get_run_trace.contains("turn_items"));
    }

    #[test]
    fn agent_run_command_accepts_model_runtime_mode() {
        let command: AgentRunCommand = serde_json::from_value(serde_json::json!({
            "input": "search policy",
            "runtimeMode": "model_loop"
        }))
        .unwrap();

        assert_eq!(command.runtime_mode.as_deref(), Some("model_loop"));
    }

    #[test]
    fn agent_poc_configured_model_route_source_contract() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let model_loop = &source[source
            .find("async fn execute_model_loop_existing_run")
            .unwrap()
            ..source
                .find("async fn model_loop_context_compaction_outcome")
                .unwrap()];
        let compaction = &source[source
            .find("async fn model_loop_context_compaction_outcome")
            .unwrap()
            ..source.find("pub async fn list_runs").unwrap()];

        assert!(source.contains("model_route_id"));
        assert!(source.contains("\"modelRouteId\""));
        assert!(source.contains("normalize_optional_agent_model_route_id"));
        assert!(source.contains("retry_policy_for_purpose_with_route_id"));
        assert!(model_loop.contains(".retry_policy_for_purpose_with_route_id("));
        assert!(model_loop.contains("command.model_route_id.as_deref()"));
        assert!(model_loop.contains("route_id: command.model_route_id.clone()"));
        assert!(model_loop.contains("command.model_route_id.as_deref()"));
        assert!(compaction.contains("model_route_id: Option<&str>"));
        assert!(compaction.contains("route_id: model_route_id.map(str::to_owned)"));
    }

    #[test]
    fn agent_poc_configured_model_route_command_trims_route_id() {
        let command: AgentRunCommand = serde_json::from_value(serde_json::json!({
            "input": "search policy",
            "runtimeMode": "model_loop",
            "modelRouteId": " runtime.llm.code_agent "
        }))
        .unwrap();
        let command = normalize_agent_run_command(command).unwrap();

        assert_eq!(
            command.model_route_id.as_deref(),
            Some("runtime.llm.code_agent")
        );
    }

    #[test]
    fn agent_poc_configured_model_route_command_rejects_overlong_route_id() {
        let command: AgentRunCommand = serde_json::from_value(serde_json::json!({
            "input": "search policy",
            "runtimeMode": "model_loop",
            "modelRouteId": "x".repeat(129)
        }))
        .unwrap();
        let err = normalize_agent_run_command(command).unwrap_err();

        assert!(err.to_string().contains("模型路由"));
    }

    #[test]
    fn model_loop_prompt_mentions_available_tool_schema() {
        let prompt = build_model_loop_system_prompt(&["rag.search".to_owned()]);

        assert!(prompt.contains("You are Novex Agent Runtime"));
        assert!(prompt.contains("rag.search"));
        assert!(prompt.contains("\"type\":\"tool_call\""));
    }

    #[test]
    fn model_loop_prompt_allows_budget_bounded_multiple_tool_calls() {
        let prompt = build_model_loop_system_prompt(&["rag.search".to_owned()]);

        assert!(prompt.contains("budget"));
        assert!(prompt.contains("observation"));
        assert!(prompt.contains("tool calls"));
        assert!(!prompt.contains("one tool call"));
    }

    #[test]
    fn model_loop_prompt_advertises_tool_call_batches() {
        let prompt = build_model_loop_system_prompt(&[
            "rag.search".to_owned(),
            "github.repo.read".to_owned(),
        ]);

        assert!(prompt.contains("\"type\":\"tool_calls\""));
        assert!(prompt.contains("\"calls\""));
    }

    #[test]
    fn agent_service_model_loop_uses_runtime_state_budget_gate() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("AgentRuntimeState::with_budget"));
        assert!(source.contains("runtime_state.can_execute_tool_call()"));
        assert!(source.contains("runtime_state.push_item"));
    }

    #[test]
    fn agent_service_model_loop_records_budget_stop_when_tool_call_budget_exhausted() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("tool_call_budget_exhausted"));
        assert!(source.contains("RunStatus::Failed"));
        assert!(source.contains("Tool call budget exhausted"));
    }

    #[test]
    fn agent_service_model_loop_uses_novex_tool_router() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("ToolRouter::from_definitions"));
        assert!(source.contains("agent_model_loop_tool_definitions"));
        assert!(source.contains("tool_router.route_tool_call"));
    }

    #[test]
    fn agent_tool_executor_registry_boundary_lives_in_novex_tools() {
        let executor_source = include_str!("../../../../crates/novex-tools/src/executor.rs");
        let definitions_source = include_str!("../../../../crates/novex-tools/src/definitions.rs");
        let backend_source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(executor_source.contains("pub struct ToolExecutorRegistry"));
        assert!(definitions_source.contains("pub fn agent_model_loop_tool_executor_bindings"));
        assert!(executor_source.contains("ToolExecutorRegistryErrorKind::MissingExecutor"));
        assert!(!backend_source.contains("struct ToolExecutorRegistry"));
    }

    #[test]
    fn agent_service_model_loop_records_tool_concurrency_policy() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("\"concurrencyPolicy\""));
        assert!(source.contains("serde_json::to_value(&routed_call.tool.concurrency"));
    }

    #[test]
    fn agent_service_model_loop_plans_parsed_tool_call_batches() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("parsed.items"));
        assert!(source.contains("ToolBatchPlan::from_routed_calls"));
        assert!(source.contains("\"batchExecutionMode\""));
        assert!(source.contains("\"toolCallBatch\""));
    }

    #[tokio::test]
    async fn parallel_tool_io_batch_polls_calls_concurrently_and_preserves_order() {
        use std::sync::Arc;
        use tokio::sync::Barrier;

        let barrier = Arc::new(Barrier::new(2));
        let calls = vec![
            test_prepared_tool_call(0, "call-1", "rag.search"),
            test_prepared_tool_call(1, "call-2", "github.repo.read"),
        ];
        let (_guard, cancel_token) = test_cancel_token();

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(250),
            execute_agent_tool_io_batch(ToolBatchExecutionMode::Parallel, calls, cancel_token, {
                let barrier = barrier.clone();
                move |prepared| {
                    let barrier = barrier.clone();
                    async move {
                        barrier.wait().await;
                        Ok(test_executed_tool_call(prepared))
                    }
                }
            }),
        )
        .await
        .expect("parallel execution should not deadlock")
        .unwrap();

        assert_eq!(result[0].prepared.call_id, "call-1");
        assert_eq!(result[1].prepared.call_id, "call-2");
        let metrics = result[0].tool_io_metrics.as_ref().unwrap();
        assert_eq!(metrics.execution_mode, ToolBatchExecutionMode::Parallel);
        assert_eq!(metrics.task_runtime, "tokio_task");
        assert_eq!(metrics.supervisor, "agent_tool_io_task_supervisor");
    }

    #[tokio::test]
    async fn serial_tool_io_batch_runs_calls_in_sequence() {
        use std::sync::{Arc, Mutex};

        let order = Arc::new(Mutex::new(Vec::new()));
        let calls = vec![
            test_prepared_tool_call(0, "call-1", "media.image.generate"),
            test_prepared_tool_call(1, "call-2", "feishu.message.send"),
        ];
        let (_guard, cancel_token) = test_cancel_token();

        let result =
            execute_agent_tool_io_batch(ToolBatchExecutionMode::Serial, calls, cancel_token, {
                let order = order.clone();
                move |prepared| {
                    let order = order.clone();
                    async move {
                        order.lock().unwrap().push(prepared.call_id.clone());
                        Ok(test_executed_tool_call(prepared))
                    }
                }
            })
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(
            *order.lock().unwrap(),
            vec!["call-1".to_owned(), "call-2".to_owned()]
        );
        let metrics = result[0].tool_io_metrics.as_ref().unwrap();
        assert_eq!(metrics.execution_mode, ToolBatchExecutionMode::Serial);
        assert_eq!(metrics.task_runtime, "inline");
    }

    #[tokio::test]
    async fn tool_io_timeout_returns_cancelled_execution() {
        let mut call = test_prepared_tool_call(0, "call-1", "rag.search");
        call.timeout = std::time::Duration::from_millis(10);
        let calls = vec![call];
        let (_guard, cancel_token) = test_cancel_token();

        let result = execute_agent_tool_io_batch(
            ToolBatchExecutionMode::Serial,
            calls,
            cancel_token,
            |prepared| async move {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                Ok(test_executed_tool_call(prepared))
            },
        )
        .await
        .unwrap();

        assert_eq!(result[0].execution.status, "cancelled");
        assert_eq!(result[0].terminal_status, RunStatus::Cancelled);
        assert_eq!(
            result[0].execution.response_payload["cancelReason"],
            "tool_io_timeout"
        );
        let metrics = result[0].tool_io_metrics.as_ref().unwrap();
        assert_eq!(metrics.execution_mode, ToolBatchExecutionMode::Serial);
        assert_eq!(metrics.terminal_status, RunStatus::Cancelled);
        assert_eq!(metrics.cancel_reason.as_deref(), Some("tool_io_timeout"));
    }

    #[test]
    fn cancelled_tool_execution_maps_to_cancelled_observation_status() {
        let execution = AgentToolExecution::cancelled(
            serde_json::json!({"cancelReason":"tool_io_timeout"}),
            "timeout".to_owned(),
        );

        assert_eq!(
            tool_observation_status_for_execution(&execution),
            ToolObservationStatus::Cancelled
        );
    }

    #[test]
    fn agent_service_model_loop_maps_cancelled_tool_observations() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("tool_observation_status_for_execution"));
    }

    #[test]
    fn agent_service_model_loop_records_tool_timeout_cancel_reason() {
        let source = include_str!("agent_tool_io_runtime.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("\"cancelReason\""));
        assert!(source.contains("tool_io_timeout"));
    }

    #[test]
    fn agent_service_model_loop_checks_external_cancel_before_model_call() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("before_model_call"));
        assert!(source.contains("after_model_call"));
        assert!(source.contains("check_model_loop_cancelled"));
    }

    #[test]
    fn agent_service_model_loop_records_external_cancel_reason() {
        let payload = model_loop_external_cancel_payload("before_model_call");

        assert_eq!(payload["cancelled"], true);
        assert_eq!(payload["cancelReason"], "external_cancel");
        assert_eq!(payload["cancelStage"], "before_model_call");
        assert_eq!(payload["runtimeMode"], "model_loop");
    }

    #[test]
    fn agent_service_model_loop_checks_external_cancel_around_tool_batches() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("before_tool_batch"));
        assert!(source.contains("after_tool_batch"));
        assert!(source.contains("before_next_turn"));
    }

    #[test]
    fn agent_service_model_loop_awaits_model_with_runtime_registry_token() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("await_model_loop_future_or_cancelled"));
        assert!(source.contains("model_call"));
    }

    #[test]
    fn provider_abort_source_contract_polls_persistent_run_status() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("MODEL_LOOP_PERSISTENT_CANCEL_POLL_INTERVAL"));
        assert!(source.contains("wait_for_model_loop_persistent_cancel"));
        assert!(source.contains("self.repo.find_run(self.tenant_id, run_id)"));
        assert!(source.contains("model_loop_cancel_requested(&run.status)"));
    }

    #[test]
    fn provider_abort_source_contract_wraps_model_and_compaction_calls() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let model_loop = &source[source
            .find("async fn execute_model_loop_existing_run")
            .unwrap()
            ..source
                .find("async fn model_loop_context_compaction_outcome")
                .unwrap()];
        let compaction = &source[source
            .find("async fn model_loop_context_compaction_outcome")
            .unwrap()
            ..source.find("pub async fn list_runs").unwrap()];

        assert!(model_loop.contains("await_model_loop_stream_call_or_cancelled_with_delta_events"));
        assert!(model_loop.contains("wait_for_model_loop_persistent_cancel"));
        assert!(compaction.contains("await_model_loop_provider_future_or_cancelled"));
        assert!(compaction.contains("wait_for_model_loop_persistent_cancel"));
    }

    #[test]
    fn agent_service_model_loop_records_model_inference_spans() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let model_loop = &source[source
            .find("async fn execute_model_loop_existing_run")
            .unwrap()
            ..source
                .find("async fn model_loop_context_compaction_outcome")
                .unwrap()];

        assert!(model_loop.contains("if let Some(model_response) = model_response.as_ref()"));
        assert!(model_loop.contains("model_inference_event_payload(model_response)"));
        assert!(source.contains("\"type\": \"model_inference\""));
        assert!(source.contains("\"latencyMs\""));
        assert!(source.contains("\"usage\""));
    }

    #[test]
    fn agent_provider_call_lease_context_contract_links_run_and_source() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let model_loop = &source[source
            .find("async fn execute_model_loop_existing_run")
            .unwrap()
            ..source
                .find("async fn model_loop_context_compaction_outcome")
                .unwrap()];
        let compaction = &source[source
            .find("async fn model_loop_context_compaction_outcome")
            .unwrap()
            ..source.find("pub async fn list_runs").unwrap()];

        assert!(source.contains("ModelProviderCallContext"));
        assert!(model_loop.contains("provider_call_context: Some(ModelProviderCallContext"));
        assert!(model_loop.contains("run_id: Some(run_id)"));
        assert!(model_loop.contains("\"agent.model_loop\""));
        assert!(compaction.contains("provider_call_context: Some(ModelProviderCallContext"));
        assert!(compaction.contains("run_id: Some(run_id)"));
        assert!(compaction.contains("\"agent.context_compaction\""));
    }

    #[test]
    fn model_inference_cost_event_payload_preserves_response_cost() {
        let response = ModelChatResp {
            conversation_id: None,
            answer: "ok".to_owned(),
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "deep-seek".to_owned(),
            model: Some("deepseek-v4-flash".to_owned()),
            latency_ms: 42,
            usage: ModelChatUsage {
                prompt_tokens: Some(11),
                completion_tokens: Some(7),
                total_tokens: Some(18),
            },
            cost_cents: Some(0.65),
            provider_attempts: vec![],
            provider_call_lease_id: None,
            provider_response_id: None,
            provider_response_status: None,
            provider_delta_chunks: vec![],
        };

        let payload = model_inference_event_payload(&response);

        assert_eq!(payload["item"]["costCents"], 0.65);
    }

    #[test]
    fn model_inference_event_payload_links_provider_call_lease() {
        let response = ModelChatResp {
            conversation_id: None,
            answer: "ok".to_owned(),
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "deep-seek".to_owned(),
            model: Some("deepseek-v4-flash".to_owned()),
            latency_ms: 42,
            usage: ModelChatUsage::default(),
            cost_cents: None,
            provider_attempts: vec![],
            provider_call_lease_id: Some(123),
            provider_response_id: None,
            provider_response_status: None,
            provider_delta_chunks: vec![],
        };

        let payload = model_inference_event_payload(&response);

        assert_eq!(payload["item"]["providerCallLeaseId"], 123);
    }

    #[test]
    fn model_delta_inference_payload_marks_streaming_metadata() {
        let response = ModelChatResp {
            conversation_id: None,
            answer: "Hello world".to_owned(),
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: Some("gpt-compatible".to_owned()),
            latency_ms: 42,
            usage: ModelChatUsage::default(),
            cost_cents: None,
            provider_attempts: vec![],
            provider_call_lease_id: None,
            provider_response_id: None,
            provider_response_status: None,
            provider_delta_chunks: vec![
                ModelProviderStreamChunk {
                    index: 0,
                    content: "Hello".to_owned(),
                    provider_event: Some("chat.completion.chunk".to_owned()),
                },
                ModelProviderStreamChunk {
                    index: 1,
                    content: " world".to_owned(),
                    provider_event: Some("chat.completion.chunk".to_owned()),
                },
            ],
        };

        let payload = model_inference_event_payload(&response);

        assert_eq!(payload["item"]["streaming"], true);
        assert_eq!(payload["item"]["deltaChunkCount"], 2);
        assert_eq!(payload["item"]["deltaTextLength"], 11);
        assert!(payload["item"].get("answer").is_none());
    }

    #[test]
    fn model_delta_event_payload_preserves_chunk_contract() {
        let response = ModelChatResp {
            conversation_id: None,
            answer: "Hello world".to_owned(),
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: Some("gpt-compatible".to_owned()),
            latency_ms: 42,
            usage: ModelChatUsage::default(),
            cost_cents: None,
            provider_attempts: vec![],
            provider_call_lease_id: None,
            provider_response_id: None,
            provider_response_status: None,
            provider_delta_chunks: vec![],
        };
        let chunk = ModelProviderStreamChunk {
            index: 3,
            content: " partial".to_owned(),
            provider_event: Some("chat.completion.chunk".to_owned()),
        };

        let payload = model_delta_event_payload(&response, &chunk);

        assert_eq!(payload["runtimeMode"], "model_loop");
        assert_eq!(payload["item"]["type"], "model_delta");
        assert_eq!(payload["item"]["source"], "provider_stream");
        assert_eq!(payload["item"]["routeId"], "runtime.llm.code_agent");
        assert_eq!(payload["item"]["provider"], "openai-compatible");
        assert_eq!(payload["item"]["model"], "gpt-compatible");
        assert_eq!(payload["item"]["deltaIndex"], 3);
        assert_eq!(payload["item"]["content"], " partial");
        assert_eq!(payload["item"]["providerEvent"], "chat.completion.chunk");
    }

    #[test]
    fn provider_stream_response_id_is_added_to_model_delta_payload() {
        let event = ModelProviderStreamEvent {
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: Some("gpt-compatible".to_owned()),
            provider_call_lease_id: Some(4242),
            provider_response_id: Some("resp_stream_1".to_owned()),
            provider_response_status: Some("in_progress".to_owned()),
            chunk: ModelProviderStreamChunk {
                index: 3,
                content: " partial".to_owned(),
                provider_event: Some("response.output_text.delta".to_owned()),
            },
        };

        let payload = model_delta_event_payload_from_stream_event(&event);

        assert_eq!(payload["item"]["providerResponseId"], "resp_stream_1");
        assert_eq!(payload["item"]["providerResponseStatus"], "in_progress");
        assert_eq!(payload["item"]["content"], " partial");
    }

    #[test]
    fn provider_stream_lease_id_is_added_to_model_delta_payload() {
        let event = ModelProviderStreamEvent {
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: Some("gpt-compatible".to_owned()),
            provider_call_lease_id: Some(4242),
            provider_response_id: Some("resp_stream_1".to_owned()),
            provider_response_status: Some("in_progress".to_owned()),
            chunk: ModelProviderStreamChunk {
                index: 3,
                content: " partial".to_owned(),
                provider_event: Some("response.output_text.delta".to_owned()),
            },
        };

        let payload = model_delta_event_payload_from_stream_event(&event);

        assert_eq!(payload["item"]["providerCallLeaseId"], 4242);
        assert_eq!(payload["item"]["providerResponseId"], "resp_stream_1");
    }

    #[test]
    fn provider_stream_tool_call_state_detects_complete_json_across_chunks() {
        let mut state = ModelLoopProviderStreamState::new();
        let first = ModelProviderStreamEvent {
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: Some("gpt-compatible".to_owned()),
            provider_call_lease_id: Some(4242),
            provider_response_id: None,
            provider_response_status: None,
            chunk: ModelProviderStreamChunk {
                index: 0,
                content: r#"{"type":"tool_"#.to_owned(),
                provider_event: Some("response.output_text.delta".to_owned()),
            },
        };
        let second = ModelProviderStreamEvent {
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: Some("gpt-compatible".to_owned()),
            provider_call_lease_id: Some(4242),
            provider_response_id: None,
            provider_response_status: None,
            chunk: ModelProviderStreamChunk {
                index: 1,
                content: r#"call","callId":"call-1","toolCode":"rag.search","arguments":{"query":"policy"}}"#.to_owned(),
                provider_event: Some("response.output_text.delta".to_owned()),
            },
        };

        assert!(state.observe_tool_call(&first).is_none());
        let payload = state
            .observe_tool_call(&second)
            .expect("complete streamed tool call should be detected");

        assert_eq!(payload["runtimeMode"], "model_loop");
        assert_eq!(payload["item"]["type"], "model_stream_tool_call");
        assert_eq!(payload["item"]["source"], "provider_stream");
        assert_eq!(payload["item"]["routeId"], "runtime.llm.code_agent");
        assert_eq!(payload["item"]["provider"], "openai-compatible");
        assert_eq!(payload["item"]["model"], "gpt-compatible");
        assert_eq!(payload["item"]["deltaIndex"], 1);
        assert_eq!(payload["item"]["toolCallCount"], 1);
        assert_eq!(payload["item"]["toolCalls"][0]["callId"], "call-1");
        assert_eq!(payload["item"]["toolCalls"][0]["toolCode"], "rag.search");
        assert_eq!(
            payload["item"]["toolCalls"][0]["arguments"]["query"],
            "policy"
        );
        assert!(state.observe_tool_call(&second).is_none());
    }

    #[test]
    fn provider_stream_response_id_is_added_to_streamed_tool_call_payload() {
        let mut state = ModelLoopProviderStreamState::new();
        let first = ModelProviderStreamEvent {
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: Some("gpt-compatible".to_owned()),
            provider_call_lease_id: Some(4242),
            provider_response_id: Some("resp_stream_1".to_owned()),
            provider_response_status: Some("in_progress".to_owned()),
            chunk: ModelProviderStreamChunk {
                index: 0,
                content: r#"{"type":"tool_"#.to_owned(),
                provider_event: Some("response.output_text.delta".to_owned()),
            },
        };
        let second = ModelProviderStreamEvent {
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: Some("gpt-compatible".to_owned()),
            provider_call_lease_id: None,
            provider_response_id: Some("resp_stream_1".to_owned()),
            provider_response_status: Some("in_progress".to_owned()),
            chunk: ModelProviderStreamChunk {
                index: 1,
                content: r#"call","callId":"call-1","toolCode":"rag.search","arguments":{"query":"policy"}}"#.to_owned(),
                provider_event: Some("response.output_text.delta".to_owned()),
            },
        };

        assert!(state.observe_tool_call(&first).is_none());
        let payload = state
            .observe_tool_call(&second)
            .expect("complete streamed tool call should be detected");

        assert_eq!(payload["item"]["providerResponseId"], "resp_stream_1");
        assert_eq!(payload["item"]["providerResponseStatus"], "in_progress");
        assert_eq!(payload["item"]["toolCalls"][0]["toolCode"], "rag.search");
    }

    #[test]
    fn streamed_tool_call_output_is_retained_for_model_loop_decision() {
        let mut state = ModelLoopProviderStreamState::new();
        let first = ModelProviderStreamEvent {
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: Some("gpt-compatible".to_owned()),
            provider_call_lease_id: None,
            provider_response_id: None,
            provider_response_status: None,
            chunk: ModelProviderStreamChunk {
                index: 0,
                content: r#"{"type":"tool_"#.to_owned(),
                provider_event: Some("response.output_text.delta".to_owned()),
            },
        };
        let second = ModelProviderStreamEvent {
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: Some("gpt-compatible".to_owned()),
            provider_call_lease_id: None,
            provider_response_id: None,
            provider_response_status: None,
            chunk: ModelProviderStreamChunk {
                index: 1,
                content: r#"call","callId":"call-1","toolCode":"rag.search","arguments":{"query":"policy"}}"#.to_owned(),
                provider_event: Some("response.output_text.delta".to_owned()),
            },
        };

        assert!(state.observe_tool_call(&first).is_none());
        assert!(state.observe_tool_call(&second).is_some());
        let streamed = state
            .detected_tool_call_output()
            .expect("streamed parsed tool call should be retained");
        let expected = AgentTurnItem::tool_call("call-1", "rag.search", json!({"query":"policy"}));

        assert_eq!(streamed.item, expected);
        assert_eq!(streamed.items, vec![expected]);
        assert_eq!(
            streamed.outcome,
            novex_agent_protocol::TurnOutcome::NeedsFollowUp
        );
    }

    #[test]
    fn agent_model_stream_native_runtime_api_uses_runtime_facade() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let model_loop = &source[source
            .find("async fn execute_model_loop_existing_run")
            .unwrap()
            ..source
                .find("async fn model_loop_context_compaction_outcome")
                .unwrap()];

        assert!(model_loop.contains("chat_completion_stream_for_purpose"));
        assert!(!model_loop.contains("provider_stream_channel"));
        assert!(!model_loop.contains("provider_stream_sender: Some"));
        assert!(model_loop.contains("await_model_loop_stream_call_or_cancelled_with_delta_events"));
        assert!(source.contains("drain_model_delta_events"));
        assert!(source.contains("model_delta_event_payload"));
    }

    #[test]
    fn agent_model_stream_call_lifecycle_owns_transport_and_events() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let model_loop = &source[source
            .find("async fn execute_model_loop_existing_run")
            .unwrap()
            ..source
                .find("async fn model_loop_context_compaction_outcome")
                .unwrap()];
        let provider_wait = &source[source
            .find("async fn await_model_loop_stream_call_or_cancelled_with_delta_events")
            .unwrap()
            ..source.find("async fn drain_model_delta_events").unwrap()];
        let normalized_model_loop = model_loop.split_whitespace().collect::<Vec<_>>().join(" ");
        let normalized_provider_wait = provider_wait
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        assert!(source.contains("ModelChatStreamCall"));
        assert!(source.contains("await_model_loop_stream_call_or_cancelled_with_delta_events"));
        assert!(normalized_model_loop.contains("run_id, model_stream_call, ) .await"));
        assert!(!model_loop.contains("model_stream_call.events"));
        assert!(!model_loop.contains("model_stream_call.response"));
        assert!(normalized_provider_wait.contains("model_stream_call: ModelChatStreamCall"));
    }

    #[test]
    fn agent_model_stream_transport_task_waits_without_boxed_future() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let provider_wait = &source[source
            .find("async fn await_model_loop_stream_call_or_cancelled_with_delta_events")
            .unwrap()
            ..source.find("async fn drain_model_delta_events").unwrap()];

        assert!(provider_wait.contains("transport,"));
        assert!(provider_wait.contains("transport.wait()"));
        assert!(!provider_wait.contains("let future = response"));
    }

    #[test]
    fn streamed_tool_call_decision_prefers_streamed_parse_over_final_text() {
        let expected = AgentTurnItem::tool_call("call-1", "rag.search", json!({"query":"policy"}));
        let streamed = ParsedModelTurnOutput {
            item: expected.clone(),
            items: vec![expected.clone()],
            outcome: novex_agent_protocol::TurnOutcome::NeedsFollowUp,
        };
        let response = ModelChatResp {
            conversation_id: None,
            answer: "This final text would otherwise parse as a final answer.".to_owned(),
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: Some("gpt-compatible".to_owned()),
            latency_ms: 42,
            usage: ModelChatUsage::default(),
            cost_cents: None,
            provider_attempts: vec![],
            provider_call_lease_id: None,
            provider_response_id: None,
            provider_response_status: None,
            provider_delta_chunks: vec![],
        };

        let parsed = model_loop_parse_turn_output(Some(&response), Some(&streamed)).unwrap();

        assert_eq!(parsed.item, expected);
        assert_eq!(parsed.items, vec![expected]);
        assert_eq!(
            parsed.outcome,
            novex_agent_protocol::TurnOutcome::NeedsFollowUp
        );
    }

    #[test]
    fn streamed_tool_call_decision_model_loop_uses_provider_completion_contract() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let model_loop = &source[source
            .find("async fn execute_model_loop_existing_run")
            .unwrap()
            ..source
                .find("async fn model_loop_context_compaction_outcome")
                .unwrap()];
        let normalized_model_loop = model_loop.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(source.contains("struct ModelLoopProviderCompletion"));
        assert!(model_loop.contains("completion.streamed_tool_call_output"));
        assert!(normalized_model_loop.contains(
            "model_loop_parse_turn_output( model_response.as_ref(), streamed_tool_call_output.as_ref(), )"
        ));
    }

    #[test]
    fn streamed_tool_call_early_stop_completion_contract_is_explicit() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("enum ModelLoopProviderCompletionReason"));
        assert!(source.contains("ProviderCompleted"));
        assert!(source.contains("StreamedToolCallDetected"));
        assert!(source.contains("response: Option<T>"));
        assert!(source.contains("completion_reason: ModelLoopProviderCompletionReason"));
    }

    #[test]
    fn streamed_tool_call_early_stop_returns_completion_without_provider_response() {
        let mut state = ModelLoopProviderStreamState::new();
        let first = ModelProviderStreamEvent {
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: Some("gpt-compatible".to_owned()),
            provider_call_lease_id: Some(4242),
            provider_response_id: Some("resp_stream_1".to_owned()),
            provider_response_status: Some("in_progress".to_owned()),
            chunk: ModelProviderStreamChunk {
                index: 0,
                content: r#"{"type":"tool_"#.to_owned(),
                provider_event: Some("response.output_text.delta".to_owned()),
            },
        };
        let second = ModelProviderStreamEvent {
            route_id: "runtime.llm.code_agent".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: Some("gpt-compatible".to_owned()),
            provider_call_lease_id: Some(4242),
            provider_response_id: Some("resp_stream_1".to_owned()),
            provider_response_status: Some("in_progress".to_owned()),
            chunk: ModelProviderStreamChunk {
                index: 1,
                content: r#"call","callId":"call-1","toolCode":"rag.search","arguments":{"query":"policy"}}"#.to_owned(),
                provider_event: Some("response.output_text.delta".to_owned()),
            },
        };

        assert!(state.observe_tool_call(&first).is_none());
        assert!(state.observe_tool_call(&second).is_some());

        let completion = model_loop_streamed_tool_call_completion::<ModelChatResp>(&state)
            .expect("streamed tool call should produce early-stop completion");
        let expected = AgentTurnItem::tool_call("call-1", "rag.search", json!({"query":"policy"}));

        assert!(completion.response.is_none());
        assert_eq!(
            completion.completion_reason,
            ModelLoopProviderCompletionReason::StreamedToolCallDetected
        );
        assert_eq!(completion.provider_call_lease_id, Some(4242));
        assert_eq!(
            completion.provider_response_id.as_deref(),
            Some("resp_stream_1")
        );
        assert_eq!(
            completion.provider_response_status.as_deref(),
            Some("in_progress")
        );
        assert_eq!(
            completion
                .streamed_tool_call_output
                .expect("completion should retain streamed parsed output")
                .item,
            expected
        );
    }

    #[test]
    fn provider_stream_native_cancel_source_contract_dispatches_after_early_stop() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let model_loop = &source[source
            .find("async fn execute_model_loop_existing_run")
            .unwrap()
            ..source
                .find("async fn model_loop_context_compaction_outcome")
                .unwrap()];

        assert!(model_loop.contains("try_cancel_streamed_provider_call("));
        assert!(source.contains("async fn try_cancel_streamed_provider_call"));
        assert!(source.contains("cancel_provider_call_lease_with_response_metadata"));
        assert!(source.contains("provider_native_cancel_event_payload"));
        assert!(source.contains("provider_native_cancel_error_event_payload"));
    }

    #[test]
    fn streamed_tool_call_early_stop_parse_accepts_missing_provider_response() {
        let expected = AgentTurnItem::tool_call("call-1", "rag.search", json!({"query":"policy"}));
        let streamed = ParsedModelTurnOutput {
            item: expected.clone(),
            items: vec![expected.clone()],
            outcome: novex_agent_protocol::TurnOutcome::NeedsFollowUp,
        };

        let parsed = model_loop_parse_turn_output(None, Some(&streamed)).unwrap();

        assert_eq!(parsed.item, expected);
        assert_eq!(parsed.items, vec![expected]);
        assert_eq!(
            parsed.outcome,
            novex_agent_protocol::TurnOutcome::NeedsFollowUp
        );
    }

    #[test]
    fn streamed_tool_call_early_stop_model_loop_uses_optional_response_contract() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let provider_wait = &source[source
            .find("async fn await_model_loop_stream_call_or_cancelled_with_delta_events")
            .unwrap()
            ..source.find("async fn drain_model_delta_events").unwrap()];
        let model_loop = &source[source
            .find("async fn execute_model_loop_existing_run")
            .unwrap()
            ..source
                .find("async fn model_loop_context_compaction_outcome")
                .unwrap()];
        let normalized_provider_wait = provider_wait
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let normalized_model_loop = model_loop.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(provider_wait.contains("model_loop_streamed_tool_call_completion::<ModelChatResp>"));
        assert!(normalized_provider_wait
            .contains("completion_reason: ModelLoopProviderCompletionReason::ProviderCompleted"));
        assert!(model_loop.contains("model_response = completion.response"));
        assert!(model_loop.contains("if let Some(model_response) = model_response.as_ref()"));
        assert!(normalized_model_loop.contains(
            "model_loop_parse_turn_output( model_response.as_ref(), streamed_tool_call_output.as_ref(), )"
        ));
    }

    #[test]
    fn provider_lifecycle_trace_payload_exposes_fallback_attempts() {
        let response = ModelChatResp {
            conversation_id: None,
            answer: "ok".to_owned(),
            route_id: "runtime.llm.backup".to_owned(),
            provider: "deep-seek".to_owned(),
            model: Some("deepseek-v4-flash".to_owned()),
            latency_ms: 20,
            usage: ModelChatUsage::default(),
            cost_cents: None,
            provider_attempts: vec![
                test_provider_attempt("primary", "runtime.llm", "failed"),
                test_provider_attempt("fallback", "runtime.llm.backup", "succeeded"),
            ],
            provider_call_lease_id: None,
            provider_response_id: None,
            provider_response_status: None,
            provider_delta_chunks: vec![],
        };

        let payload = model_inference_event_payload(&response);

        assert_eq!(payload["item"]["fallbackUsed"], true);
        assert_eq!(payload["item"]["fallbackRouteId"], "runtime.llm.backup");
        assert_eq!(
            payload["item"]["providerAttempts"].as_array().map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn route_circuit_breaker_trace_payload_marks_circuit_open_attempts() {
        let response = ModelChatResp {
            conversation_id: None,
            answer: "ok".to_owned(),
            route_id: "runtime.llm.backup".to_owned(),
            provider: "deep-seek".to_owned(),
            model: Some("deepseek-v4-flash".to_owned()),
            latency_ms: 20,
            usage: ModelChatUsage::default(),
            cost_cents: None,
            provider_attempts: vec![
                test_provider_attempt_with_error(
                    "primary",
                    "runtime.llm",
                    "skipped",
                    Some("circuit_open"),
                ),
                test_provider_attempt("fallback", "runtime.llm.backup", "succeeded"),
            ],
            provider_call_lease_id: None,
            provider_response_id: None,
            provider_response_status: None,
            provider_delta_chunks: vec![],
        };

        let payload = model_inference_event_payload(&response);

        assert_eq!(payload["item"]["circuitOpen"], true);
        assert_eq!(
            payload["item"]["providerAttempts"][0]["errorKind"],
            "circuit_open"
        );
    }

    #[test]
    fn model_inference_error_event_payload_classifies_retryable_http_errors() {
        let payload = model_inference_error_event_payload(
            &AppError::bad_request("LLM 模型调用失败: HTTP 502"),
            12,
        );

        assert_eq!(payload["item"]["type"], "model_inference_error");
        assert_eq!(payload["item"]["routeId"], "runtime.llm.code_agent");
        assert_eq!(payload["item"]["errorKind"], "provider_http");
        assert_eq!(payload["item"]["httpStatus"], 502);
        assert_eq!(payload["item"]["retryable"], true);
        assert_eq!(payload["item"]["latencyMs"], 12);
    }

    #[test]
    fn model_inference_error_provider_retry_payload_marks_attempts() {
        let payload = model_inference_error_attempt_event_payload(
            &AppError::bad_request("LLM 模型调用失败: HTTP 429"),
            12,
            2,
            3,
            true,
        );

        assert_eq!(payload["item"]["type"], "model_inference_error");
        assert_eq!(payload["item"]["attempt"], 2);
        assert_eq!(payload["item"]["maxAttempts"], 3);
        assert_eq!(payload["item"]["willRetry"], true);
        assert_eq!(payload["item"]["retryable"], true);
        assert_eq!(payload["item"]["httpStatus"], 429);
    }

    #[test]
    fn agent_service_model_loop_records_provider_error_spans() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let normalized_source = source.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(normalized_source
            .contains("let error_payload = model_inference_error_attempt_event_payload( &err,"));
        assert!(source.contains("RunEventKind::Error"));
        assert!(source.contains("\"model_inference_error\""));
        assert!(source.contains("\"stopReason\": \"model_call_failed\""));
    }

    #[test]
    fn agent_service_model_loop_provider_retry_retries_retryable_errors() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("retry_policy_for_purpose_with_route_id("));
        assert!(source.contains("command.model_route_id.as_deref()"));
        assert!(source.contains("for attempt in 1..=model_retry_policy.max_attempts()"));
        assert!(source.contains("will_retry"));
        assert!(source.contains("model_inference_error_attempt_event_payload"));
    }

    #[test]
    fn agent_service_tool_io_awaits_runtime_registry_cancel_token() {
        let service_source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let runtime_source = include_str!("agent_tool_io_runtime.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(service_source.contains("execute_agent_tool_io_batch"));
        assert!(service_source.contains("cancel_token.clone()"));
        assert!(runtime_source.contains("execute_agent_tool_io_with_timeout_and_cancel"));
        assert!(runtime_source.contains("\"cancelReason\": \"external_cancel\""));
    }

    #[tokio::test]
    async fn model_loop_future_runtime_registry_cancel_returns_cancelled_await() {
        let registry = AgentRuntimeRegistry::default();
        let (_guard, token) = registry.register_run(1, 42);
        assert!(registry.cancel_run(1, 42));

        let result = await_model_loop_future_or_cancelled(token, "model_call", async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            Ok::<_, AppError>("finished")
        })
        .await
        .unwrap();

        assert_eq!(result, ModelLoopFutureAwait::Cancelled);
    }

    #[tokio::test]
    async fn provider_abort_persistent_cancel_returns_cancelled_before_provider_future() {
        let registry = AgentRuntimeRegistry::default();
        let (_guard, token) = registry.register_run(1, 42);

        let result = await_model_loop_provider_future_or_cancelled(
            token,
            async { Ok::<_, AppError>(()) },
            "model_call",
            std::future::pending::<Result<&'static str, AppError>>(),
        )
        .await
        .unwrap();

        assert_eq!(result, ModelLoopFutureAwait::Cancelled);
    }

    #[tokio::test]
    async fn provider_abort_local_token_returns_cancelled_before_provider_future() {
        let registry = AgentRuntimeRegistry::default();
        let (_guard, token) = registry.register_run(1, 42);
        assert!(registry.cancel_run(1, 42));

        let result = await_model_loop_provider_future_or_cancelled(
            token,
            std::future::pending::<Result<(), AppError>>(),
            "model_call",
            std::future::pending::<Result<&'static str, AppError>>(),
        )
        .await
        .unwrap();

        assert_eq!(result, ModelLoopFutureAwait::Cancelled);
    }

    #[tokio::test]
    async fn tool_io_runtime_registry_cancel_returns_external_cancel_execution() {
        let registry = AgentRuntimeRegistry::default();
        let (_guard, token) = registry.register_run(1, 42);
        assert!(registry.cancel_run(1, 42));
        let calls = vec![test_prepared_tool_call(0, "call-1", "rag.search")];

        let result = execute_agent_tool_io_batch(
            ToolBatchExecutionMode::Serial,
            calls,
            token,
            |prepared| async move {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                Ok(test_executed_tool_call(prepared))
            },
        )
        .await
        .unwrap();

        assert_eq!(result[0].execution.status, "cancelled");
        assert_eq!(result[0].terminal_status, RunStatus::Cancelled);
        assert_eq!(
            result[0].execution.response_payload["cancelReason"],
            "external_cancel"
        );
        assert_eq!(
            result[0].execution.response_payload["cancelStage"],
            "tool_io"
        );
        let metrics = result[0].tool_io_metrics.as_ref().unwrap();
        assert_eq!(metrics.execution_mode, ToolBatchExecutionMode::Serial);
        assert_eq!(metrics.terminal_status, RunStatus::Cancelled);
        assert_eq!(metrics.cancel_reason.as_deref(), Some("external_cancel"));
    }

    #[test]
    fn agent_service_parallel_tool_execution_separates_io_from_persistence() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("execute_agent_tool_io_batch"));
        assert!(source.contains("execute_agent_tool_io"));
        assert!(source.contains("record_agent_tool_execution"));
    }

    #[test]
    fn agent_tool_io_task_control_lives_in_runtime_module() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("agent_tool_io_runtime::{"));
        assert!(source.contains("execute_agent_tool_io_batch"));
        for needle in [
            "async fn execute_agent_tool_io_batch",
            "async fn execute_agent_tool_io_with_timeout_and_cancel",
        ] {
            assert!(
                !source.contains(needle),
                "{needle} should live in agent_tool_io_runtime.rs"
            );
        }
    }

    #[test]
    fn agent_service_model_loop_evaluates_batch_approval_before_execution() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        let approval_index = source.find("batch_policy.requires_approval").unwrap();
        let execution_index = source.find("execute_agent_tool_io_batch(").unwrap();
        assert!(approval_index < execution_index);
    }

    #[test]
    fn agent_service_model_loop_executes_parallel_batches_via_io_executor() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("execute_agent_tool_io_batch("));
        assert!(source.contains("batch_execution_mode"));
        assert!(source.contains("for executed_call in executed_calls"));
    }

    #[test]
    fn agent_service_model_loop_attaches_tool_io_task_metrics_to_observations() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("\"toolIoTask\""));
        assert!(source.contains("executed_call.tool_io_metrics"));
        assert!(source.contains("tool_io_metrics_payload"));
    }

    #[test]
    fn model_loop_tool_executor_binding_payload_serializes_dispatch_metadata() {
        let binding = novex_tools::ToolExecutorBinding::new(
            "media.image.generate",
            "model.media.image.generate",
            novex_tools::ToolExecutorKind::Model,
        )
        .with_background_tasks()
        .waits_for_runtime_cancellation();

        let payload = model_loop_tool_executor_binding_payload(Some(&binding));

        assert_eq!(payload["toolCode"], "media.image.generate");
        assert_eq!(payload["executorCode"], "model.media.image.generate");
        assert_eq!(payload["kind"], "model");
        assert_eq!(payload["supportsBackgroundTasks"], true);
        assert_eq!(payload["waitsForRuntimeCancellation"], true);
        assert_eq!(model_loop_tool_executor_binding_payload(None), Value::Null);
    }

    #[test]
    fn agent_service_model_loop_uses_tool_executor_registry_for_dispatch_metadata() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains(
            "ToolExecutorRegistry::from_bindings(agent_model_loop_tool_executor_bindings())"
        ));
        assert!(source.contains("executor_registry"));
        assert!(source.contains(".executor_for(&routed_call.tool.code)"));
        assert!(source.contains("executor_binding: Some(executor_binding.clone())"));
        assert!(source.contains("\"executorBinding\""));
        assert!(source.contains("prepared.executor_binding"));
    }

    #[test]
    fn agent_service_tool_executor_dispatch_plan_selection_boundary_guides_tool_io() {
        let service_source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let executor_source = include_str!("agent_tool_executor.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(service_source.contains("ToolExecutorDispatchPlan::from_binding"));
        assert!(service_source.contains("agent_tool_requires_github_connector_credential"));
        assert!(service_source.contains("agent_tool_requires_mcp_lookup"));
        assert!(service_source.contains("executor_dispatch.as_ref(),"));
        assert!(service_source.contains("execute_agent_tool("));
        assert!(executor_source.contains("AgentToolExecutorSelection::from_dispatch"));
        assert!(executor_source.contains("AgentToolExecutorSelection::MediaImage"));
        assert!(executor_source.contains("AgentToolExecutorSelection::GitHubRepoSearch"));
        assert!(executor_source.contains("AgentToolExecutorSelection::DryRun"));
    }

    #[test]
    fn agent_tool_input_adapters_live_in_novex_tools() {
        let executor_source = include_str!("agent_tool_executor.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(executor_source.contains("feishu_message_text_from_tool_input,"));
        assert!(executor_source.contains("github_read_request_from_tool_input,"));
        assert!(executor_source.contains("github_search_request_from_tool_input,"));
        assert!(executor_source.contains("media_image_request_from_tool_input,"));

        for local_definition in [
            "fn feishu_message_text_from_tool_input",
            "fn media_image_request_from_tool_input",
            "fn github_search_request_from_tool_input",
            "fn github_read_request_from_tool_input",
        ] {
            assert!(
                !executor_source.contains(local_definition),
                "{local_definition} should live in novex-tools"
            );
        }
    }

    #[test]
    fn agent_tool_execution_envelope_lives_in_novex_tools() {
        let service_source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let executor_source = include_str!("agent_tool_executor.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let runtime_source = include_str!("agent_tool_io_runtime.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(service_source.contains("AgentToolExecution,"));
        assert!(executor_source.contains("AgentToolExecution,"));
        assert!(runtime_source.contains("AgentToolExecution,"));
        assert!(
            !service_source.contains("struct AgentToolExecution"),
            "AgentToolExecution should live in novex-tools"
        );
        assert!(
            !executor_source.contains("struct AgentToolExecution"),
            "AgentToolExecution should live in novex-tools"
        );
        assert!(
            !runtime_source.contains("struct AgentToolExecution"),
            "AgentToolExecution should live in novex-tools"
        );
        assert!(executor_source.contains("AgentToolExecution::succeeded("));
        assert!(executor_source.contains("AgentToolExecution::failed("));
        assert!(runtime_source.contains("AgentToolExecution::cancelled("));
    }

    #[test]
    fn agent_concrete_tool_executors_live_in_executor_module() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("execute_agent_tool,"));
        for needle in [
            "async fn execute_agent_tool(",
            "async fn execute_mcp_tool(",
            "async fn execute_github_repo_search_tool(",
            "async fn execute_github_repo_read_tool(",
            "async fn execute_media_image_tool(",
            "async fn execute_feishu_message_tool(",
        ] {
            assert!(
                !source.contains(needle),
                "{needle} should live in agent_tool_executor.rs"
            );
        }
    }

    #[test]
    fn model_loop_tool_router_exposes_prompt_codes() {
        let router = build_model_loop_tool_router().unwrap();

        assert!(router.tool_codes().contains(&"rag.search".to_owned()));
        assert!(router.tool_codes().contains(&"github.repo.read".to_owned()));
    }

    #[test]
    fn agent_service_model_loop_records_unknown_tool_stop_reason() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("\"stopReason\": stop_reason"));
        assert!(source.contains("\"toolRouteError\": err"));
        assert_eq!(
            tool_route_stop_reason(ToolRouteErrorKind::UnknownTool),
            "unknown_tool"
        );
    }

    #[test]
    fn agent_service_model_loop_records_context_compaction_event() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let normalized_source = source.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(source.contains("runtime_state.should_compact_context()"));
        assert!(normalized_source.contains("runtime_state .compact_context_with_summary"));
        assert!(source.contains("AgentTurnItem::ContextCompaction"));
        assert!(source.contains("\"compactionWindowId\""));
    }

    #[test]
    fn agent_service_model_loop_installs_response_item_history_for_sampling() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let normalized_source = source.split_whitespace().collect::<Vec<_>>().join(" ");
        let model_loop = &source[source
            .find("async fn execute_model_loop_existing_run")
            .unwrap()
            ..source
                .find("async fn model_loop_context_compaction_outcome")
                .unwrap()];

        assert!(source.contains("build_model_loop_messages_from_history"));
        assert!(normalized_source.contains(
            "build_model_loop_messages_from_history( &command.input, &tool_codes, command.workbench_context.as_ref(), skill_context.as_deref(), &runtime_state.items, )"
        ));
        assert!(!model_loop.contains("let mut messages = vec!"));
        assert!(!model_loop.contains("messages.push(ModelChatMessage"));
    }

    #[test]
    fn model_loop_budget_enables_context_compaction_threshold() {
        let budget = agent_runtime_budget_from_task_budget(TaskBudget {
            max_steps: Some(6),
            max_tool_calls: Some(3),
            max_seconds: Some(30),
            max_cost_cents: Some(0),
        });

        assert_eq!(budget.max_turns, 6);
        assert_eq!(budget.max_tool_calls, 3);
        assert_eq!(budget.compact_after_observations, Some(2));
    }

    #[test]
    fn model_loop_history_messages_project_response_items_for_follow_up() {
        let tool_codes = build_model_loop_tool_router().unwrap().tool_codes();
        let history = vec![
            AgentTurnItem::user_message("Find refund policy"),
            AgentTurnItem::tool_call("call-1", "rag.search", json!({ "query": "refund policy" })),
            AgentTurnItem::tool_observation(
                "call-1",
                ToolObservationStatus::Succeeded,
                json!({ "answer": "refund within 7 days" }),
            ),
        ];

        let messages = build_model_loop_messages_from_history(
            "Find refund policy",
            &tool_codes,
            None,
            None,
            &history,
        );

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("Novex Agent Runtime"));
        assert!(messages[0].content.contains("github.repo.read"));
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[1].content, "Find refund policy");
        assert_eq!(messages[2].role, "assistant");
        assert!(messages[2].content.contains(r#""type":"tool_call""#));
        assert!(messages[2].content.contains(r#""toolCode":"rag.search""#));
        assert_eq!(messages[3].role, "user");
        assert!(messages[3].content.contains("call-1"));
        assert!(messages[3].content.contains("rag.search"));
        assert!(messages[3].content.contains("refund within 7 days"));
        assert!(messages[3].content.contains("final answer"));
    }

    #[test]
    fn model_loop_history_messages_resume_from_latest_compaction_window() {
        let tool_codes = build_model_loop_tool_router().unwrap().tool_codes();
        let history = vec![
            AgentTurnItem::user_message("Find refund policy"),
            AgentTurnItem::tool_call("call-1", "rag.search", json!({ "query": "refund policy" })),
            AgentTurnItem::tool_observation(
                "call-1",
                ToolObservationStatus::Succeeded,
                json!({ "answer": "old evidence" }),
            ),
            AgentTurnItem::ContextCompaction {
                summary: "Observation for call-1: refund within 7 days".to_owned(),
            },
            AgentTurnItem::tool_call("call-2", "github.repo.read", json!({ "path": "README.md" })),
        ];

        let messages = build_model_loop_messages_from_history(
            "Find refund policy",
            &tool_codes,
            None,
            None,
            &history,
        );

        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[1].content, "Find refund policy");
        assert_eq!(messages[2].role, "user");
        assert!(messages[2].content.contains("refund within 7 days"));
        assert!(messages[2]
            .content
            .contains("Continue from this compacted context"));
        assert_eq!(messages[3].role, "assistant");
        assert!(messages[3].content.contains(r#""callId":"call-2""#));
        assert!(!messages
            .iter()
            .any(|message| message.content.contains("old evidence")));
    }

    #[test]
    fn model_loop_compaction_prompt_uses_deterministic_candidate_and_tool_context() {
        let tool_codes = vec!["rag.search".to_owned(), "github.repo.read".to_owned()];

        let messages = build_model_loop_context_compaction_messages(
            "Find refund policy",
            "Observation for call-1: refund within 7 days",
            &tool_codes,
        );

        assert_eq!(messages[0].role, "system");
        assert!(messages[0]
            .content
            .contains("Novex Agent Context Compactor"));
        assert!(messages[1].content.contains("Find refund policy"));
        assert!(messages[1].content.contains("refund within 7 days"));
        assert!(messages[1].content.contains("rag.search, github.repo.read"));
    }

    #[test]
    fn remote_compaction_prompt_includes_endpoint_metadata() {
        let request = test_remote_compaction_request();

        let messages = build_model_loop_remote_context_compaction_messages(
            "Find refund policy",
            "Observation for call-1: refund within 7 days",
            &["rag.search".to_owned()],
            Some(&request),
        );

        assert!(messages[0]
            .content
            .contains("remote compaction endpoint adapter"));
        assert!(messages[1].content.contains("responses_compaction_v2"));
        assert!(messages[1].content.contains("observation_threshold"));
        assert!(messages[1].content.contains("inputHistoryCount"));
    }

    #[test]
    fn remote_compaction_maps_to_model_request_metadata() {
        let request = test_remote_compaction_request();

        let metadata = model_chat_request_metadata_for_remote_compaction(Some(&request)).unwrap();
        let compaction = metadata.compaction.as_ref().unwrap();

        assert_eq!(
            metadata.request_kind,
            crate::application::ai::model_service::ModelChatRequestKind::Compaction
        );
        assert_eq!(compaction.implementation, "responses_compaction_v2");
        assert_eq!(compaction.trigger, "auto");
        assert_eq!(compaction.reason, "observation_threshold");
        assert_eq!(compaction.phase, "model_loop_follow_up");
        assert_eq!(compaction.strategy, "memento");
        assert_eq!(compaction.window_id, 1);
        assert_eq!(compaction.input_history_count, 2);
        assert_eq!(compaction.retained_history_count, 1);
        assert_eq!(compaction.tool_codes, vec!["rag.search"]);
    }

    #[test]
    fn model_loop_model_compaction_response_accepts_json_or_plain_text() {
        assert_eq!(
            model_loop_context_compaction_summary_from_response(r#"{"summary":"short policy"}"#),
            "short policy"
        );
        assert_eq!(
            model_loop_context_compaction_summary_from_response("plain short policy"),
            "plain short policy"
        );
    }

    #[test]
    fn agent_service_model_loop_uses_model_assisted_context_compaction() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let normalized_source = source.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(source.contains("runtime_state.compaction_candidate_summary()"));
        assert!(source.contains("model_loop_context_compaction_outcome"));
        assert!(source.contains("chat_completion_for_purpose("));
        assert!(source.contains("ModelRoutePurpose::CodeAgent"));
        assert!(normalized_source.contains("runtime_state .compact_context_with_summary"));
        assert!(source.contains("\"compactionStrategy\""));
        assert!(source.contains("\"compactionStatus\""));
    }

    #[test]
    fn remote_compaction_agent_service_records_endpoint_request() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("remote_compaction_request"));
        assert!(source.contains("\"remoteCompaction\""));
        assert!(source.contains("\"compactionImplementation\""));
        assert!(source.contains("\"modelRequestMetadata\""));
        assert!(source.contains("\"compactionTransport\""));
    }

    #[test]
    fn remote_compaction_agent_service_passes_provider_metadata_to_model_call() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let compaction = &source[source
            .find("async fn model_loop_context_compaction_outcome")
            .unwrap()
            ..source.find("pub async fn list_runs").unwrap()];

        assert!(compaction.contains("request_metadata"));
        assert!(compaction.contains("model_chat_request_metadata_for_remote_compaction"));
    }

    #[test]
    fn observation_prompt_includes_tool_result_and_final_answer_instruction() {
        let prompt =
            build_model_loop_observation_history_prompt(&[ModelLoopToolObservationProjection {
                call_id: "call-1".to_owned(),
                tool_code: Some("rag.search".to_owned()),
                status: ToolObservationStatus::Succeeded,
                output: serde_json::json!({"hits":[{"title":"Policy"}]}),
            }]);

        assert!(prompt.contains("rag.search"));
        assert!(prompt.contains("call-1"));
        assert!(prompt.contains("Policy"));
        assert!(prompt.contains("final answer"));
    }

    #[test]
    fn agent_runtime_low_risk_tool_can_finish_without_approval() {
        let command = normalize_agent_run_command(AgentRunCommand {
            input: "search the training handbook".to_owned(),
            runtime_mode: None,
            execution_mode: None,
            model_route_id: None,
            auto_approve: false,
            budget: TaskBudget {
                max_steps: Some(6),
                max_tool_calls: Some(1),
                max_seconds: Some(30),
                max_cost_cents: Some(0),
            },
            workbench_context: None,
        })
        .unwrap();
        let plan = build_agent_plan(&command, MemoryContext::empty()).unwrap();

        assert_eq!(plan.selected_tool_code.as_deref(), Some("rag.search"));
        assert!(!plan.requires_approval);
        assert_eq!(plan.initial_status, "running");
    }

    #[test]
    fn agent_runtime_medium_risk_tool_pauses_without_auto_approval() {
        let command = normalize_agent_run_command(AgentRunCommand {
            input: "send a Feishu reminder".to_owned(),
            runtime_mode: None,
            execution_mode: None,
            model_route_id: None,
            auto_approve: false,
            budget: TaskBudget {
                max_steps: Some(6),
                max_tool_calls: Some(1),
                max_seconds: Some(30),
                max_cost_cents: Some(0),
            },
            workbench_context: None,
        })
        .unwrap();
        let plan = build_agent_plan(&command, MemoryContext::empty()).unwrap();

        assert_eq!(
            plan.selected_tool_code.as_deref(),
            Some("feishu.message.send")
        );
        assert!(plan.requires_approval);
        assert_eq!(plan.pause_reason.as_deref(), Some("approval"));
    }

    #[test]
    fn agent_plan_carries_db_memory_context_into_retrieval_payload() {
        let memory_context = novex_memory::MemoryContext {
            snippets: vec![novex_memory::MemorySnippet {
                tenant_id: "42".to_owned(),
                scope: novex_memory::MemoryScope::User,
                scope_id: "7".to_owned(),
                key: "profile.locale".to_owned(),
                content: "Prefers Chinese answers".to_owned(),
                write_policy: novex_memory::MemoryWritePolicy::UserApproved,
            }],
        };
        let command = normalize_agent_run_command(AgentRunCommand {
            input: "answer in my preferred language".to_owned(),
            runtime_mode: None,
            execution_mode: None,
            model_route_id: None,
            auto_approve: false,
            budget: TaskBudget::default(),
            workbench_context: None,
        })
        .unwrap();

        let plan = build_agent_plan(&command, memory_context.clone()).unwrap();
        let payload = agent_context_retrieval_payload(&command.input, &plan.memory_context);

        assert_eq!(plan.memory_context, memory_context);
        assert_eq!(payload["hitCount"], 1);
        assert_eq!(payload["source"], "ai_memory");
        assert_eq!(
            payload["memoryContext"]["snippets"][0]["content"],
            "Prefers Chinese answers"
        );
    }

    #[test]
    fn agent_memory_context_from_records_applies_shared_scope_filter() {
        let now = chrono::NaiveDate::from_ymd_opt(2026, 6, 6)
            .unwrap()
            .and_hms_opt(1, 2, 3)
            .unwrap();
        let records = vec![
            crate::infrastructure::persistence::ai_memory_repository::MemoryRecord {
                id: 10,
                scope_type: "user".to_owned(),
                scope_id: "7".to_owned(),
                source_kind: "manual".to_owned(),
                source_id: None,
                content: "Prefers Chinese answers".to_owned(),
                summary: "profile.locale".to_owned(),
                sensitivity: "preference".to_owned(),
                write_policy: "user_approved".to_owned(),
                ttl_days: Some(90),
                expires_at: None,
                metadata: json!({}),
                status: 1,
                create_time: now,
                update_time: None,
            },
            crate::infrastructure::persistence::ai_memory_repository::MemoryRecord {
                id: 11,
                scope_type: "user".to_owned(),
                scope_id: "8".to_owned(),
                source_kind: "manual".to_owned(),
                source_id: None,
                content: "Wrong user".to_owned(),
                summary: "profile.locale".to_owned(),
                sensitivity: "preference".to_owned(),
                write_policy: "user_approved".to_owned(),
                ttl_days: Some(90),
                expires_at: None,
                metadata: json!({}),
                status: 1,
                create_time: now,
                update_time: None,
            },
        ];

        let context = agent_memory_context_from_records(42, 7, records);

        assert_eq!(context.snippets.len(), 1);
        assert_eq!(context.snippets[0].tenant_id, "42");
        assert_eq!(context.snippets[0].scope_id, "7");
        assert_eq!(context.snippets[0].key, "profile.locale");
    }

    #[test]
    fn agent_tool_policy_requires_manual_approval_for_high_risk_even_when_auto_approved() {
        let decision = agent_tool_policy_decision(
            &ToolLookupRecord {
                id: 1,
                code: "github.issue.write".to_owned(),
                tool_kind: "connector".to_owned(),
                executor_kind: "connector".to_owned(),
                risk_level: 3,
                approval_policy: 1,
                permission_code: Some("ai:agent:run".to_owned()),
            },
            true,
        );

        assert!(decision.requires_approval);
        assert_eq!(decision.pause_reason.as_deref(), Some("approval"));
        assert_eq!(decision.policy_reason, "high_risk_requires_manual_approval");
    }

    #[test]
    fn guardian_review_high_risk_policy_requires_human_even_when_auto_approved() {
        let decision = guardian_review_for_tool_policy(
            &ToolLookupRecord {
                id: 1,
                code: "github.issue.write".to_owned(),
                tool_kind: "connector".to_owned(),
                executor_kind: "connector".to_owned(),
                risk_level: 3,
                approval_policy: 1,
                permission_code: Some("ai:agent:run".to_owned()),
            },
            true,
        );

        assert_eq!(decision.outcome, GuardianReviewOutcome::NeedsHuman);
        assert!(decision.requires_human_approval);
        assert!(!decision.can_execute);
        assert_eq!(decision.rationale, "high_risk_requires_human_approval");
    }

    #[test]
    fn guardian_review_approval_pause_payload_is_recorded() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("guardian_review_payload_for_tool_policy"));
        assert!(source.contains("guardian_review_decision_for_tool_policy"));
        assert!(source.contains("review_tool_approval"));
        assert!(source.contains("\"guardianReview\""));
    }

    #[test]
    fn guardian_model_review_backend_uses_dedicated_route_and_timeout() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let normalized_source = source.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(source.contains("GUARDIAN_REVIEW_TIMEOUT"));
        assert!(normalized_source.contains("tokio::time::timeout( GUARDIAN_REVIEW_TIMEOUT"));
        assert!(source.contains("ModelRoutePurpose::GuardianReview"));
        assert!(source.contains("build_guardian_model_review_prompt"));
        assert!(source.contains("parse_guardian_model_assessment"));
    }

    #[test]
    fn guardian_model_review_request_includes_runtime_transcript_and_tool_action() {
        let mut runtime_state = AgentRuntimeState::new("run-1");
        runtime_state.push_item(AgentTurnItem::user_message("create an issue"));
        runtime_state.push_item(AgentTurnItem::tool_observation(
            "call-1",
            ToolObservationStatus::Succeeded,
            json!({"repo":"owner/repo"}),
        ));
        let tool = ToolLookupRecord {
            id: 1,
            code: "github.issue.write".to_owned(),
            tool_kind: "connector".to_owned(),
            executor_kind: "connector".to_owned(),
            risk_level: 3,
            approval_policy: 1,
            permission_code: Some("ai:agent:run".to_owned()),
        };

        let request = guardian_model_review_request_for_tool(
            "create an issue",
            Some(&runtime_state.items),
            &tool,
            json!({"title":"Bug"}),
        );

        assert!(request
            .transcript
            .iter()
            .any(|entry| entry.content.contains("create an issue")));
        assert!(request
            .transcript
            .iter()
            .any(|entry| entry.content.contains("owner/repo")));
        assert_eq!(request.reviewed_action.tool_code, "github.issue.write");
        assert_eq!(request.reviewed_action.arguments["title"], "Bug");
        assert_eq!(
            request.reviewed_action.permission_code.as_deref(),
            Some("ai:agent:run")
        );
    }

    #[test]
    fn guardian_model_review_metadata_is_added_to_decision() {
        let mut decision = guardian_review_for_tool_policy(
            &ToolLookupRecord {
                id: 1,
                code: "github.issue.write".to_owned(),
                tool_kind: "connector".to_owned(),
                executor_kind: "connector".to_owned(),
                risk_level: 3,
                approval_policy: 1,
                permission_code: Some("ai:agent:run".to_owned()),
            },
            true,
        );
        decision.review_status = GuardianReviewStatus::Reviewed;
        let response = ModelChatResp {
            conversation_id: None,
            answer: "{}".to_owned(),
            route_id: "runtime.llm.guardian".to_owned(),
            provider: "deep-seek".to_owned(),
            model: Some("deepseek-v4-flash".to_owned()),
            latency_ms: 17,
            usage: ModelChatUsage::default(),
            cost_cents: None,
            provider_attempts: vec![],
            provider_call_lease_id: None,
            provider_response_id: None,
            provider_response_status: None,
            provider_delta_chunks: vec![],
        };

        let decision = guardian_review_decision_with_model_metadata(decision, &response, 19);

        assert_eq!(
            decision.model_route_id.as_deref(),
            Some("runtime.llm.guardian")
        );
        assert_eq!(decision.model_provider.as_deref(), Some("deep-seek"));
        assert_eq!(decision.model_name.as_deref(), Some("deepseek-v4-flash"));
        assert_eq!(decision.review_latency_ms, Some(19));
    }

    #[test]
    fn guardian_auto_approval_reviewed_approved_decision_allows_execution() {
        let mut decision = guardian_review_for_tool_policy(
            &ToolLookupRecord {
                id: 1,
                code: "github.issue.write".to_owned(),
                tool_kind: "connector".to_owned(),
                executor_kind: "connector".to_owned(),
                risk_level: 3,
                approval_policy: 1,
                permission_code: Some("ai:agent:run".to_owned()),
            },
            true,
        );
        decision.outcome = GuardianReviewOutcome::Approved;
        decision.source = GuardianDecisionSource::Guardian;
        decision.requires_human_approval = false;
        decision.can_execute = true;
        decision.review_status = GuardianReviewStatus::Reviewed;

        assert!(guardian_auto_approval_allows_execution(&decision));
    }

    #[test]
    fn guardian_auto_approval_rejects_policy_only_and_failed_closed_decisions() {
        let policy_only = guardian_review_for_tool_policy(
            &ToolLookupRecord {
                id: 1,
                code: "rag.search".to_owned(),
                tool_kind: "function".to_owned(),
                executor_kind: "agent".to_owned(),
                risk_level: 1,
                approval_policy: 0,
                permission_code: Some("ai:agent:run".to_owned()),
            },
            true,
        );
        assert_eq!(policy_only.source, GuardianDecisionSource::Policy);
        assert_eq!(policy_only.review_status, GuardianReviewStatus::PolicyOnly);
        assert!(!guardian_auto_approval_allows_execution(&policy_only));

        let mut failed_closed = policy_only.clone();
        failed_closed.source = GuardianDecisionSource::Guardian;
        failed_closed.review_status = GuardianReviewStatus::FailedClosed;
        failed_closed.failure_reason = Some(GuardianReviewFailureReason::Timeout);
        failed_closed.can_execute = false;

        assert!(!guardian_auto_approval_allows_execution(&failed_closed));
    }

    #[test]
    fn guardian_auto_approval_backend_continues_before_deterministic_pause() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let deterministic_branch = &source[source.find("if let Some(tool) = selected_tool").unwrap()
            ..source.find("async fn create_model_loop_run").unwrap()];

        let review_index = deterministic_branch
            .find("guardian_review_decision_for_tool_policy")
            .unwrap();
        let gate_index = deterministic_branch
            .find("guardian_auto_approval_allows_execution(&guardian_review_decision)")
            .unwrap();
        let pause_index = deterministic_branch.find("pause_for_approval").unwrap();

        assert!(review_index < gate_index);
        assert!(gate_index < pause_index);
        assert!(deterministic_branch.contains("\"guardianAutoApproved\""));
        assert!(deterministic_branch.contains("\"guardian_auto_approved\""));
    }

    #[test]
    fn guardian_auto_approval_backend_handles_batch_before_execution() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let batch_branch = &source[source.find("if batch_policy.requires_approval").unwrap()
            ..source.find("let mut last_recorded_step_id").unwrap()];
        let normalized_batch_branch = batch_branch.split_whitespace().collect::<String>();
        let execution_index = source.find("execute_agent_tool_io_batch(").unwrap();
        let gate_index = source
            .find("guardian_auto_approval_allows_execution(&guardian_review_decision)")
            .unwrap();

        assert!(gate_index < execution_index);
        assert!(normalized_batch_branch.contains("guardian_auto_approved_calls.insert"));
        assert!(batch_branch.contains("guardian_review_override"));
        assert!(source.contains("guardian_review_override: Option<Value>"));
    }

    #[test]
    fn agent_event_stream_query_clamps_cursor_batch_and_timeouts() {
        let settings = AgentRunEventStreamQuery {
            after_sequence_no: -10,
            batch_size: Some(999),
            poll_ms: Some(1),
            max_idle_ms: Some(999_999),
        }
        .settings();

        assert_eq!(settings.after_sequence_no, 0);
        assert_eq!(settings.batch_size, 200);
        assert_eq!(settings.poll_ms, 250);
        assert_eq!(settings.max_idle_ms, 300_000);
    }

    #[test]
    fn agent_event_stream_service_exposes_cursor_and_terminal_helpers() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("list_events_after_sequence"));
        assert!(source.contains("RunEventCursorFilter"));
        assert!(source.contains("is_run_terminal"));
        assert!(source.contains(".is_terminal()"));
    }

    #[test]
    fn agent_runtime_routes_mcp_tools_through_audited_observation_path() {
        let service_source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let executor_source = include_str!("agent_tool_executor.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(service_source.contains("execute_agent_tool("));
        assert!(service_source.contains("RunEventKind::Observation"));
        assert!(executor_source.contains("execute_mcp_tool"));
        assert!(executor_source.contains("ToolKind::Mcp"));
    }

    #[test]
    fn agent_run_events_convert_to_trace_bundle() {
        let events = vec![
            fake_agent_event(
                "input_received",
                1,
                json!({"item":{"type":"user_message","content":"hi"}}),
            ),
            fake_agent_event("tool_called", 2, json!({"toolCode":"rag.search"})),
            fake_agent_event("final_output", 3, json!({"answer":"done"})),
        ];

        let bundle = agent_events_to_trace_bundle("agent-1", events);

        assert_eq!(bundle.trace_id, "agent-1");
        assert_eq!(bundle.tool_call_count(), 1);
        assert_eq!(bundle.replay_summary().final_status, "succeeded");
    }

    #[test]
    fn agent_run_events_convert_runtime_spans_to_trace_bundle() {
        let events = vec![
            fake_agent_event("retrieval", 1, json!({"hitCount":2,"source":"ai_memory"})),
            fake_agent_event(
                "action_selected",
                2,
                json!({"toolCallBatch":[{"toolCode":"rag.search"}]}),
            ),
            fake_agent_event(
                "observation",
                3,
                json!({
                    "item":{"type":"context_compaction","summary":"older tool results compacted"},
                    "compactedItemCount":4
                }),
            ),
            fake_agent_event("cancelled", 4, json!({"cancelReason":"external_cancel"})),
        ];

        let bundle = agent_events_to_trace_bundle("agent-1", events);

        assert!(bundle
            .events
            .iter()
            .any(|event| event.kind == TraceEventKind::Retrieval));
        assert!(bundle
            .events
            .iter()
            .any(|event| event.kind == TraceEventKind::ActionSelected));
        assert!(bundle
            .events
            .iter()
            .any(|event| event.kind == TraceEventKind::ContextCompaction));
        assert!(bundle
            .events
            .iter()
            .any(|event| event.kind == TraceEventKind::Cancellation));
        assert_eq!(bundle.replay_summary().final_status, "cancelled");
    }

    #[test]
    fn agent_run_events_convert_inference_spans_to_trace_bundle() {
        let events = vec![fake_agent_event(
            "thought",
            1,
            json!({
                "runtimeMode": "model_loop",
                "item": {
                    "type": "model_inference",
                    "routeId": "runtime.llm.code_agent",
                    "provider": "deep-seek",
                    "model": "deepseek-v4-flash",
                    "latencyMs": 42,
                    "usage": {
                        "promptTokens": 11,
                        "completionTokens": 7,
                        "totalTokens": 18
                    },
                    "costCents": null
                }
            }),
        )];

        let bundle = agent_events_to_trace_bundle("agent-1", events);

        assert_eq!(bundle.events[0].kind, TraceEventKind::Inference);
        assert_eq!(
            bundle.events[0].payload["item"]["routeId"],
            "runtime.llm.code_agent"
        );
    }

    #[test]
    fn agent_run_events_convert_model_delta_spans_to_trace_bundle() {
        let events = vec![fake_agent_event(
            "thought",
            2,
            json!({
                "runtimeMode": "model_loop",
                "item": {
                    "type": "model_delta",
                    "source": "provider_stream",
                    "routeId": "runtime.llm.code_agent",
                    "provider": "openai-compatible",
                    "model": "gpt-compatible",
                    "deltaIndex": 1,
                    "content": " world",
                    "providerEvent": "chat.completion.chunk"
                }
            }),
        )];

        let bundle = agent_events_to_trace_bundle("agent-1", events);

        assert_eq!(bundle.events[0].kind, TraceEventKind::Inference);
        assert_eq!(bundle.events[0].payload["item"]["type"], "model_delta");
        assert_eq!(
            bundle.events[0].payload["item"]["source"],
            "provider_stream"
        );
        assert_eq!(bundle.events[0].payload["item"]["deltaIndex"], 1);
        assert_eq!(bundle.events[0].payload["item"]["content"], " world");
        assert_eq!(
            bundle.events[0].payload["item"]["providerEvent"],
            "chat.completion.chunk"
        );
    }

    #[test]
    fn agent_run_events_convert_model_stream_tool_call_spans_to_trace_bundle() {
        let events = vec![fake_agent_event(
            "thought",
            2,
            json!({
                "runtimeMode": "model_loop",
                "item": {
                    "type": "model_stream_tool_call",
                    "source": "provider_stream",
                    "routeId": "runtime.llm.code_agent",
                    "provider": "openai-compatible",
                    "model": "gpt-compatible",
                    "deltaIndex": 1,
                    "toolCallCount": 1,
                    "toolCalls": [
                        {
                            "callId": "call-1",
                            "toolCode": "rag.search",
                            "arguments": {"query": "policy"}
                        }
                    ]
                }
            }),
        )];

        let bundle = agent_events_to_trace_bundle("agent-1", events);

        assert_eq!(bundle.events[0].kind, TraceEventKind::Inference);
        assert_eq!(
            bundle.events[0].payload["item"]["type"],
            "model_stream_tool_call"
        );
        assert_eq!(bundle.events[0].payload["item"]["toolCallCount"], 1);
        assert_eq!(
            bundle.events[0].payload["item"]["toolCalls"][0]["toolCode"],
            "rag.search"
        );
    }

    #[test]
    fn agent_run_events_convert_tool_io_task_observation_metrics_to_trace_bundle() {
        let events = vec![fake_agent_event(
            "observation",
            3,
            json!({
                "item": {
                    "type": "tool_observation",
                    "callId": "call-1",
                    "status": "succeeded",
                    "output": {"status": "succeeded"}
                },
                "toolIoTask": {
                    "executionMode": "parallel",
                    "taskRuntime": "tokio_task",
                    "supervisor": "agent_tool_io_task_supervisor",
                    "batchIndex": 0,
                    "durationMs": 12,
                    "terminalStatus": "succeeded"
                }
            }),
        )];

        let bundle = agent_events_to_trace_bundle("agent-1", events);

        assert_eq!(bundle.events[0].kind, TraceEventKind::Observation);
        assert_eq!(
            bundle.events[0].payload["toolIoTask"]["executionMode"],
            "parallel"
        );
        assert_eq!(bundle.events[0].payload["toolIoTask"]["durationMs"], 12);
    }

    #[test]
    fn agent_run_events_convert_provider_error_spans_to_trace_bundle() {
        let events = vec![fake_agent_event(
            "thought",
            1,
            json!({
                "runtimeMode": "model_loop",
                "item": {
                    "type": "model_inference_error",
                    "routeId": "runtime.llm.code_agent",
                    "routePurpose": "code_agent",
                    "attempt": 1,
                    "maxAttempts": 1,
                    "retryable": true,
                    "errorKind": "provider_http",
                    "httpStatus": 502,
                    "message": "LLM model call failed: HTTP 502",
                    "latencyMs": 12
                }
            }),
        )];

        let bundle = agent_events_to_trace_bundle("agent-1", events);

        assert_eq!(bundle.events[0].kind, TraceEventKind::Inference);
        assert_eq!(
            bundle.events[0].payload["item"]["type"],
            "model_inference_error"
        );
        assert_eq!(bundle.events[0].payload["item"]["httpStatus"], 502);
    }

    #[test]
    fn guardian_review_approval_requested_trace_preserves_payload() {
        let payload = json!({
            "toolCode": "github.issue.write",
            "permissionCode": "ai:agent:run",
            "guardianReview": {
                "outcome": "needs_human",
                "source": "policy",
                "requiresHumanApproval": true
            }
        });
        let event = fake_agent_event("approval_requested", 7, payload.clone());

        let trace_event = trace_event_from_run_event(&event).unwrap();

        assert_eq!(trace_event.kind, TraceEventKind::ApprovalRequested);
        assert_eq!(trace_event.payload, payload);
    }

    #[test]
    fn agent_trace_snapshot_contains_replay_summary() {
        let events = vec![
            fake_agent_event("tool_called", 2, json!({"toolCode":"rag.search"})),
            fake_agent_event("final_output", 3, json!({"answer":"done"})),
        ];

        let snapshot = agent_trace_snapshot_payload("agent-1", &events);

        assert_eq!(snapshot["summary"]["toolCallCount"], 1);
        assert_eq!(snapshot["summary"]["finalStatus"], "succeeded");
    }

    #[test]
    fn agent_rollout_migration_defines_replay_bundle_table() {
        let migration = include_str!("../../../migrations/202606160002_create_ai_rollout.sql");

        assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_rollout"));
        assert!(migration.contains("trace_id"));
        assert!(migration.contains("event_bundle"));
        assert!(migration.contains("summary_payload"));
    }

    fn fake_agent_event(event_type: &str, sequence_no: i64, payload: Value) -> RunEventRecord {
        RunEventRecord {
            id: sequence_no,
            run_id: 42,
            step_id: None,
            event_type: event_type.to_owned(),
            sequence_no,
            status: "running".to_owned(),
            payload,
            create_time: Utc::now().naive_utc(),
        }
    }

    #[test]
    fn agent_run_transition_uses_core_status_graph_for_resume() {
        assert!(ensure_agent_run_transition("waiting_approval", RunStatus::Resuming).is_ok());
        assert!(ensure_agent_run_transition("paused", RunStatus::Resuming).is_ok());
    }

    #[test]
    fn agent_run_transition_rejects_terminal_cancel() {
        let err = ensure_agent_run_transition("succeeded", RunStatus::Cancelling).unwrap_err();

        assert!(matches!(err, AppError::Conflict(_)));
        assert!(err.to_string().contains("当前 Run 状态不允许流转"));
    }

    #[test]
    fn agent_run_transition_rejects_unknown_db_status() {
        let err = ensure_agent_run_transition("legacy_running", RunStatus::Cancelling).unwrap_err();

        assert!(matches!(err, AppError::Conflict(_)));
        assert!(err.to_string().contains("未知 Run 状态"));
    }

    #[test]
    fn agent_runtime_records_poc_trace_contract_events() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "RunEventKind::IntentRouted",
            "agent_turn_item_event_payload(&input_item)",
            "command.runtime_mode.as_deref() == Some(\"model_loop\")",
            "create_model_loop_run",
            "chat_completion_for_purpose",
            "record_retrieval_context",
            "RunEventKind::Retrieval",
            "step_type_code(RunStepType::Retrieval)",
            "load_agent_memory_context",
            "agent_context_retrieval_payload",
            "memorySnippetCount",
            "RunEventKind::ToolCalled",
            "RunEventKind::StatusChanged",
            "RunEventKind::FinalOutput",
            "AgentRolloutSaveRecord",
            "upsert_rollout_bundle",
        ] {
            assert!(
                source.contains(needle),
                "{needle} missing from Agent run events"
            );
        }
    }

    #[test]
    fn feishu_message_text_prefers_explicit_message_then_agent_input() {
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
    }

    #[test]
    fn media_job_asset_migration_defines_generation_contract() {
        let migration_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/202606060005_create_ai_media_runtime.sql"
        );
        let migration =
            std::fs::read_to_string(migration_path).expect("missing AI media runtime migration");

        for needle in [
            "CREATE TABLE IF NOT EXISTS ai_media_asset",
            "CREATE TABLE IF NOT EXISTS ai_media_job",
            "tool_call_audit_id",
            "model_route",
            "provider_asset_id",
            "asset_url",
            "policy_result",
            "idx_ai_media_job_trace",
            "idx_ai_media_asset_tenant_id",
        ] {
            assert!(
                migration.contains(needle),
                "{needle} missing from migration"
            );
        }
    }

    #[test]
    fn media_image_request_from_tool_input_prefers_prompt_and_size() {
        let request = media_image_request_from_tool_input(&serde_json::json!({
            "prompt": "Create a course poster",
            "input": "ignored",
            "size": "1024x1024"
        }));

        assert_eq!(request.prompt, "Create a course poster");
        assert_eq!(request.size.as_deref(), Some("1024x1024"));
        assert_eq!(request.count, 1);
    }

    #[test]
    fn media_tool_result_builds_job_and_asset_records() {
        let now = Utc::now().naive_utc();
        let execution = AgentToolExecution::succeeded(
            serde_json::json!({
                "dryRun": false,
                "toolCode": MEDIA_IMAGE_TOOL_CODE,
                "status": "succeeded",
                "provider": "right-code-draw",
                "requestPayload": {
                    "prompt": "Create a course poster"
                },
                "assetUrl": "https://cdn.example.com/poster.png",
                "providerAssetId": "img-1"
            }),
            false,
            "Agent generated image asset.".to_owned(),
        );

        let records = media_records_from_tool_execution(7, 42, 9, 123, &execution, now)
            .expect("media execution should create persistence records");

        assert_eq!(records.job.tool_code, MEDIA_IMAGE_TOOL_CODE);
        assert_eq!(records.job.prompt, "Create a course poster");
        assert_eq!(records.job.tool_call_audit_id, Some(123));
        assert_eq!(records.job.status, "succeeded");
        assert_eq!(
            records.asset.as_ref().unwrap().asset_url.as_deref(),
            Some("https://cdn.example.com/poster.png")
        );
        assert_eq!(
            records.asset.as_ref().unwrap().provider_asset_id.as_deref(),
            Some("img-1")
        );
        assert_eq!(
            records.job.asset_id,
            Some(records.asset.as_ref().unwrap().id)
        );
    }

    #[test]
    fn media_tool_result_uses_dynamic_model_route_from_execution_payload() {
        let now = Utc::now().naive_utc();
        let execution = AgentToolExecution::succeeded(
            serde_json::json!({
                "dryRun": false,
                "toolCode": MEDIA_IMAGE_TOOL_CODE,
                "status": "succeeded",
                "provider": "right-code-draw",
                "modelRoute": "live.dynamic.draw",
                "requestPayload": {
                    "prompt": "Create a course poster"
                },
                "assetUrl": "https://cdn.example.com/poster.png"
            }),
            false,
            "Agent generated image asset.".to_owned(),
        );

        let records = media_records_from_tool_execution(7, 42, 9, 123, &execution, now)
            .expect("media execution should create persistence records");

        assert_eq!(
            records.job.model_route.as_deref(),
            Some("live.dynamic.draw")
        );
    }

    #[test]
    fn media_image_tool_uses_tenant_bound_model_route() {
        let service_source = include_str!("agent_service.rs");
        let executor_source = include_str!("agent_tool_executor.rs");
        assert!(service_source.contains("ModelRuntimeService::for_tenant(db.clone(), tenant_id)"));
        assert!(executor_source
            .contains("resolve_route_for_purpose(ModelRoutePurpose::MediaGeneration)"));
        let static_env_config = ["ModelRuntimeConfig", "::from_env()"].concat();
        let static_draw_persistence = ["then(|| ", "\"runtime.draw\".to_owned())"].concat();
        assert!(!service_source.contains(&static_env_config));
        assert!(!service_source.contains(&static_draw_persistence));
    }

    #[test]
    fn connector_credential_migration_keeps_github_separate_from_identity_login() {
        let migration_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/202606060006_create_ai_connector_credential.sql"
        );
        let migration = std::fs::read_to_string(migration_path)
            .expect("missing AI connector credential migration");

        for needle in [
            "CREATE TABLE IF NOT EXISTS ai_connector_credential",
            "connector_id",
            "scope_type",
            "auth_type",
            "secret_ref",
            "expires_at",
            "scopes",
            "idx_ai_connector_credential_connector",
        ] {
            assert!(
                migration.contains(needle),
                "{needle} missing from migration"
            );
        }
        assert!(migration.contains("INSERT INTO ai_connector_credential"));
        assert!(migration.contains("github.default"));
        assert!(migration.contains("env:GITHUB_CONNECTOR_TOKEN"));
    }

    #[test]
    fn github_search_request_from_tool_input_uses_repo_query_and_path() {
        let request = github_search_request_from_tool_input(&serde_json::json!({
            "repository": "acme/app",
            "query": "parser worker",
            "path": "src",
            "limit": 5
        }))
        .expect("github search input should be valid");

        assert_eq!(request.repository, "acme/app");
        assert_eq!(request.query, "parser worker");
        assert_eq!(request.path.as_deref(), Some("src"));
        assert_eq!(request.limit, 5);
    }

    #[test]
    fn github_read_request_from_tool_input_uses_repo_path_and_ref() {
        let request = github_read_request_from_tool_input(&serde_json::json!({
            "repository": "acme/app",
            "path": "src/lib.rs",
            "ref": "main"
        }))
        .expect("github read input should be valid");

        assert_eq!(request.repository, "acme/app");
        assert_eq!(request.path, "src/lib.rs");
        assert_eq!(request.reference.as_deref(), Some("main"));
    }

    #[test]
    fn github_search_request_from_natural_language_input_extracts_repo_query_and_path() {
        let request = github_search_request_from_tool_input(&serde_json::json!({
            "input": "search GitHub repo acme/app for parser worker under src"
        }))
        .expect("github search natural-language input should be valid");

        assert_eq!(request.repository, "acme/app");
        assert_eq!(request.query, "parser worker");
        assert_eq!(request.path.as_deref(), Some("src"));
    }

    #[test]
    fn github_read_request_from_natural_language_input_extracts_repo_path_and_ref() {
        let request = github_read_request_from_tool_input(&serde_json::json!({
            "input": "read GitHub file acme/app src/lib.rs ref main"
        }))
        .expect("github read natural-language input should be valid");

        assert_eq!(request.repository, "acme/app");
        assert_eq!(request.path, "src/lib.rs");
        assert_eq!(request.reference.as_deref(), Some("main"));
    }

    #[test]
    fn agent_runtime_tool_budget_rejects_tool_plan_when_zero_tool_calls_allowed() {
        let err = normalize_agent_run_command(AgentRunCommand {
            input: "search the training handbook".to_owned(),
            runtime_mode: None,
            execution_mode: None,
            model_route_id: None,
            auto_approve: false,
            budget: TaskBudget {
                max_steps: Some(6),
                max_tool_calls: Some(0),
                max_seconds: Some(30),
                max_cost_cents: Some(0),
            },
            workbench_context: None,
        })
        .and_then(|command| build_agent_plan(&command, MemoryContext::empty()).map(|_| command))
        .unwrap_err();

        assert!(err.to_string().contains("工具调用预算不足"));
    }

    #[test]
    fn agent_runtime_training_quiz_outputs_employee_readable_questions() {
        let output = final_output_for_intent("training_quiz");

        assert!(output.contains("测验已生成"));
        assert!(output.contains("1."));
        assert!(output.contains("客户数据"));
        assert!(!output.contains("without tool"));
    }
}
