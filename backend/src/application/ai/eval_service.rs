use chrono::Utc;
#[cfg(test)]
use novex_eval::{
    score_case, score_cost_case, score_customer_service_grounded_resolution_case,
    score_customer_service_handoff_accuracy_case, score_intent_case, score_latency_case,
    score_rag_case, score_retrieval_recall_case, score_tool_case, EvalCaseScore,
};
use novex_eval::{
    EvalCaseActual, EvalCaseCandidate, EvalCaseExpected, EvalMetricKind, EvalTargetKind,
};
use novex_trace::TraceBundle;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::{
    application::system::{ensure_max_chars, format_datetime},
    infrastructure::persistence::ai_agent_repository::AiAgentRepository,
    infrastructure::persistence::ai_eval_repository::{
        AiEvalRepository, EvalCaseFilter, EvalCaseRecord, EvalCaseSaveRecord, EvalDatasetFilter,
        EvalDatasetRecord, EvalOutboxSaveRecord, EvalResultFilter, EvalResultRecord, EvalRunFilter,
        EvalRunRecord, EvalRunSaveRecord, EvalTaskSaveRecord,
    },
    shared::{
        error::AppError,
        id::next_id,
        pagination::{PageQuery, PageResult, DEFAULT_PAGE},
    },
};

const DEFAULT_TENANT_ID: i64 = 1;
const DEFAULT_EVAL_PAGE_SIZE: u64 = 20;
const DEFAULT_CASE_PAGE_SIZE: u64 = 100;
const ENABLED_STATUS: i16 = 1;
const DISABLED_STATUS: i16 = 0;
const EVAL_RUN_MODE_DETERMINISTIC: &str = "deterministic";
const EVAL_RUN_MODE_LIVE_RAG: &str = "live_rag";
const EVAL_RUN_MODE_TRACE_REPLAY: &str = "trace_replay";
const EVAL_RUN_STATUS_QUEUED: &str = "queued";
const EVAL_TASK_STATUS_QUEUED: &str = "queued";
const EVAL_TASK_REQUESTED_EVENT: &str = "eval.task.requested";
const DEFAULT_TASK_MAX_ATTEMPTS: i32 = 3;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalDatasetQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_eval_size")]
    pub size: u64,
    #[serde(default = "default_enabled_status")]
    pub status: Option<i16>,
    #[serde(default)]
    pub code: Option<String>,
}

impl Default for EvalDatasetQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            size: DEFAULT_EVAL_PAGE_SIZE,
            status: Some(ENABLED_STATUS),
            code: None,
        }
    }
}

impl EvalDatasetQuery {
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
pub struct EvalCaseQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_case_size")]
    pub size: u64,
    #[serde(default = "default_enabled_status")]
    pub status: Option<i16>,
    #[serde(default)]
    pub target_kind: Option<String>,
}

impl Default for EvalCaseQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            size: DEFAULT_CASE_PAGE_SIZE,
            status: Some(ENABLED_STATUS),
            target_kind: None,
        }
    }
}

impl EvalCaseQuery {
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
pub struct EvalRunQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_eval_size")]
    pub size: u64,
    #[serde(default)]
    pub dataset_code: Option<String>,
}

impl Default for EvalRunQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            size: DEFAULT_EVAL_PAGE_SIZE,
            dataset_code: None,
        }
    }
}

impl EvalRunQuery {
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
pub struct EvalResultQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_case_size")]
    pub size: u64,
}

impl Default for EvalResultQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            size: DEFAULT_CASE_PAGE_SIZE,
        }
    }
}

impl EvalResultQuery {
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
pub struct EvalRunCommand {
    #[serde(default)]
    pub dataset_id: Option<i64>,
    #[serde(default)]
    pub dataset_code: String,
    #[serde(default, rename = "runMode")]
    pub run_mode: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalCaseCaptureCommand {
    #[serde(default)]
    pub dataset_id: Option<i64>,
    #[serde(default)]
    pub dataset_code: String,
    #[serde(default = "default_capture_dry_run")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalDatasetResp {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub target_scope: String,
    pub status: i16,
    pub metadata: Value,
    pub case_count: i64,
    pub create_time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalCaseResp {
    pub id: i64,
    pub dataset_id: i64,
    pub case_code: String,
    pub target_kind: String,
    pub metric_kind: String,
    pub prompt: String,
    pub expected_payload: Value,
    pub tags: Value,
    pub status: i16,
    pub sort: i32,
    pub create_time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalRunResp {
    pub run_id: i64,
    pub dataset_id: i64,
    pub dataset_code: String,
    pub status: String,
    pub total_cases: i32,
    pub passed_cases: i32,
    pub failed_cases: i32,
    pub average_score: f64,
    pub metric_breakdown: Value,
    pub report_payload: Value,
    pub create_time: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalResultResp {
    pub id: i64,
    pub run_id: i64,
    pub case_id: i64,
    pub case_code: String,
    pub target_kind: String,
    pub metric_kind: String,
    pub score: f64,
    pub passed: bool,
    pub expected_payload: Value,
    pub actual_payload: Value,
    pub reason: String,
    pub cost_cents: i32,
    pub latency_ms: i32,
    pub create_time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalCaseCaptureResp {
    pub dry_run: bool,
    pub case_id: Option<i64>,
    pub case_code: String,
    pub candidate: EvalCaseCandidate,
}

#[derive(Debug, Clone)]
pub struct EvalService {
    tenant_id: i64,
    repo: AiEvalRepository,
    agent_repo: AiAgentRepository,
}

impl EvalService {
    pub fn new(db: PgPool) -> Self {
        Self::for_tenant(db, DEFAULT_TENANT_ID)
    }

    pub fn for_tenant(db: PgPool, tenant_id: i64) -> Self {
        Self {
            tenant_id,
            repo: AiEvalRepository::new(db.clone()),
            agent_repo: AiAgentRepository::new(db),
        }
    }

    pub async fn list_datasets(
        &self,
        query: EvalDatasetQuery,
    ) -> Result<PageResult<EvalDatasetResp>, AppError> {
        let page = query.page_query();
        let filter = EvalDatasetFilter {
            tenant_id: self.tenant_id,
            status: query.status,
            code: query.code.as_deref(),
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_datasets(&filter).await?;
        let list = self
            .repo
            .list_datasets(&filter)
            .await?
            .into_iter()
            .map(EvalDatasetResp::from)
            .collect();
        Ok(PageResult::new(list, total))
    }

    pub async fn list_cases(
        &self,
        dataset_id: i64,
        query: EvalCaseQuery,
    ) -> Result<PageResult<EvalCaseResp>, AppError> {
        let page = query.page_query();
        let filter = EvalCaseFilter {
            tenant_id: self.tenant_id,
            dataset_id,
            status: query.status,
            target_kind: query.target_kind.as_deref(),
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_cases(&filter).await?;
        let list = self
            .repo
            .list_cases(&filter)
            .await?
            .into_iter()
            .map(EvalCaseResp::from)
            .collect();
        Ok(PageResult::new(list, total))
    }

    pub async fn capture_case_from_agent_run(
        &self,
        user_id: i64,
        run_id: i64,
        command: EvalCaseCaptureCommand,
    ) -> Result<EvalCaseCaptureResp, AppError> {
        let command = normalize_eval_case_capture_command(command)?;
        let Some(rollout) = self
            .agent_repo
            .find_rollout_by_run_id(self.tenant_id, run_id)
            .await?
        else {
            return Err(AppError::NotFound);
        };
        let bundle = serde_json::from_value::<TraceBundle>(rollout.event_bundle)
            .map_err(|err| AppError::bad_request(format!("Agent trace bundle 格式错误: {err}")))?;
        let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);
        let case_code = eval_case_code_from_agent_run(run_id);
        if command.dry_run {
            return Ok(EvalCaseCaptureResp {
                dry_run: true,
                case_id: None,
                case_code,
                candidate,
            });
        }

        let Some(dataset) = self
            .repo
            .find_dataset_by_selector(
                self.tenant_id,
                command.dataset_id,
                Some(&command.dataset_code),
            )
            .await?
        else {
            return Err(AppError::NotFound);
        };
        let now = Utc::now().naive_utc();
        let case_id = self
            .repo
            .upsert_case(&EvalCaseSaveRecord {
                id: next_id(),
                tenant_id: self.tenant_id,
                dataset_id: dataset.id,
                case_code: case_code.clone(),
                target_kind: target_kind_code(candidate.target_kind),
                metric_kind: metric_code(candidate.metric_kind),
                prompt: candidate.prompt.clone(),
                expected_payload: serde_json::to_value(&candidate.expected)
                    .unwrap_or_else(|_| json!({})),
                tags: candidate.tags.clone(),
                status: DISABLED_STATUS,
                sort: 0,
                user_id,
                now,
            })
            .await?;

        Ok(EvalCaseCaptureResp {
            dry_run: false,
            case_id: Some(case_id),
            case_code,
            candidate,
        })
    }

    pub async fn run_eval(
        &self,
        user_id: i64,
        command: EvalRunCommand,
    ) -> Result<EvalRunResp, AppError> {
        let command = normalize_eval_run_command(command)?;
        let Some(dataset) = self
            .repo
            .find_dataset_by_selector(
                self.tenant_id,
                command.dataset_id,
                Some(&command.dataset_code),
            )
            .await?
        else {
            return Err(AppError::NotFound);
        };
        let case_filter = EvalCaseFilter {
            tenant_id: self.tenant_id,
            dataset_id: dataset.id,
            status: Some(ENABLED_STATUS),
            target_kind: None,
            limit: DEFAULT_CASE_PAGE_SIZE as i64,
            offset: 0,
        };
        let cases = self.repo.list_cases(&case_filter).await?;
        if cases.is_empty() {
            return Err(AppError::bad_request("评测集没有启用用例"));
        }

        let run_id = next_id();
        let now = Utc::now().naive_utc();
        let metric_breakdown = json!({});
        let mut report_payload =
            eval_report_payload(cases.len() as i32, 0, 0, 0.0, metric_breakdown.clone());
        attach_trace_gate_summary(&mut report_payload, &command, cases.len());
        let run = EvalRunSaveRecord {
            id: run_id,
            tenant_id: self.tenant_id,
            dataset_id: dataset.id,
            dataset_code: dataset.code.clone(),
            status: EVAL_RUN_STATUS_QUEUED.to_owned(),
            total_cases: cases.len() as i32,
            passed_cases: 0,
            failed_cases: 0,
            average_score: 0.0,
            metric_breakdown,
            report_payload,
            triggered_by: user_id,
            user_id,
            now,
        };
        let runtime_config = eval_task_runtime_config(&dataset, &command);
        let mut tasks = Vec::with_capacity(cases.len());
        let mut outbox_records = Vec::with_capacity(cases.len());
        for case in &cases {
            let task_id = next_id();
            tasks.push(EvalTaskSaveRecord {
                id: task_id,
                tenant_id: self.tenant_id,
                run_id,
                dataset_id: dataset.id,
                case_id: case.id,
                case_code: case.case_code.clone(),
                target_kind: case.target_kind.clone(),
                metric_kind: case.metric_kind.clone(),
                run_mode: command.run_mode.clone(),
                status: EVAL_TASK_STATUS_QUEUED.to_owned(),
                attempt: 0,
                max_attempts: DEFAULT_TASK_MAX_ATTEMPTS,
                input_snapshot: eval_case_input_snapshot(case),
                expected_snapshot: case.expected_payload.clone(),
                tags_snapshot: case.tags.clone(),
                runtime_config: runtime_config.clone(),
                user_id,
                now,
            });
            outbox_records.push(EvalOutboxSaveRecord {
                id: next_id(),
                tenant_id: self.tenant_id,
                run_id,
                task_id,
                event_type: EVAL_TASK_REQUESTED_EVENT.to_owned(),
                payload: eval_task_payload(
                    task_id,
                    run_id,
                    self.tenant_id,
                    case,
                    &command.run_mode,
                    DEFAULT_TASK_MAX_ATTEMPTS,
                ),
                user_id,
                now,
            });
        }
        self.repo
            .create_run_with_tasks_and_outbox(&run, &tasks, &outbox_records)
            .await?;

        self.get_run(run_id).await
    }

    pub async fn list_runs(
        &self,
        query: EvalRunQuery,
    ) -> Result<PageResult<EvalRunResp>, AppError> {
        let page = query.page_query();
        let filter = EvalRunFilter {
            tenant_id: self.tenant_id,
            dataset_code: query.dataset_code.as_deref(),
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_runs(&filter).await?;
        let list = self
            .repo
            .list_runs(&filter)
            .await?
            .into_iter()
            .map(EvalRunResp::from)
            .collect();
        Ok(PageResult::new(list, total))
    }

    pub async fn get_run(&self, run_id: i64) -> Result<EvalRunResp, AppError> {
        let Some(record) = self.repo.find_run(self.tenant_id, run_id).await? else {
            return Err(AppError::NotFound);
        };
        Ok(EvalRunResp::from(record))
    }

    pub async fn list_results(
        &self,
        run_id: i64,
        query: EvalResultQuery,
    ) -> Result<PageResult<EvalResultResp>, AppError> {
        let page = query.page_query();
        let filter = EvalResultFilter {
            tenant_id: self.tenant_id,
            run_id,
            limit: page.limit(),
            offset: page.offset(),
        };
        let total = self.repo.count_results(&filter).await?;
        let list = self
            .repo
            .list_results(&filter)
            .await?
            .into_iter()
            .map(EvalResultResp::from)
            .collect();
        Ok(PageResult::new(list, total))
    }
}

pub fn normalize_eval_run_command(mut command: EvalRunCommand) -> Result<EvalRunCommand, AppError> {
    command.dataset_code = command.dataset_code.trim().to_owned();
    command.run_mode = command.run_mode.trim().to_ascii_lowercase();
    if command.run_mode.is_empty() {
        command.run_mode = EVAL_RUN_MODE_DETERMINISTIC.to_owned();
    }
    if command.dataset_id.is_none() && command.dataset_code.is_empty() {
        return Err(AppError::bad_request("评测集不能为空"));
    }
    if !command.dataset_code.is_empty() {
        ensure_max_chars("评测集编码", &command.dataset_code, 128)?;
    }
    if !matches!(
        command.run_mode.as_str(),
        EVAL_RUN_MODE_DETERMINISTIC | EVAL_RUN_MODE_LIVE_RAG | EVAL_RUN_MODE_TRACE_REPLAY
    ) {
        return Err(AppError::bad_request("评测运行模式不合法"));
    }
    Ok(command)
}

pub fn normalize_eval_case_capture_command(
    mut command: EvalCaseCaptureCommand,
) -> Result<EvalCaseCaptureCommand, AppError> {
    command.dataset_code = command.dataset_code.trim().to_owned();
    if !command.dry_run && command.dataset_id.is_none() && command.dataset_code.is_empty() {
        return Err(AppError::bad_request("评测集不能为空"));
    }
    if !command.dataset_code.is_empty() {
        ensure_max_chars("评测集编码", &command.dataset_code, 128)?;
    }
    Ok(command)
}

#[cfg(test)]
fn eval_run_uses_live_rag(command: &EvalRunCommand) -> bool {
    command.run_mode == EVAL_RUN_MODE_LIVE_RAG
}

fn eval_run_uses_trace_replay(command: &EvalRunCommand) -> bool {
    command.run_mode == EVAL_RUN_MODE_TRACE_REPLAY
}

pub fn eval_task_payload(
    task_id: i64,
    run_id: i64,
    tenant_id: i64,
    case: &EvalCaseRecord,
    run_mode: &str,
    max_attempts: i32,
) -> Value {
    json!({
        "taskId": task_id,
        "runId": run_id,
        "tenantId": tenant_id,
        "caseId": case.id,
        "runMode": run_mode,
        "attempt": 0,
        "maxAttempts": max_attempts
    })
}

fn eval_case_input_snapshot(case: &EvalCaseRecord) -> Value {
    json!({
        "prompt": case.prompt,
        "caseCode": case.case_code,
        "targetKind": case.target_kind,
        "metricKind": case.metric_kind
    })
}

fn eval_task_runtime_config(dataset: &EvalDatasetRecord, command: &EvalRunCommand) -> Value {
    json!({
        "datasetId": dataset.id,
        "datasetCode": dataset.code,
        "datasetMetadata": dataset.metadata,
        "runMode": command.run_mode
    })
}

#[cfg(test)]
fn live_rag_knowledge_dataset_id(
    dataset_metadata: &Value,
    case_tags: &Value,
    expected_payload: &Value,
) -> Option<i64> {
    json_positive_i64(dataset_metadata, "knowledgeDatasetId")
        .or_else(|| json_positive_i64(dataset_metadata, "knowledge_dataset_id"))
        .or_else(|| json_positive_i64(case_tags, "knowledgeDatasetId"))
        .or_else(|| json_positive_i64(case_tags, "knowledge_dataset_id"))
        .or_else(|| json_positive_i64(expected_payload, "knowledgeDatasetId"))
        .or_else(|| json_positive_i64(expected_payload, "knowledge_dataset_id"))
}

#[cfg(test)]
fn json_positive_i64(value: &Value, key: &str) -> Option<i64> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
}

pub fn build_eval_actual(
    target_kind: &str,
    _expected: &EvalCaseExpected,
    prompt: &str,
) -> EvalCaseActual {
    match target_kind {
        "rag" => build_rag_eval_actual(prompt),
        "customer_service" => build_customer_service_eval_actual(prompt),
        "intent" => EvalCaseActual {
            intent: Some(classify_eval_intent(prompt)),
            latency_ms: 3,
            ..Default::default()
        },
        "tool" => EvalCaseActual {
            tool_code: Some(select_eval_tool(prompt)),
            latency_ms: 8,
            ..Default::default()
        },
        _ => EvalCaseActual {
            answer: Some(prompt.to_owned()),
            latency_ms: 5,
            ..Default::default()
        },
    }
}

fn build_customer_service_eval_actual(prompt: &str) -> EvalCaseActual {
    let lower = prompt.to_ascii_lowercase();
    if lower.contains("refund window") || lower.contains("refunds") {
        return EvalCaseActual {
            answer: Some("Refunds are available within 30 days.".to_owned()),
            citations: vec!["cs-faq:refunds".to_owned()],
            latency_ms: 18,
            ..Default::default()
        };
    }
    if lower.contains("custom warranty") || lower.contains("guarantee") {
        return EvalCaseActual {
            answer: Some("insufficient evidence".to_owned()),
            citations: Vec::new(),
            latency_ms: 17,
            ..Default::default()
        };
    }
    if lower.contains("human") || lower.contains("handoff") || lower.contains("angry") {
        return EvalCaseActual {
            answer: Some("I will request a human handoff.".to_owned()),
            intent: Some("human_handoff".to_owned()),
            tool_code: Some("handoff.request".to_owned()),
            latency_ms: 22,
            ..Default::default()
        };
    }
    if lower.contains("ticket") {
        return EvalCaseActual {
            answer: Some("Ticket creation requires approval before I can create it.".to_owned()),
            citations: vec!["cs-policy:approval".to_owned()],
            tool_code: Some("ticket.create".to_owned()),
            latency_ms: 21,
            ..Default::default()
        };
    }

    EvalCaseActual {
        answer: Some("insufficient evidence".to_owned()),
        citations: Vec::new(),
        latency_ms: 17,
        ..Default::default()
    }
}

fn build_rag_eval_actual(prompt: &str) -> EvalCaseActual {
    let lower = prompt.to_ascii_lowercase();
    let (answer, citations) = if lower.contains("training start") {
        (
            "Training starts on Monday.",
            vec!["training-handbook:0".to_owned()],
        )
    } else if lower.contains("hr policy") {
        (
            "The HR policy is described in the training handbook.",
            vec!["training-handbook:1".to_owned()],
        )
    } else if lower.contains("safety module") {
        (
            "The safety module is required.",
            vec!["training-handbook:2".to_owned()],
        )
    } else if lower.contains("reviews completion") {
        (
            "A manager reviews completion.",
            vec!["training-handbook:3".to_owned()],
        )
    } else if lower.contains("quiz questions") {
        (
            "The quiz generates 5 questions.",
            vec!["training-handbook:4".to_owned()],
        )
    } else if lower.contains("reminders sent") {
        (
            "Reminders are sent through Feishu.",
            vec!["training-handbook:5".to_owned()],
        )
    } else if lower.contains("weak") || lower.contains("inspect after the quiz") {
        (
            "HR inspects weak points after the quiz.",
            vec!["training-handbook:6".to_owned()],
        )
    } else if lower.contains("knowledge visibility") {
        (
            "RBAC limits knowledge visibility.",
            vec!["training-handbook:7".to_owned()],
        )
    } else if lower.contains("policy defined") || lower.contains("policy source") {
        (
            "The policy is defined in the knowledge handbook.",
            vec!["kb-handbook:0".to_owned()],
        )
    } else if lower.contains("faq") {
        (
            "The FAQ answers access requests.",
            vec!["kb-handbook:1".to_owned()],
        )
    } else if lower.contains("product") {
        (
            "Product setup is documented in the knowledge base.",
            vec!["kb-handbook:2".to_owned()],
        )
    } else if lower.contains("support") {
        (
            "Support escalation is described in the support runbook.",
            vec!["kb-handbook:3".to_owned()],
        )
    } else {
        (prompt, Vec::new())
    };

    EvalCaseActual {
        answer: Some(answer.to_owned()),
        citations,
        latency_ms: 12,
        ..Default::default()
    }
}

fn classify_eval_intent(prompt: &str) -> String {
    let lower = prompt.to_ascii_lowercase();
    if lower.contains("quiz") {
        "training_quiz"
    } else if lower.contains("approval") || lower.contains("approve") || lower.contains("human") {
        "human_handoff"
    } else if lower.contains("refund")
        || lower.contains("ticket")
        || lower.contains("warranty")
        || lower.contains("customer")
    {
        "customer_service"
    } else if lower.contains("github")
        || lower.contains("repository")
        || lower.contains("repo")
        || lower.contains("code")
        || lower.contains("parser")
    {
        "code_search"
    } else if lower.contains("feishu")
        || lower.contains("send")
        || lower.contains("notify")
        || lower.contains("bounded agent task")
    {
        "tool_task"
    } else if lower.contains("handbook")
        || lower.contains("onboarding")
        || lower.contains("look up")
        || lower.contains("runbook")
    {
        "rag_question"
    } else {
        "chat"
    }
    .to_owned()
}

fn select_eval_tool(prompt: &str) -> String {
    let lower = prompt.to_ascii_lowercase();
    if lower.contains("audit") {
        "tool.audit.record"
    } else if lower.contains("handoff") || lower.contains("human") || lower.contains("angry") {
        "handoff.request"
    } else if lower.contains("ticket") {
        "ticket.create"
    } else if lower.contains("customer") {
        "customer.lookup"
    } else if lower.contains("refund") || lower.contains("faq") {
        "faq.search"
    } else if lower.contains("github") || lower.contains("repository") || lower.contains("repo") {
        "github.repo.search"
    } else if lower.contains("feishu")
        || lower.contains("reminder")
        || lower.contains("notify")
        || lower.contains("notice")
        || lower.contains("send")
    {
        "feishu.message.send"
    } else if lower.contains("image")
        || lower.contains("poster")
        || lower.contains("visual")
        || lower.contains("picture")
    {
        "media.image.generate"
    } else {
        "rag.search"
    }
    .to_owned()
}

pub fn eval_report_payload(
    total_cases: i32,
    passed_cases: i32,
    failed_cases: i32,
    average_score: f64,
    metric_breakdown: Value,
) -> Value {
    json!({
        "totalCases": total_cases,
        "passedCases": passed_cases,
        "failedCases": failed_cases,
        "averageScore": average_score,
        "metricBreakdown": metric_breakdown
    })
}

fn attach_trace_gate_summary(
    report_payload: &mut Value,
    command: &EvalRunCommand,
    case_count: usize,
) {
    if !eval_run_uses_trace_replay(command) {
        return;
    }
    if let Some(object) = report_payload.as_object_mut() {
        object.insert(
            "traceGate".to_owned(),
            json!({
                "runMode": command.run_mode.as_str(),
                "caseCount": case_count,
            }),
        );
    }
}

#[cfg(test)]
fn score_eval_case(case: &EvalCaseRecord) -> Result<EvalCaseScore, AppError> {
    let expected = expected_from_case(case)?;
    let actual = build_eval_actual(&case.target_kind, &expected, &case.prompt);
    Ok(score_eval_case_with_actual(case, &expected, &actual))
}

#[cfg(test)]
fn score_eval_case_with_actual(
    case: &EvalCaseRecord,
    expected: &EvalCaseExpected,
    actual: &EvalCaseActual,
) -> EvalCaseScore {
    let target_kind = target_kind_from_code(&case.target_kind);
    match metric_kind_from_code(&case.metric_kind) {
        EvalMetricKind::Latency => score_latency_case(
            case.case_code.clone(),
            target_kind,
            actual,
            expected_u32(&case.expected_payload, "maxLatencyMs", 5_000),
        ),
        EvalMetricKind::Cost => score_cost_case(
            case.case_code.clone(),
            target_kind,
            actual,
            expected_u32(&case.expected_payload, "maxCostCents", 0),
        ),
        EvalMetricKind::RetrievalRecall => {
            score_retrieval_recall_case(case.case_code.clone(), target_kind, expected, actual)
        }
        EvalMetricKind::CitationAccuracy => {
            let mut score = score_rag_case(expected, actual);
            score.case_id = case.case_code.clone();
            score.target_kind = target_kind;
            score
        }
        EvalMetricKind::IntentAccuracy => {
            let mut score = score_intent_case(expected, actual);
            score.case_id = case.case_code.clone();
            score.target_kind = target_kind;
            score
        }
        EvalMetricKind::ToolAccuracy => {
            let mut score = score_tool_case(expected, actual);
            score.case_id = case.case_code.clone();
            score.target_kind = target_kind;
            score
        }
        EvalMetricKind::GroundedResolution => score_customer_service_grounded_resolution_case(
            case.case_code.clone(),
            expected,
            actual,
        ),
        EvalMetricKind::HandoffAccuracy => {
            score_customer_service_handoff_accuracy_case(case.case_code.clone(), expected, actual)
        }
        metric => {
            let mut score = score_case(case.case_code.clone(), target_kind, expected, actual);
            score.metric = metric;
            score
        }
    }
}

#[cfg(test)]
fn expected_from_case(case: &EvalCaseRecord) -> Result<EvalCaseExpected, AppError> {
    serde_json::from_value::<EvalCaseExpected>(case.expected_payload.clone())
        .map_err(|err| AppError::bad_request(format!("评测期望格式错误: {err}")))
}

impl From<EvalDatasetRecord> for EvalDatasetResp {
    fn from(record: EvalDatasetRecord) -> Self {
        Self {
            id: record.id,
            code: record.code,
            name: record.name,
            description: record.description,
            target_scope: record.target_scope,
            status: record.status,
            metadata: record.metadata,
            case_count: record.case_count,
            create_time: format_datetime(record.create_time),
        }
    }
}

impl From<EvalCaseRecord> for EvalCaseResp {
    fn from(record: EvalCaseRecord) -> Self {
        Self {
            id: record.id,
            dataset_id: record.dataset_id,
            case_code: record.case_code,
            target_kind: record.target_kind,
            metric_kind: record.metric_kind,
            prompt: record.prompt,
            expected_payload: record.expected_payload,
            tags: record.tags,
            status: record.status,
            sort: record.sort,
            create_time: format_datetime(record.create_time),
        }
    }
}

impl From<EvalRunRecord> for EvalRunResp {
    fn from(record: EvalRunRecord) -> Self {
        Self {
            run_id: record.id,
            dataset_id: record.dataset_id,
            dataset_code: record.dataset_code,
            status: record.status,
            total_cases: record.total_cases,
            passed_cases: record.passed_cases,
            failed_cases: record.failed_cases,
            average_score: record.average_score,
            metric_breakdown: record.metric_breakdown,
            report_payload: record.report_payload,
            create_time: format_datetime(record.create_time),
            finished_at: record.finished_at.map(format_datetime),
        }
    }
}

impl From<EvalResultRecord> for EvalResultResp {
    fn from(record: EvalResultRecord) -> Self {
        Self {
            id: record.id,
            run_id: record.run_id,
            case_id: record.case_id,
            case_code: record.case_code,
            target_kind: record.target_kind,
            metric_kind: record.metric_kind,
            score: record.score,
            passed: record.passed,
            expected_payload: record.expected_payload,
            actual_payload: record.actual_payload,
            reason: record.reason,
            cost_cents: record.cost_cents,
            latency_ms: record.latency_ms,
            create_time: format_datetime(record.create_time),
        }
    }
}

#[cfg(test)]
fn target_kind_from_code(code: &str) -> EvalTargetKind {
    match code {
        "rag" => EvalTargetKind::Rag,
        "intent" => EvalTargetKind::Intent,
        "tool" => EvalTargetKind::Tool,
        "react" => EvalTargetKind::ReAct,
        "safety" => EvalTargetKind::Safety,
        "customer_service" => EvalTargetKind::CustomerService,
        _ => EvalTargetKind::Rag,
    }
}

#[cfg(test)]
fn metric_kind_from_code(code: &str) -> EvalMetricKind {
    match code {
        "retrieval_recall" => EvalMetricKind::RetrievalRecall,
        "citation_accuracy" => EvalMetricKind::CitationAccuracy,
        "faithfulness" => EvalMetricKind::Faithfulness,
        "intent_accuracy" => EvalMetricKind::IntentAccuracy,
        "tool_accuracy" => EvalMetricKind::ToolAccuracy,
        "cost" => EvalMetricKind::Cost,
        "latency" => EvalMetricKind::Latency,
        "grounded_resolution" => EvalMetricKind::GroundedResolution,
        "handoff_accuracy" => EvalMetricKind::HandoffAccuracy,
        _ => EvalMetricKind::Faithfulness,
    }
}

fn target_kind_code(target: EvalTargetKind) -> String {
    match target {
        EvalTargetKind::Rag => "rag",
        EvalTargetKind::Intent => "intent",
        EvalTargetKind::Tool => "tool",
        EvalTargetKind::ReAct => "react",
        EvalTargetKind::Safety => "safety",
        EvalTargetKind::CustomerService => "customer_service",
    }
    .to_owned()
}

#[cfg(test)]
fn expected_u32(payload: &Value, key: &str, fallback: u32) -> u32 {
    payload
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(fallback)
}

fn metric_code(metric: EvalMetricKind) -> String {
    match metric {
        EvalMetricKind::RetrievalRecall => "retrieval_recall",
        EvalMetricKind::CitationAccuracy => "citation_accuracy",
        EvalMetricKind::Faithfulness => "faithfulness",
        EvalMetricKind::IntentAccuracy => "intent_accuracy",
        EvalMetricKind::ToolAccuracy => "tool_accuracy",
        EvalMetricKind::Cost => "cost",
        EvalMetricKind::Latency => "latency",
        EvalMetricKind::GroundedResolution => "grounded_resolution",
        EvalMetricKind::HandoffAccuracy => "handoff_accuracy",
    }
    .to_owned()
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_eval_size() -> u64 {
    DEFAULT_EVAL_PAGE_SIZE
}

fn default_case_size() -> u64 {
    DEFAULT_CASE_PAGE_SIZE
}

fn default_enabled_status() -> Option<i16> {
    Some(ENABLED_STATUS)
}

fn default_capture_dry_run() -> bool {
    true
}

fn eval_case_code_from_agent_run(run_id: i64) -> String {
    format!("agent-trace-{run_id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_eval::{actual_from_trace_bundle, EvalCaseExpected};
    use novex_trace::{TraceBundle, TraceEvent};
    use serde_json::json;
    use sqlx::postgres::PgPoolOptions;

    #[tokio::test]
    async fn eval_service_can_be_bound_to_request_tenant() {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
            .unwrap();
        let service = EvalService::for_tenant(db, 42);

        assert_eq!(service.tenant_id, 42);
    }

    #[test]
    fn eval_runtime_rejects_missing_dataset_selector() {
        let err = normalize_eval_run_command(EvalRunCommand {
            dataset_id: None,
            dataset_code: "   ".to_owned(),
            run_mode: String::new(),
        })
        .unwrap_err();

        assert!(err.to_string().contains("评测集不能为空"));
    }

    #[test]
    fn eval_runtime_normalizes_live_rag_run_mode() {
        let command = normalize_eval_run_command(EvalRunCommand {
            dataset_id: Some(10),
            dataset_code: "  knowledge_base_regression  ".to_owned(),
            run_mode: " LIVE_RAG ".to_owned(),
        })
        .unwrap();

        assert_eq!(command.dataset_code, "knowledge_base_regression");
        assert_eq!(command.run_mode, "live_rag");
        assert!(eval_run_uses_live_rag(&command));
    }

    #[test]
    fn eval_runtime_normalizes_trace_replay_run_mode() {
        let command = normalize_eval_run_command(EvalRunCommand {
            dataset_id: Some(10),
            dataset_code: "  agent_workspace_regression  ".to_owned(),
            run_mode: " TRACE_REPLAY ".to_owned(),
        })
        .unwrap();

        assert_eq!(command.dataset_code, "agent_workspace_regression");
        assert_eq!(command.run_mode, EVAL_RUN_MODE_TRACE_REPLAY);
        assert!(eval_run_uses_trace_replay(&command));
    }

    #[test]
    fn eval_case_capture_command_requires_dataset_when_persisting() {
        let err = normalize_eval_case_capture_command(EvalCaseCaptureCommand {
            dataset_id: None,
            dataset_code: " ".to_owned(),
            dry_run: false,
        })
        .unwrap_err();

        assert!(err.to_string().contains("评测集不能为空"));
    }

    #[test]
    fn live_rag_eval_reads_knowledge_dataset_id_from_metadata_tags_or_expected() {
        assert_eq!(
            live_rag_knowledge_dataset_id(
                &json!({"knowledgeDatasetId": 7001}),
                &json!({}),
                &json!({})
            ),
            Some(7001)
        );
        assert_eq!(
            live_rag_knowledge_dataset_id(
                &json!({}),
                &json!({"knowledgeDatasetId": 7002}),
                &json!({})
            ),
            Some(7002)
        );
        assert_eq!(
            live_rag_knowledge_dataset_id(
                &json!({}),
                &json!({}),
                &json!({"knowledgeDatasetId": 7003})
            ),
            Some(7003)
        );
    }

    #[test]
    fn eval_runtime_builds_rag_actual_from_prompt_adapter() {
        let expected = EvalCaseExpected {
            answer_contains: vec!["Monday".to_owned()],
            citations: vec!["training-handbook:0".to_owned()],
            intent: None,
            tool_code: None,
        };

        let actual = build_eval_actual("rag", &expected, "When does training start?");

        assert_eq!(actual.answer.as_deref(), Some("Training starts on Monday."));
        assert_eq!(actual.citations, vec!["training-handbook:0"]);
    }

    #[test]
    fn eval_runtime_actual_is_derived_from_prompt_not_expected_payload() {
        let expected = EvalCaseExpected {
            answer_contains: vec!["Friday".to_owned()],
            citations: vec!["wrong-source:99".to_owned()],
            intent: Some("wrong_intent".to_owned()),
            tool_code: Some("wrong.tool".to_owned()),
        };

        let actual = build_eval_actual("rag", &expected, "When does training start?");

        assert_eq!(actual.answer.as_deref(), Some("Training starts on Monday."));
        assert_eq!(actual.citations, vec!["training-handbook:0"]);
    }

    #[test]
    fn eval_runtime_report_response_exposes_pass_fail_and_breakdown() {
        let report = eval_report_payload(
            20,
            18,
            2,
            0.9,
            json!({ "citation_accuracy": 0.875, "intent_accuracy": 1.0 }),
        );

        assert_eq!(report["totalCases"], 20);
        assert_eq!(report["passedCases"], 18);
        assert_eq!(report["metricBreakdown"]["intent_accuracy"], 1.0);
    }

    #[test]
    fn eval_runtime_report_response_exposes_trace_gate_summary() {
        let command = EvalRunCommand {
            dataset_id: Some(10),
            dataset_code: "agent_workspace_regression".to_owned(),
            run_mode: EVAL_RUN_MODE_TRACE_REPLAY.to_owned(),
        };
        let mut report = eval_report_payload(2, 2, 0, 1.0, json!({ "tool_accuracy": 1.0 }));

        attach_trace_gate_summary(&mut report, &command, 2);

        assert_eq!(report["traceGate"]["runMode"], "trace_replay");
        assert_eq!(report["traceGate"]["caseCount"], 2);
    }

    #[test]
    fn customer_service_eval_seed_contains_resolution_and_handoff_cases() {
        let seed_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/202606160007_seed_customer_service_eval.sql"
        );
        let seed = std::fs::read_to_string(seed_path)
            .expect("missing customer service eval seed migration");

        for needle in [
            "'customer-service-agent-regression'",
            "'customer_service'",
            "'grounded_resolution'",
            "'handoff_accuracy'",
            "'cs-refund-with-citation'",
            "'cs-insufficient-evidence'",
            "'cs-human-handoff'",
            "'cs-ticket-approval'",
        ] {
            assert!(seed.contains(needle), "{needle} missing");
        }
    }

    #[test]
    fn eval_runtime_scores_latency_and_cost_cases_from_metric_kind() {
        let latency_case = EvalCaseRecord {
            id: 1,
            dataset_id: 10,
            case_code: "llm-latency".to_owned(),
            target_kind: "safety".to_owned(),
            metric_kind: "latency".to_owned(),
            prompt: "Draft a short answer.".to_owned(),
            expected_payload: json!({ "maxLatencyMs": 50 }),
            tags: json!(["llm", "latency"]),
            status: 1,
            sort: 1,
            create_time: chrono::NaiveDate::from_ymd_opt(2026, 6, 5)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
        };
        let cost_case = EvalCaseRecord {
            id: 2,
            dataset_id: 10,
            case_code: "llm-cost".to_owned(),
            target_kind: "safety".to_owned(),
            metric_kind: "cost".to_owned(),
            prompt: "Draft a short answer.".to_owned(),
            expected_payload: json!({ "maxCostCents": 0 }),
            tags: json!(["llm", "cost"]),
            status: 1,
            sort: 2,
            create_time: latency_case.create_time,
        };

        let latency_score = score_eval_case(&latency_case).unwrap();
        let cost_score = score_eval_case(&cost_case).unwrap();

        assert_eq!(metric_code(latency_score.metric), "latency");
        assert!(latency_score.passed);
        assert_eq!(metric_code(cost_score.metric), "cost");
        assert!(cost_score.passed);
    }

    #[test]
    fn eval_runtime_scores_agent_trace_tool_and_answer() {
        let bundle = TraceBundle::new("agent-1")
            .with_event(TraceEvent::user_message(
                1,
                "How should we handle customer data?",
            ))
            .with_event(TraceEvent::tool_call(2, "call-1", "rag.search"))
            .with_event(TraceEvent::final_answer(
                3,
                "Customer data must stay in approved systems.",
            ));
        let actual = actual_from_trace_bundle(&bundle);
        let expected = EvalCaseExpected {
            answer_contains: vec!["approved systems".to_owned()],
            citations: vec![],
            intent: None,
            tool_code: Some("rag.search".to_owned()),
        };
        let tool_case = fake_eval_case(
            "trace-tool",
            "react",
            "tool_accuracy",
            json!({"toolCode":"rag.search"}),
        );
        let answer_case = fake_eval_case(
            "trace-answer",
            "react",
            "faithfulness",
            json!({"answerContains":["approved systems"]}),
        );

        let tool_score = score_eval_case_with_actual(&tool_case, &expected, &actual);
        let answer_score = score_eval_case_with_actual(&answer_case, &expected, &actual);

        assert!(tool_score.passed);
        assert_eq!(tool_score.metric, EvalMetricKind::ToolAccuracy);
        assert!(answer_score.passed);
        assert_eq!(answer_score.metric, EvalMetricKind::Faithfulness);
    }

    #[test]
    fn customer_service_eval_scores_missing_evidence_gate() {
        let case = EvalCaseRecord {
            id: 1,
            dataset_id: 10,
            case_code: "cs-missing-citation".to_owned(),
            target_kind: "customer_service".to_owned(),
            metric_kind: "grounded_resolution".to_owned(),
            prompt: "What is the refund window?".to_owned(),
            expected_payload: json!({
                "answerContains": ["30 days"],
                "citations": ["wrong-source:99"]
            }),
            tags: json!(["customer-service", "citation"]),
            status: 1,
            sort: 1,
            create_time: chrono::NaiveDate::from_ymd_opt(2026, 6, 16)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
        };

        let score = score_eval_case(&case).unwrap();

        assert_eq!(score.metric, EvalMetricKind::GroundedResolution);
        assert!(!score.passed);
        assert!(score.reason.contains("missing evidence"));
    }

    #[test]
    fn eval_task_queue_migration_defines_required_tables() {
        let migration =
            include_str!("../../../migrations/202606100001_create_ai_eval_task_queue.sql");

        for needle in [
            "CREATE TABLE IF NOT EXISTS ai_eval_task",
            "CREATE TABLE IF NOT EXISTS ai_eval_outbox",
            "idx_ai_eval_task_status",
            "idx_ai_eval_outbox_status",
            "uq_ai_eval_outbox_task_event",
        ] {
            assert!(
                migration.contains(needle),
                "{needle} missing from eval queue migration"
            );
        }
    }

    #[test]
    fn eval_run_creation_builds_task_outbox_payload() {
        let case = EvalCaseRecord {
            id: 3401001,
            dataset_id: 3400001,
            case_code: "rag-training-start".to_owned(),
            target_kind: "rag".to_owned(),
            metric_kind: "citation_accuracy".to_owned(),
            prompt: "When does training start?".to_owned(),
            expected_payload: json!({"answerContains":["Monday"]}),
            tags: json!(["rag"]),
            status: 1,
            sort: 1,
            create_time: chrono::NaiveDate::from_ymd_opt(2026, 6, 10)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
        };

        let payload = eval_task_payload(99, 88, 1, &case, "live_rag", 3);

        assert_eq!(payload["taskId"], 99);
        assert_eq!(payload["runId"], 88);
        assert_eq!(payload["tenantId"], 1);
        assert_eq!(payload["caseId"], 3401001);
        assert_eq!(payload["runMode"], "live_rag");
        assert_eq!(payload["attempt"], 0);
        assert_eq!(payload["maxAttempts"], 3);
    }

    fn fake_eval_case(
        case_code: &str,
        target_kind: &str,
        metric_kind: &str,
        expected_payload: Value,
    ) -> EvalCaseRecord {
        EvalCaseRecord {
            id: 1,
            dataset_id: 10,
            case_code: case_code.to_owned(),
            target_kind: target_kind.to_owned(),
            metric_kind: metric_kind.to_owned(),
            prompt: "trace replay".to_owned(),
            expected_payload,
            tags: json!({ "agentRunId": 42 }),
            status: 1,
            sort: 1,
            create_time: chrono::NaiveDate::from_ymd_opt(2026, 6, 16)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
        }
    }
}
