use std::{env, time::Duration};

use chrono::{NaiveDateTime, Utc};
use novex_agent::{plan_react_run_with_memory, AgentIntent, AgentLoopKind};
use novex_agent_protocol::{AgentTurnItem, ToolObservationStatus};
use novex_agent_runtime::{parse_model_turn_output, AgentRuntimeBudget, AgentRuntimeState};
use novex_ai_core::{validate_run_transition, RunEventKind, RunStatus, RunStepType, TaskBudget};
use novex_connectors::{
    parse_credential_scope, parse_github_code_search_response, resolve_env_secret_ref,
    select_connector_credential, ConnectorCredentialBinding, FeishuTextMessage,
    GitHubCodeSearchRequest, GitHubFileReadRequest, ResolvedConnectorCredential,
};
use novex_mcp::{McpToolInvocationRequest, McpToolInvocationResult};
use novex_memory::{
    build_memory_context, MemoryAccessContext, MemoryContext, MemoryScope, MemoryScopeRef,
    MemorySnippet, MemoryWritePolicy,
};
use novex_model::ModelRoutePurpose;
use novex_tools::{
    agent_model_loop_tool_definitions, evaluate_tool_execution_policy,
    parse_media_image_generation_response, ApprovalPolicy, MediaImageGenerationRequest,
    ToolExecutionPolicyDecision, ToolExecutionPolicyInput, ToolKind, ToolRiskLevel, ToolRouteError,
    ToolRouteErrorKind, ToolRouter,
};
use novex_trace::{TraceBundle, TraceEvent, TraceReplaySummary};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::{
    application::ai::model_service::{ModelChatCommand, ModelChatMessage, ModelRuntimeService},
    application::system::{ensure_max_chars, format_datetime},
    infrastructure::persistence::{
        ai_agent_repository::{
            AgentRolloutSaveRecord, AgentRunFilter, AgentRunRecord, AgentRunSaveRecord,
            AgentRunStatusUpdate, AgentTraceSaveRecord, AiAgentRepository, RunEventFilter,
            RunEventRecord, RunEventSaveRecord, RunPauseSaveRecord, RunSaveRecord, RunStatusUpdate,
            RunStepSaveRecord,
        },
        ai_capability_repository::{AiCapabilityRepository, ToolAuditSaveRecord, ToolLookupRecord},
        ai_capability_repository::{ConnectorCredentialLookupRecord, McpToolExecutionRecord},
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
const MAX_TRACE_REPLAY_EVENTS: i64 = 1000;
const FEISHU_TOOL_CODE: &str = "feishu.message.send";
const MEDIA_IMAGE_TOOL_CODE: &str = "media.image.generate";
const GITHUB_REPO_SEARCH_TOOL_CODE: &str = "github.repo.search";
const GITHUB_REPO_READ_TOOL_CODE: &str = "github.repo.read";
const GITHUB_CONNECTOR_CODE: &str = "github.default";
const FEISHU_WEBHOOK_TIMEOUT: Duration = Duration::from_secs(10);
const MEDIA_IMAGE_TIMEOUT: Duration = Duration::from_secs(30);
const GITHUB_CONNECTOR_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_AGENT_MEMORY_SNIPPETS: usize = 6;
const MAX_AGENT_MEMORY_CANDIDATES: i64 = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FeishuWebhookConfig {
    webhook_url: String,
}

impl FeishuWebhookConfig {
    fn from_env() -> Option<Self> {
        Self::from_env_map(|key| env::var(key).ok())
    }

    fn from_env_map<F>(mut env_get: F) -> Option<Self>
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

#[derive(Debug, Clone)]
struct AgentToolExecution {
    response_payload: Value,
    status: String,
    dry_run: bool,
    error_message: Option<String>,
    final_output: String,
}

#[derive(Debug, Clone)]
struct RecordedToolExecution {
    audit_id: i64,
    step_id: i64,
    execution: AgentToolExecution,
    terminal_status: RunStatus,
}

#[derive(Debug, Clone)]
struct MediaPersistenceRecords {
    asset: Option<MediaAssetSaveRecord>,
    job: MediaJobSaveRecord,
}

type GitHubConnectorAuth = ResolvedConnectorCredential;

impl AgentToolExecution {
    fn succeeded(response_payload: Value, dry_run: bool, final_output: String) -> Self {
        Self {
            response_payload,
            status: "succeeded".to_owned(),
            dry_run,
            error_message: None,
            final_output,
        }
    }

    fn failed(response_payload: Value, error_message: String, final_output: String) -> Self {
        Self {
            response_payload,
            status: "failed".to_owned(),
            dry_run: false,
            error_message: Some(error_message),
            final_output,
        }
    }

    fn succeeded_status(&self) -> bool {
        self.status == "succeeded"
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunCommand {
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub runtime_mode: Option<String>,
    #[serde(default)]
    pub auto_approve: bool,
    #[serde(default)]
    pub budget: TaskBudget,
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
}

#[derive(Debug, Clone)]
pub struct AgentService {
    tenant_id: i64,
    repo: AiAgentRepository,
    capability_repo: AiCapabilityRepository,
    media_repo: AiMediaRepository,
    memory_repo: AiMemoryRepository,
    model_runtime: ModelRuntimeService,
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
        Self {
            tenant_id,
            repo: AiAgentRepository::new(db.clone()),
            capability_repo: AiCapabilityRepository::new(db.clone()),
            media_repo: AiMediaRepository::new(db.clone()),
            memory_repo: AiMemoryRepository::new(db.clone()),
            model_runtime: ModelRuntimeService::for_tenant(db.clone(), tenant_id),
        }
    }

    pub async fn create_run(
        &self,
        user_id: i64,
        command: AgentRunCommand,
    ) -> Result<AgentRunResp, AppError> {
        let command = normalize_agent_run_command(command)?;
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
        let initial_status = if plan.requires_approval {
            run_status_code(RunStatus::WaitingApproval)
        } else {
            run_status_code(RunStatus::Running)
        };

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
                self.pause_for_approval(user_id, run_id, &tool, &command.input, now)
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

        let input_item = novex_agent_protocol::AgentTurnItem::user_message(command.input.as_str());
        let mut runtime_state = AgentRuntimeState::with_budget(
            run_id.to_string(),
            agent_runtime_budget_from_task_budget(command.budget),
        );
        runtime_state.push_item(input_item.clone());
        let mut input_payload = agent_turn_item_event_payload(&input_item);
        if let Some(object) = input_payload.as_object_mut() {
            object.insert("input".to_owned(), json!(&command.input));
            object.insert("runtimeMode".to_owned(), json!("model_loop"));
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

        let tool_router = build_model_loop_tool_router().map_err(tool_route_error_to_app_error)?;
        let tool_codes = tool_router.tool_codes();
        let mut messages = vec![
            ModelChatMessage {
                role: "system".to_owned(),
                content: build_model_loop_system_prompt(&tool_codes),
            },
            ModelChatMessage {
                role: "user".to_owned(),
                content: command.input.clone(),
            },
        ];
        let mut last_tool_terminal_status = RunStatus::Succeeded;

        for _turn_index in 0..runtime_state.budget.max_turns {
            let model_response = self
                .model_runtime
                .chat_completion_for_purpose(
                    ModelRoutePurpose::CodeAgent,
                    ModelChatCommand {
                        messages: messages.clone(),
                        temperature: Some(0.2),
                        max_tokens: Some(1024),
                        ..ModelChatCommand::default()
                    },
                )
                .await?;

            let parsed = parse_model_turn_output(&model_response.answer).map_err(|err| {
                AppError::bad_request(format!("Agent 模型输出解析失败: {}", err.message))
            })?;
            let parsed_payload = agent_turn_item_event_payload(&parsed.item);

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
                    call_id,
                    tool_code,
                    arguments,
                } => {
                    if !runtime_state.can_execute_tool_call() {
                        let final_output = format!(
                            "Tool call budget exhausted before executing requested tool `{tool_code}`."
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
                                "toolCode": tool_code,
                                "arguments": arguments,
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

                    let routed_call = match tool_router.route_tool_call(
                        &call_id,
                        &tool_code,
                        arguments.clone(),
                    ) {
                        Ok(routed_call) => routed_call,
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
                    };
                    let concurrency_policy_payload =
                        serde_json::to_value(&routed_call.tool.concurrency).unwrap_or(Value::Null);
                    let tool_code = routed_call.tool.code;
                    let arguments = routed_call.arguments;
                    runtime_state.push_item(AgentTurnItem::tool_call(
                        call_id.clone(),
                        tool_code.clone(),
                        arguments.clone(),
                    ));
                    let mut action_payload =
                        agent_turn_item_event_payload(&AgentTurnItem::tool_call(
                            call_id.clone(),
                            tool_code.clone(),
                            arguments.clone(),
                        ));
                    if let Some(object) = action_payload.as_object_mut() {
                        object.insert("runtimeMode".to_owned(), json!("model_loop"));
                        object.insert("concurrencyPolicy".to_owned(), concurrency_policy_payload);
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
                    let Some(tool) = self
                        .capability_repo
                        .find_tool_by_code(self.tenant_id, &tool_code)
                        .await?
                    else {
                        return Err(AppError::NotFound);
                    };
                    let policy = agent_tool_policy_decision(&tool, command.auto_approve);
                    if policy.requires_approval {
                        ensure_agent_run_transition(
                            &run_status_code(RunStatus::Running),
                            RunStatus::WaitingApproval,
                        )?;
                        self.update_status(AgentStatusUpdate {
                            user_id,
                            run_id,
                            status: run_status_code(RunStatus::WaitingApproval),
                            output_payload: json!({ "toolCode": tool.code }),
                            final_output: None,
                            pause_reason: policy.pause_reason.as_deref(),
                            finished: false,
                        })
                        .await?;
                        self.pause_for_approval(user_id, run_id, &tool, &command.input, now)
                            .await?;
                        self.refresh_trace_snapshot(
                            user_id,
                            run_id,
                            json!({ "runtimeMode": "model_loop", "pauseReason": "approval" }),
                        )
                        .await?;
                        return self.get_run(run_id).await;
                    } else {
                        let recorded = self
                            .execute_and_record_tool_call(user_id, run_id, &tool, arguments.clone())
                            .await?;
                        last_tool_terminal_status = recorded.terminal_status;
                        let observation_status = if recorded.execution.succeeded_status() {
                            ToolObservationStatus::Succeeded
                        } else {
                            ToolObservationStatus::Failed
                        };
                        let observation_item = AgentTurnItem::tool_observation(
                            &call_id,
                            observation_status,
                            recorded.execution.response_payload.clone(),
                        );
                        runtime_state.push_item(observation_item.clone());
                        let mut observation_payload =
                            agent_turn_item_event_payload(&observation_item);
                        if let Some(object) = observation_payload.as_object_mut() {
                            object.insert("toolCode".to_owned(), json!(&tool.code));
                            object.insert("auditId".to_owned(), json!(recorded.audit_id));
                            object.insert("dryRun".to_owned(), json!(recorded.execution.dry_run));
                            object.insert("runtimeMode".to_owned(), json!("model_loop"));
                        }
                        self.append_event(
                            user_id,
                            run_id,
                            Some(recorded.step_id),
                            RunEventKind::ToolCalled,
                            run_status_code(RunStatus::Running),
                            json!({
                                "toolCode": tool.code,
                                "arguments": arguments.clone(),
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

                        if runtime_state.should_compact_context() {
                            if let Some(compaction) = runtime_state.compact_context() {
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
                                }
                                self.append_event(
                                    user_id,
                                    run_id,
                                    Some(recorded.step_id),
                                    RunEventKind::Observation,
                                    run_status_code(RunStatus::Running),
                                    compaction_payload,
                                )
                                .await?;
                                let summary = compaction.summary.as_str();
                                messages = build_compacted_model_loop_messages(
                                    &command.input,
                                    summary,
                                    &tool_codes,
                                );
                                continue;
                            }
                        }

                        messages.push(ModelChatMessage {
                            role: "assistant".to_owned(),
                            content: model_response.answer.clone(),
                        });
                        messages.push(ModelChatMessage {
                            role: "user".to_owned(),
                            content: build_observation_follow_up_prompt(
                                &tool.code,
                                &recorded.execution.response_payload,
                            ),
                        });
                        continue;
                    }
                }
                _ => {
                    runtime_state.push_item(parsed.item);
                    self.finish_model_loop_run(
                        user_id,
                        run_id,
                        None,
                        last_tool_terminal_status,
                        &model_response.answer,
                        json!({ "answer": model_response.answer.clone(), "runtimeMode": "model_loop" }),
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

    pub async fn get_run_trace(&self, run_id: i64) -> Result<AgentTraceReplayResp, AppError> {
        let Some(run) = self.repo.find_run(self.tenant_id, run_id).await? else {
            return Err(AppError::NotFound);
        };
        if let Some(rollout) = self
            .repo
            .find_rollout_by_run_id(self.tenant_id, run_id)
            .await?
        {
            if let Ok(bundle) = serde_json::from_value::<TraceBundle>(rollout.event_bundle) {
                return Ok(AgentTraceReplayResp::from(bundle));
            }
        }
        let filter = RunEventFilter {
            tenant_id: self.tenant_id,
            run_id,
            limit: MAX_TRACE_REPLAY_EVENTS,
            offset: 0,
        };
        let events = self.repo.list_events(&filter).await?;

        Ok(AgentTraceReplayResp::from(agent_events_to_trace_bundle(
            run.trace_id,
            events,
        )))
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

        self.execute_tool_and_finish(user_id, run_id, &tool, command.input)
            .await?;
        self.refresh_trace_snapshot(user_id, run_id, json!({ "resumed": true }))
            .await?;
        self.get_run(run_id).await
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
        let now = Utc::now().naive_utc();
        self.repo
            .cancel_active_pauses(self.tenant_id, run_id, user_id, now)
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
            json!({ "cancelled": true }),
        )
        .await?;
        self.refresh_trace_snapshot(user_id, run_id, json!({ "cancelled": true }))
            .await?;
        self.get_run(run_id).await
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
        self.repo
            .create_run(&RunSaveRecord {
                id: run_id,
                tenant_id: self.tenant_id,
                run_type: "agent".to_owned(),
                status: run_status_code(RunStatus::Running),
                source_type: "admin".to_owned(),
                source_id: Some(user_id.to_string()),
                trace_id: trace_id.to_owned(),
                input_payload: json!({ "input": command.input }),
                output_payload: Value::Null,
                budget_policy: serde_json::to_value(plan.task_budget).unwrap_or(Value::Null),
                created_by: user_id,
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
                status: run_status_code(RunStatus::Running),
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
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        let step_id = next_id();
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
            json!({ "toolCode": tool.code, "riskLevel": tool.risk_level }),
        )
        .await?;
        self.append_event(
            user_id,
            run_id,
            Some(step_id),
            RunEventKind::ApprovalRequested,
            run_status_code(RunStatus::WaitingApproval),
            json!({ "toolCode": tool.code, "permissionCode": tool.permission_code }),
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
        let now = Utc::now().naive_utc();
        let audit_id = next_id();
        let connector_credential = if is_github_connector_tool(&tool.code) {
            self.capability_repo
                .find_connector_credential(self.tenant_id, GITHUB_CONNECTOR_CODE, user_id)
                .await?
        } else {
            None
        };
        let mcp_tool = if matches!(agent_tool_kind(tool), ToolKind::Mcp) {
            self.capability_repo
                .find_mcp_tool_for_execution(self.tenant_id, &tool.code)
                .await?
        } else {
            None
        };
        let execution = execute_agent_tool(
            tool,
            &input,
            connector_credential.as_ref(),
            mcp_tool.as_ref(),
            Some(&self.model_runtime),
        )
        .await;
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
        self.repo
            .create_event(&RunEventSaveRecord {
                id: next_id(),
                tenant_id: self.tenant_id,
                run_id,
                step_id,
                event_type: event_kind_code(event_type),
                sequence_no,
                status,
                payload,
                user_id,
                now: Utc::now().naive_utc(),
            })
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

async fn execute_agent_tool(
    tool: &ToolLookupRecord,
    input: &Value,
    connector_credential: Option<&ConnectorCredentialLookupRecord>,
    mcp_tool: Option<&McpToolExecutionRecord>,
    model_runtime: Option<&ModelRuntimeService>,
) -> AgentToolExecution {
    if matches!(agent_tool_kind(tool), ToolKind::Mcp) {
        return execute_mcp_tool(&tool.code, input, mcp_tool).await;
    }
    let tool_code = tool.code.as_str();
    if tool_code == FEISHU_TOOL_CODE {
        return execute_feishu_message_tool(input).await;
    }
    if tool_code == MEDIA_IMAGE_TOOL_CODE {
        return execute_media_image_tool(input, model_runtime).await;
    }
    if tool_code == GITHUB_REPO_SEARCH_TOOL_CODE {
        return execute_github_repo_search_tool(input, connector_credential).await;
    }
    if tool_code == GITHUB_REPO_READ_TOOL_CODE {
        return execute_github_repo_read_tool(input, connector_credential).await;
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
    AgentToolExecution::succeeded(
        json!({
            "dryRun": result.dry_run,
            "toolCode": result.tool_code,
            "status": result.status,
            "provider": "mcp",
            "server": mcp_server_payload(tool),
            "request": request,
            "response": result.output,
            "auth": auth,
            "mocked": false,
        }),
        result.dry_run,
        format!("Agent dry-run prepared MCP tool {}.", tool.tool_code),
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

fn mcp_auth_payload(secret_ref: Option<&str>, auth_type: &str) -> Value {
    mcp_auth_payload_from_sources(secret_ref, auth_type, |key| env::var(key).ok())
}

fn mcp_auth_payload_from_sources<F>(
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

    let client = match reqwest::Client::builder()
        .timeout(MEDIA_IMAGE_TIMEOUT)
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            let error = format!("图片生成客户端初始化失败: {err}");
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

    let response = match client
        .post(&endpoint)
        .bearer_auth(route.api_key())
        .header("x-api-key", route.api_key())
        .json(&request_payload)
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            let error = format!("图片生成请求失败: {err}");
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

    let status = response.status();
    let provider_payload = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !status.is_success() {
        let error = format!("图片生成请求失败: HTTP {}", status.as_u16());
        return AgentToolExecution::failed(
            json!({
                "dryRun": false,
                "toolCode": MEDIA_IMAGE_TOOL_CODE,
                "status": "failed",
                "provider": provider,
                "modelRoute": route_id,
                "model": model,
                "requestPayload": request_payload,
                "response": provider_payload,
                "error": error,
            }),
            error,
            "Agent failed to generate image.".to_owned(),
        );
    }

    let Some(result) = parse_media_image_generation_response(&provider_payload) else {
        let error = "图片生成响应缺少资产 URL".to_owned();
        return AgentToolExecution::failed(
            json!({
                "dryRun": false,
                "toolCode": MEDIA_IMAGE_TOOL_CODE,
                "status": "failed",
                "provider": provider,
                "modelRoute": route_id,
                "model": model,
                "requestPayload": request_payload,
                "response": provider_payload,
                "error": error,
            }),
            error,
            "Agent failed to generate image.".to_owned(),
        );
    };

    AgentToolExecution::succeeded(
        json!({
            "dryRun": false,
            "toolCode": MEDIA_IMAGE_TOOL_CODE,
            "status": "succeeded",
            "provider": provider,
            "modelRoute": route_id,
            "model": model,
            "assetUrl": result.asset_url,
            "providerAssetId": result.provider_asset_id,
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

fn feishu_message_text_from_tool_input(input: &Value) -> String {
    non_empty_json_string(input.get("message"))
        .or_else(|| non_empty_json_string(input.get("text")))
        .or_else(|| non_empty_json_string(input.get("input")))
        .unwrap_or_else(|| "Novex notification".to_owned())
}

fn media_image_request_from_tool_input(input: &Value) -> MediaImageGenerationRequest {
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

fn github_search_request_from_tool_input(input: &Value) -> Option<GitHubCodeSearchRequest> {
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

fn github_read_request_from_tool_input(input: &Value) -> Option<GitHubFileReadRequest> {
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

fn is_github_connector_tool(tool_code: &str) -> bool {
    matches!(
        tool_code,
        GITHUB_REPO_SEARCH_TOOL_CODE | GITHUB_REPO_READ_TOOL_CODE
    )
}

fn github_connector_auth(
    credential: Option<&ConnectorCredentialLookupRecord>,
) -> Option<GitHubConnectorAuth> {
    github_connector_auth_from_sources(credential, |key| env::var(key).ok())
}

fn github_connector_auth_from_sources<F>(
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

fn feishu_response_code(value: &Value) -> Option<i64> {
    value
        .get("code")
        .or_else(|| value.get("StatusCode"))
        .and_then(Value::as_i64)
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

fn agent_tool_kind(tool: &ToolLookupRecord) -> ToolKind {
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

pub fn normalize_agent_run_command(
    mut command: AgentRunCommand,
) -> Result<AgentRunCommand, AppError> {
    command.input = command.input.trim().to_owned();
    if command.input.is_empty() {
        return Err(AppError::bad_request("Agent 输入不能为空"));
    }
    ensure_max_chars("Agent 输入", &command.input, 4000)?;
    command.runtime_mode = normalize_agent_runtime_mode(command.runtime_mode)?;
    command.budget = novex_ai_core::normalize_task_budget(command.budget)
        .map_err(|err| AppError::bad_request(format!("任务预算超出限制: {}", err.field)))?;
    Ok(command)
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

fn build_model_loop_tool_router() -> Result<ToolRouter, ToolRouteError> {
    ToolRouter::from_definitions(agent_model_loop_tool_definitions())
}

fn build_model_loop_system_prompt(tool_codes: &[String]) -> String {
    format!(
        "You are Novex Agent Runtime. You may answer directly or request tool calls while staying within the run budget. Available tools: {}. After each tool observation, decide whether another tool call is necessary or produce the final answer. To call a tool, reply with compact JSON exactly like {{\"type\":\"tool_call\",\"callId\":\"call-1\",\"toolCode\":\"rag.search\",\"arguments\":{{\"query\":\"...\"}}}}. Otherwise reply with the final answer. Never request a tool outside the available tools or after the tool-call budget is exhausted.",
        tool_codes.join(", ")
    )
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

fn build_observation_follow_up_prompt(tool_code: &str, observation: &Value) -> String {
    format!(
        "Tool `{tool_code}` returned this observation:\n{}\nUse it to produce the final answer. If the observation is insufficient, say what is missing.",
        serde_json::to_string_pretty(observation).unwrap_or_else(|_| "{}".to_owned())
    )
}

fn build_compacted_model_loop_messages(
    original_input: &str,
    summary: &str,
    tool_codes: &[String],
) -> Vec<ModelChatMessage> {
    vec![
        ModelChatMessage {
            role: "system".to_owned(),
            content: build_model_loop_system_prompt(tool_codes),
        },
        ModelChatMessage {
            role: "user".to_owned(),
            content: original_input.to_owned(),
        },
        ModelChatMessage {
            role: "user".to_owned(),
            content: format!(
                "Prior agent context was compacted to keep the run inside the model context window:\n{summary}\nContinue from this compacted context. You may call another available tool if needed, otherwise produce the final answer."
            ),
        },
    ]
}

fn tool_route_error_to_app_error(err: ToolRouteError) -> AppError {
    AppError::bad_request(format!("Agent 工具路由初始化失败: {}", err.message))
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
        "observation" => Some(TraceEvent::observation(
            sequence_no,
            trace_call_id(event),
            trace_observation_output(&event.payload),
        )),
        "approval_requested" => Some(TraceEvent::approval_requested(
            sequence_no,
            trace_payload_text(&event.payload, &["toolCode", "tool_code"])
                .unwrap_or_else(|| "unknown".to_owned()),
        )),
        "final_output" => Some(TraceEvent::final_answer(
            sequence_no,
            trace_payload_text(&event.payload, &["answer", "content"])
                .unwrap_or_else(|| trace_payload_fallback(&event.payload)),
        )),
        "error" => Some(TraceEvent::error(
            sequence_no,
            trace_payload_text(&event.payload, &["message", "error"])
                .unwrap_or_else(|| trace_payload_fallback(&event.payload)),
        )),
        _ => None,
    }
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
        let summary = bundle.replay_summary();
        Self {
            trace_id: bundle.trace_id,
            events: bundle.events,
            summary,
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
    use novex_ai_core::TaskBudget;
    use sqlx::postgres::PgPoolOptions;

    #[tokio::test]
    async fn agent_service_can_be_bound_to_request_tenant() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let service = AgentService::for_tenant(db, 42);

        assert_eq!(service.tenant_id, 42);
    }

    #[test]
    fn agent_runtime_rejects_blank_run_input() {
        let err = normalize_agent_run_command(AgentRunCommand {
            input: "   ".to_owned(),
            runtime_mode: None,
            auto_approve: false,
            budget: TaskBudget::default(),
        })
        .unwrap_err();

        assert!(err.to_string().contains("Agent 输入不能为空"));
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
    fn agent_run_command_accepts_model_runtime_mode() {
        let command: AgentRunCommand = serde_json::from_value(serde_json::json!({
            "input": "search policy",
            "runtimeMode": "model_loop"
        }))
        .unwrap();

        assert_eq!(command.runtime_mode.as_deref(), Some("model_loop"));
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
    fn agent_service_model_loop_records_tool_concurrency_policy() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("\"concurrencyPolicy\""));
        assert!(source.contains("serde_json::to_value(&routed_call.tool.concurrency"));
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

        assert!(source.contains("runtime_state.should_compact_context()"));
        assert!(source.contains("runtime_state.compact_context()"));
        assert!(source.contains("AgentTurnItem::ContextCompaction"));
        assert!(source.contains("\"compactionWindowId\""));
    }

    #[test]
    fn agent_service_model_loop_uses_compacted_messages_for_next_sample() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let normalized_source = source.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(source.contains("build_compacted_model_loop_messages"));
        assert!(normalized_source.contains("messages = build_compacted_model_loop_messages"));
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
    fn compacted_model_loop_messages_preserve_prompt_input_and_summary() {
        let tool_codes = build_model_loop_tool_router().unwrap().tool_codes();
        let messages = build_compacted_model_loop_messages(
            "Find refund policy",
            "Observation for call-1: refund within 7 days",
            &tool_codes,
        );

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("Novex Agent Runtime"));
        assert!(messages[0].content.contains("github.repo.read"));
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[1].content, "Find refund policy");
        assert_eq!(messages[2].role, "user");
        assert!(messages[2].content.contains("refund within 7 days"));
        assert!(messages[2]
            .content
            .contains("Continue from this compacted context"));
    }

    #[test]
    fn observation_prompt_includes_tool_result_and_final_answer_instruction() {
        let prompt = build_observation_follow_up_prompt(
            "rag.search",
            &serde_json::json!({"hits":[{"title":"Policy"}]}),
        );

        assert!(prompt.contains("rag.search"));
        assert!(prompt.contains("Policy"));
        assert!(prompt.contains("final answer"));
    }

    #[test]
    fn agent_runtime_low_risk_tool_can_finish_without_approval() {
        let command = normalize_agent_run_command(AgentRunCommand {
            input: "search the training handbook".to_owned(),
            runtime_mode: None,
            auto_approve: false,
            budget: TaskBudget {
                max_steps: Some(6),
                max_tool_calls: Some(1),
                max_seconds: Some(30),
                max_cost_cents: Some(0),
            },
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
            auto_approve: false,
            budget: TaskBudget {
                max_steps: Some(6),
                max_tool_calls: Some(1),
                max_seconds: Some(30),
                max_cost_cents: Some(0),
            },
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
            auto_approve: false,
            budget: TaskBudget::default(),
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
    fn agent_runtime_routes_mcp_tools_through_audited_observation_path() {
        let source = include_str!("agent_service.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("execute_mcp_tool"));
        assert!(source.contains("ToolKind::Mcp"));
        assert!(source.contains("RunEventKind::Observation"));
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
        let source = include_str!("agent_service.rs");
        assert!(source.contains("ModelRuntimeService::for_tenant(db.clone(), tenant_id)"));
        assert!(source.contains("resolve_route_for_purpose(ModelRoutePurpose::MediaGeneration)"));
        let static_env_config = ["ModelRuntimeConfig", "::from_env()"].concat();
        let static_draw_persistence = ["then(|| ", "\"runtime.draw\".to_owned())"].concat();
        assert!(!source.contains(&static_env_config));
        assert!(!source.contains(&static_draw_persistence));
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
    fn agent_runtime_tool_budget_rejects_tool_plan_when_zero_tool_calls_allowed() {
        let err = normalize_agent_run_command(AgentRunCommand {
            input: "search the training handbook".to_owned(),
            runtime_mode: None,
            auto_approve: false,
            budget: TaskBudget {
                max_steps: Some(6),
                max_tool_calls: Some(0),
                max_seconds: Some(30),
                max_cost_cents: Some(0),
            },
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
