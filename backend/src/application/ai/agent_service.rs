use chrono::{NaiveDateTime, Utc};
use novex_agent::{plan_react_run, AgentIntent, AgentLoopKind};
use novex_ai_core::{RunEventKind, RunStatus, RunStepType, TaskBudget};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::{
    application::system::{ensure_max_chars, format_datetime},
    infrastructure::persistence::{
        ai_agent_repository::{
            AgentRunFilter, AgentRunRecord, AgentRunSaveRecord, AgentRunStatusUpdate,
            AgentTraceSaveRecord, AiAgentRepository, RunEventFilter, RunEventRecord,
            RunEventSaveRecord, RunPauseSaveRecord, RunSaveRecord, RunStatusUpdate,
            RunStepSaveRecord,
        },
        ai_capability_repository::{AiCapabilityRepository, ToolAuditSaveRecord, ToolLookupRecord},
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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunCommand {
    #[serde(default)]
    pub input: String,
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

#[derive(Debug, Clone)]
pub struct AgentService {
    repo: AiAgentRepository,
    capability_repo: AiCapabilityRepository,
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
}

impl AgentService {
    pub fn new(db: PgPool) -> Self {
        Self {
            repo: AiAgentRepository::new(db.clone()),
            capability_repo: AiCapabilityRepository::new(db),
        }
    }

    pub async fn create_run(
        &self,
        user_id: i64,
        command: AgentRunCommand,
    ) -> Result<AgentRunResp, AppError> {
        let command = normalize_agent_run_command(command)?;
        let mut plan = build_agent_plan(&command)?;
        let selected_tool = if let Some(tool_code) = plan.selected_tool_code.as_deref() {
            let Some(tool) = self
                .capability_repo
                .find_tool_by_code(DEFAULT_TENANT_ID, tool_code)
                .await?
            else {
                return Err(AppError::NotFound);
            };
            plan.requires_approval = tool.risk_level >= 2 && !command.auto_approve;
            plan.pause_reason = plan.requires_approval.then(|| "approval".to_owned());
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
        self.append_event(
            user_id,
            run_id,
            None,
            RunEventKind::InputReceived,
            run_status_code(RunStatus::Running),
            json!({ "input": command.input }),
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

    pub async fn list_runs(
        &self,
        query: AgentRunQuery,
    ) -> Result<PageResult<AgentRunResp>, AppError> {
        let page = query.page_query();
        let filter = AgentRunFilter {
            tenant_id: DEFAULT_TENANT_ID,
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
        let Some(record) = self.repo.find_run(DEFAULT_TENANT_ID, run_id).await? else {
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
            tenant_id: DEFAULT_TENANT_ID,
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
        if run.status != run_status_code(RunStatus::WaitingApproval)
            && run.status != run_status_code(RunStatus::Paused)
        {
            return Err(AppError::conflict("当前 Run 不可恢复"));
        }
        let Some(pause) = self
            .repo
            .find_active_pause(DEFAULT_TENANT_ID, run_id)
            .await?
        else {
            return Err(AppError::NotFound);
        };
        let now = Utc::now().naive_utc();
        self.repo
            .complete_pause(
                DEFAULT_TENANT_ID,
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
        let Some(tool_code) = run.selected_tool_code.as_deref() else {
            return Err(AppError::bad_request("恢复 Run 缺少工具上下文"));
        };
        let Some(tool) = self
            .capability_repo
            .find_tool_by_code(DEFAULT_TENANT_ID, tool_code)
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
        if is_terminal_status(&run.status) {
            return Err(AppError::conflict("当前 Run 已终止"));
        }
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
            .cancel_active_pauses(DEFAULT_TENANT_ID, run_id, user_id, now)
            .await?;
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
                tenant_id: DEFAULT_TENANT_ID,
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
                tenant_id: DEFAULT_TENANT_ID,
                run_id,
                intent: plan.intent.clone(),
                loop_kind: plan.loop_kind.clone(),
                selected_tool_code: plan.selected_tool_code.clone(),
                status: run_status_code(RunStatus::Running),
                pause_reason: None,
                task_budget: serde_json::to_value(plan.task_budget).unwrap_or(Value::Null),
                metadata: json!({ "milestone": "M3", "poc": true }),
                user_id,
                now,
            })
            .await?;
        self.repo
            .create_agent_trace(&AgentTraceSaveRecord {
                id: next_id(),
                tenant_id: DEFAULT_TENANT_ID,
                run_id,
                trace_id: trace_id.to_owned(),
                event_snapshot: json!([]),
                model_route_snapshot: json!({ "mode": "deterministic", "model": "none" }),
                tool_snapshot: json!({}),
                metadata: json!({ "milestone": "M3" }),
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
                tenant_id: DEFAULT_TENANT_ID,
                run_id,
                parent_step_id: None,
                step_type: step_type_code(RunStepType::Approval),
                status: run_status_code(RunStatus::WaitingApproval),
                sequence_no: self
                    .repo
                    .next_event_sequence(DEFAULT_TENANT_ID, run_id)
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
                tenant_id: DEFAULT_TENANT_ID,
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

    async fn execute_tool_and_finish(
        &self,
        user_id: i64,
        run_id: i64,
        tool: &ToolLookupRecord,
        input: Value,
    ) -> Result<(), AppError> {
        let now = Utc::now().naive_utc();
        let audit_id = next_id();
        let response_payload = json!({
            "dryRun": true,
            "toolCode": tool.code,
            "status": "succeeded",
            "inputEcho": input,
            "message": "agent dry-run only; no external side effects"
        });
        self.capability_repo
            .create_tool_call_audit(&ToolAuditSaveRecord {
                id: audit_id,
                tenant_id: DEFAULT_TENANT_ID,
                tool_id: tool.id,
                tool_code: tool.code.clone(),
                caller_kind: "agent_run".to_owned(),
                caller_id: Some(run_id),
                request_payload: json!({
                    "runId": run_id,
                    "toolCode": tool.code,
                    "input": response_payload["inputEcho"].clone()
                }),
                response_payload: response_payload.clone(),
                status: "succeeded".to_owned(),
                dry_run: true,
                risk_level: tool.risk_level,
                permission_code: tool.permission_code.clone(),
                error_message: None,
                user_id,
                now,
            })
            .await?;
        let step_id = next_id();
        self.repo
            .create_step(&RunStepSaveRecord {
                id: step_id,
                tenant_id: DEFAULT_TENANT_ID,
                run_id,
                parent_step_id: None,
                step_type: step_type_code(RunStepType::ToolCall),
                status: run_status_code(RunStatus::Succeeded),
                sequence_no: self
                    .repo
                    .next_event_sequence(DEFAULT_TENANT_ID, run_id)
                    .await?,
                input_payload: response_payload["inputEcho"].clone(),
                output_payload: response_payload.clone(),
                tool_call_audit_id: Some(audit_id),
                user_id,
                now,
            })
            .await?;
        self.append_event(
            user_id,
            run_id,
            Some(step_id),
            RunEventKind::ToolCalled,
            run_status_code(RunStatus::Running),
            json!({ "toolCode": tool.code, "auditId": audit_id }),
        )
        .await?;
        self.append_event(
            user_id,
            run_id,
            Some(step_id),
            RunEventKind::Observation,
            run_status_code(RunStatus::Running),
            response_payload.clone(),
        )
        .await?;
        let final_output = format!("Agent dry-run executed {}.", tool.code);
        self.update_status(AgentStatusUpdate {
            user_id,
            run_id,
            status: run_status_code(RunStatus::Succeeded),
            output_payload: json!({ "answer": final_output, "auditId": audit_id }),
            final_output: Some(&final_output),
            pause_reason: None,
            finished: true,
        })
        .await?;
        self.append_event(
            user_id,
            run_id,
            Some(step_id),
            RunEventKind::FinalOutput,
            run_status_code(RunStatus::Succeeded),
            json!({ "answer": final_output }),
        )
        .await?;
        self.refresh_trace_snapshot(
            user_id,
            run_id,
            json!({ "toolCode": tool.code, "auditId": audit_id }),
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
                tenant_id: DEFAULT_TENANT_ID,
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
                tenant_id: DEFAULT_TENANT_ID,
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
            .next_event_sequence(DEFAULT_TENANT_ID, run_id)
            .await?;
        self.repo
            .create_event(&RunEventSaveRecord {
                id: next_id(),
                tenant_id: DEFAULT_TENANT_ID,
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
            tenant_id: DEFAULT_TENANT_ID,
            run_id,
            limit: DEFAULT_EVENT_PAGE_SIZE as i64,
            offset: 0,
        };
        let events: Vec<AgentRunEventResp> = self
            .repo
            .list_events(&filter)
            .await?
            .into_iter()
            .map(AgentRunEventResp::from)
            .collect();
        self.repo
            .update_trace_snapshot(
                DEFAULT_TENANT_ID,
                run_id,
                &serde_json::to_value(events).unwrap_or_else(|_| json!([])),
                &tool_snapshot,
                user_id,
                Utc::now().naive_utc(),
            )
            .await
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
    command.budget = novex_ai_core::normalize_task_budget(command.budget)
        .map_err(|err| AppError::bad_request(format!("任务预算超出限制: {}", err.field)))?;
    Ok(command)
}

fn build_agent_plan(command: &AgentRunCommand) -> Result<AgentPlanSummary, AppError> {
    let plan = plan_react_run(&command.input, command.budget)
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
    })
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

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "cancelled" | "failed" | "succeeded")
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

    #[test]
    fn agent_runtime_rejects_blank_run_input() {
        let err = normalize_agent_run_command(AgentRunCommand {
            input: "   ".to_owned(),
            auto_approve: false,
            budget: TaskBudget::default(),
        })
        .unwrap_err();

        assert!(err.to_string().contains("Agent 输入不能为空"));
    }

    #[test]
    fn agent_runtime_low_risk_tool_can_finish_without_approval() {
        let command = normalize_agent_run_command(AgentRunCommand {
            input: "search the training handbook".to_owned(),
            auto_approve: false,
            budget: TaskBudget {
                max_steps: Some(6),
                max_tool_calls: Some(1),
                max_seconds: Some(30),
                max_cost_cents: Some(0),
            },
        })
        .unwrap();
        let plan = build_agent_plan(&command).unwrap();

        assert_eq!(plan.selected_tool_code.as_deref(), Some("rag.search"));
        assert!(!plan.requires_approval);
        assert_eq!(plan.initial_status, "running");
    }

    #[test]
    fn agent_runtime_medium_risk_tool_pauses_without_auto_approval() {
        let command = normalize_agent_run_command(AgentRunCommand {
            input: "send a Feishu reminder".to_owned(),
            auto_approve: false,
            budget: TaskBudget {
                max_steps: Some(6),
                max_tool_calls: Some(1),
                max_seconds: Some(30),
                max_cost_cents: Some(0),
            },
        })
        .unwrap();
        let plan = build_agent_plan(&command).unwrap();

        assert_eq!(
            plan.selected_tool_code.as_deref(),
            Some("feishu.message.send")
        );
        assert!(plan.requires_approval);
        assert_eq!(plan.pause_reason.as_deref(), Some("approval"));
    }

    #[test]
    fn agent_runtime_tool_budget_rejects_tool_plan_when_zero_tool_calls_allowed() {
        let err = normalize_agent_run_command(AgentRunCommand {
            input: "search the training handbook".to_owned(),
            auto_approve: false,
            budget: TaskBudget {
                max_steps: Some(6),
                max_tool_calls: Some(0),
                max_seconds: Some(30),
                max_cost_cents: Some(0),
            },
        })
        .and_then(|command| build_agent_plan(&command).map(|_| command))
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
