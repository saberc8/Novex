use std::{collections::BTreeMap, time::Instant};

use chrono::{Duration as ChronoDuration, Utc};
use futures_lite::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions};
use novex_eval::{
    actual_from_trace_bundle, score_case, score_cost_case,
    score_customer_service_grounded_resolution_case, score_customer_service_handoff_accuracy_case,
    score_intent_case, score_latency_case, score_rag_case, score_retrieval_recall_case,
    score_tool_case, EvalCaseActual, EvalCaseExpected, EvalCaseScore, EvalMetricKind,
    EvalTargetKind,
};
use novex_trace::TraceBundle;
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::{
    application::ai::{
        eval_service::{build_eval_actual, eval_report_payload},
        knowledge_service::{KnowledgeService, RagAskCommand},
    },
    infrastructure::{
        mq::rabbitmq::{EvalRabbitMqClient, EvalRabbitMqConfig, EvalTaskMessage},
        persistence::{
            ai_agent_repository::AiAgentRepository,
            ai_eval_repository::{
                AiEvalRepository, EvalResultFilter, EvalResultRecord, EvalResultSaveRecord,
                EvalTaskRecord, EvalTaskSummaryRecord,
            },
        },
    },
    shared::{error::AppError, id::next_id},
};

const TASK_STATUS_RETRY: &str = "retry";
const TASK_STATUS_FAILED: &str = "failed";
const TASK_STATUS_DEAD: &str = "dead";
const TASK_STATUS_CANCELLED: &str = "cancelled";
const TASK_STATUS_SUCCEEDED: &str = "succeeded";
const RUN_STATUS_RUNNING: &str = "running";
const RUN_STATUS_SUCCEEDED: &str = "succeeded";
const RUN_STATUS_FAILED: &str = "failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalTaskHandleOutcome {
    Ignored,
    Completed,
    Retried,
    Dead,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunAggregation {
    StillRunning,
    Finished {
        status: String,
        total_cases: i32,
        passed_cases: i32,
        failed_cases: i32,
        average_score: f64,
    },
}

#[async_trait::async_trait]
pub trait EvalTargetExecutor: Send + Sync {
    async fn execute(&self, task: &EvalTaskRecord) -> Result<EvalCaseActual, AppError>;
}

#[derive(Debug, Clone)]
pub struct RealEvalTargetExecutor {
    db: PgPool,
}

impl RealEvalTargetExecutor {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl EvalTargetExecutor for RealEvalTargetExecutor {
    async fn execute(&self, task: &EvalTaskRecord) -> Result<EvalCaseActual, AppError> {
        if task.run_mode == "trace_replay" {
            return execute_trace_replay_task(self.db.clone(), task).await;
        }
        if task.run_mode == "live_rag" && task.target_kind == "rag" {
            return execute_live_rag_task(self.db.clone(), task).await;
        }

        let expected = expected_from_task(task)?;
        Ok(build_eval_actual(
            &task.target_kind,
            &expected,
            task_prompt(task).as_deref().unwrap_or_default(),
        ))
    }
}

pub async fn run_eval_worker_runtime(
    db: PgPool,
    rabbitmq: EvalRabbitMqConfig,
    worker_id: String,
    task_timeout_seconds: u64,
) -> Result<(), AppError> {
    let mq = EvalRabbitMqClient::connect(rabbitmq).await?;
    let executor = RealEvalTargetExecutor::new(db.clone());
    let mut consumer = mq
        .channel()
        .basic_consume(
            &mq.config().execute_queue,
            &worker_id,
            BasicConsumeOptions::default(),
            lapin::types::FieldTable::default(),
        )
        .await
        .map_err(|error| AppError::Anyhow(anyhow::anyhow!("consume eval queue: {error}")))?;

    while let Some(delivery) = consumer.next().await {
        let delivery = delivery
            .map_err(|error| AppError::Anyhow(anyhow::anyhow!("receive eval message: {error}")))?;
        let message = serde_json::from_slice::<EvalTaskMessage>(&delivery.data)
            .map_err(|error| AppError::Anyhow(anyhow::anyhow!("decode eval message: {error}")))?;
        let repo = AiEvalRepository::new(db.clone());
        let outcome =
            handle_eval_task_message(&repo, &executor, &message, &worker_id, task_timeout_seconds)
                .await;
        match outcome {
            Ok(EvalTaskHandleOutcome::Retried) => {
                let mut retry = message.clone();
                retry.attempt += 1;
                mq.publish_eval_retry(&retry).await?;
            }
            Ok(EvalTaskHandleOutcome::Dead) => {
                mq.publish_eval_dead(&message).await?;
            }
            Ok(EvalTaskHandleOutcome::Completed | EvalTaskHandleOutcome::Ignored) => {}
            Err(error) => {
                tracing::error!(error = ?error, task_id = message.task_id, "execute eval task failed");
                return Err(error);
            }
        }
        delivery
            .ack(BasicAckOptions::default())
            .await
            .map_err(|error| AppError::Anyhow(anyhow::anyhow!("ack eval message: {error}")))?;
    }

    Ok(())
}

pub async fn handle_eval_task_message<E>(
    repo: &AiEvalRepository,
    executor: &E,
    message: &EvalTaskMessage,
    worker_id: &str,
    task_timeout_seconds: u64,
) -> Result<EvalTaskHandleOutcome, AppError>
where
    E: EvalTargetExecutor,
{
    let now = Utc::now().naive_utc();
    let lease_until = now + ChronoDuration::seconds(task_timeout_seconds.max(1) as i64);
    let Some(task) = repo
        .try_start_task(
            message.tenant_id,
            message.task_id,
            worker_id,
            lease_until,
            now,
        )
        .await?
    else {
        if let Some(task) = repo.find_task(message.tenant_id, message.task_id).await? {
            if is_terminal_task_status(&task.status) {
                aggregate_run_if_finished(
                    repo,
                    task.tenant_id,
                    task.run_id,
                    user_id_from_task(&task),
                )
                .await?;
            }
        }
        return Ok(EvalTaskHandleOutcome::Ignored);
    };
    let user_id = user_id_from_task(&task);
    repo.update_run_status(
        task.tenant_id,
        task.run_id,
        RUN_STATUS_RUNNING,
        user_id,
        now,
    )
    .await?;

    match executor.execute(&task).await {
        Ok(actual) => {
            let score = score_task_with_actual(&task, &actual)?;
            repo.create_result(&result_record_from_score(&task, &score, &actual, now))
                .await?;
            repo.complete_task(
                task.tenant_id,
                task.id,
                &task_success_trace_ref(&actual),
                user_id,
                Utc::now().naive_utc(),
            )
            .await?;
            aggregate_run_if_finished(repo, task.tenant_id, task.run_id, user_id).await?;
            Ok(EvalTaskHandleOutcome::Completed)
        }
        Err(error) => {
            let error_msg = error.to_string();
            let status = next_failure_status(task.attempt, task.max_attempts);
            if status == TASK_STATUS_DEAD {
                let actual = failure_actual(&error_msg);
                let score = failed_task_score(&task, &actual, &error_msg);
                repo.create_result(&result_record_from_score(&task, &score, &actual, now))
                    .await?;
            }
            repo.fail_task(
                task.tenant_id,
                task.id,
                status,
                &error_msg,
                user_id,
                Utc::now().naive_utc(),
            )
            .await?;
            aggregate_run_if_finished(repo, task.tenant_id, task.run_id, user_id).await?;
            Ok(if status == TASK_STATUS_DEAD {
                EvalTaskHandleOutcome::Dead
            } else {
                EvalTaskHandleOutcome::Retried
            })
        }
    }
}

pub async fn aggregate_run_if_finished(
    repo: &AiEvalRepository,
    tenant_id: i64,
    run_id: i64,
    user_id: i64,
) -> Result<RunAggregation, AppError> {
    let tasks = repo.list_run_tasks(tenant_id, run_id).await?;
    let Some(status) = aggregate_status_from_tasks(&tasks) else {
        return Ok(RunAggregation::StillRunning);
    };
    let results = repo
        .list_results(&EvalResultFilter {
            tenant_id,
            run_id,
            limit: tasks.len().max(1) as i64,
            offset: 0,
        })
        .await?;
    let total_cases = tasks.len() as i32;
    let passed_cases = results.iter().filter(|result| result.passed).count() as i32;
    let failed_cases = total_cases - passed_cases;
    let average_score = if total_cases == 0 {
        0.0
    } else {
        results.iter().map(|result| result.score).sum::<f64>() / f64::from(total_cases)
    };
    let metric_breakdown = metric_breakdown_from_results(&results);
    let report_payload = eval_report_payload(
        total_cases,
        passed_cases,
        failed_cases,
        average_score,
        metric_breakdown.clone(),
    );
    repo.update_run_summary(
        tenant_id,
        run_id,
        &status,
        total_cases,
        passed_cases,
        failed_cases,
        average_score,
        &metric_breakdown,
        &report_payload,
        user_id,
        Utc::now().naive_utc(),
    )
    .await?;
    Ok(RunAggregation::Finished {
        status,
        total_cases,
        passed_cases,
        failed_cases,
        average_score,
    })
}

fn aggregate_status_from_tasks(tasks: &[EvalTaskSummaryRecord]) -> Option<String> {
    if tasks.is_empty()
        || tasks
            .iter()
            .any(|task| !is_terminal_task_status(&task.status))
    {
        return None;
    }
    if tasks
        .iter()
        .any(|task| matches!(task.status.as_str(), TASK_STATUS_FAILED | TASK_STATUS_DEAD))
    {
        Some(RUN_STATUS_FAILED.to_owned())
    } else {
        Some(RUN_STATUS_SUCCEEDED.to_owned())
    }
}

fn is_terminal_task_status(status: &str) -> bool {
    matches!(
        status,
        TASK_STATUS_SUCCEEDED | TASK_STATUS_FAILED | TASK_STATUS_DEAD | TASK_STATUS_CANCELLED
    )
}

fn next_failure_status(attempt: i32, max_attempts: i32) -> &'static str {
    if attempt >= max_attempts {
        TASK_STATUS_DEAD
    } else {
        TASK_STATUS_RETRY
    }
}

async fn execute_live_rag_task(
    db: PgPool,
    task: &EvalTaskRecord,
) -> Result<EvalCaseActual, AppError> {
    let knowledge_dataset_id = live_rag_knowledge_dataset_id(task)
        .ok_or_else(|| AppError::bad_request("live_rag 评测缺少 knowledgeDatasetId"))?;
    let user_id = task.create_user.unwrap_or_default();
    let started = Instant::now();
    let response = KnowledgeService::new(db)
        .ask_dataset_for_tenant(
            task.tenant_id,
            user_id,
            knowledge_dataset_id,
            RagAskCommand {
                question: task_prompt(task).unwrap_or_default(),
                limit: 5,
                ..RagAskCommand::default()
            },
        )
        .await?;

    Ok(EvalCaseActual {
        answer: Some(response.answer),
        citations: response
            .citations
            .into_iter()
            .map(|citation| citation.chunk_id)
            .collect(),
        latency_ms: started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32,
        ..Default::default()
    })
}

async fn execute_trace_replay_task(
    db: PgPool,
    task: &EvalTaskRecord,
) -> Result<EvalCaseActual, AppError> {
    let repo = AiAgentRepository::new(db);
    let rollout = if let Some(agent_run_id) = trace_replay_agent_run_id(task) {
        repo.find_rollout_by_run_id(task.tenant_id, agent_run_id)
            .await?
    } else if let Some(trace_id) = trace_replay_trace_id(task) {
        repo.find_rollout_by_trace_id(task.tenant_id, &trace_id)
            .await?
    } else {
        return Err(AppError::bad_request(
            "trace_replay 评测缺少 agentRunId 或 traceId",
        ));
    };
    let Some(rollout) = rollout else {
        return Err(AppError::NotFound);
    };
    let bundle = serde_json::from_value::<TraceBundle>(rollout.event_bundle)
        .map_err(|err| AppError::bad_request(format!("Agent trace bundle 格式错误: {err}")))?;
    Ok(actual_from_trace_bundle(&bundle))
}

fn trace_replay_agent_run_id(task: &EvalTaskRecord) -> Option<i64> {
    json_positive_i64(&task.tags_snapshot, "agentRunId")
        .or_else(|| json_positive_i64(&task.tags_snapshot, "agent_run_id"))
        .or_else(|| json_positive_i64(&task.expected_snapshot, "agentRunId"))
        .or_else(|| json_positive_i64(&task.expected_snapshot, "agent_run_id"))
        .or_else(|| json_positive_i64(&task.runtime_config, "agentRunId"))
        .or_else(|| json_positive_i64(&task.runtime_config, "agent_run_id"))
        .or_else(|| json_positive_i64(&task.input_snapshot, "agentRunId"))
        .or_else(|| json_positive_i64(&task.input_snapshot, "agent_run_id"))
}

fn trace_replay_trace_id(task: &EvalTaskRecord) -> Option<String> {
    json_non_empty_string(&task.tags_snapshot, "traceId")
        .or_else(|| json_non_empty_string(&task.tags_snapshot, "trace_id"))
        .or_else(|| json_non_empty_string(&task.expected_snapshot, "traceId"))
        .or_else(|| json_non_empty_string(&task.expected_snapshot, "trace_id"))
        .or_else(|| json_non_empty_string(&task.runtime_config, "traceId"))
        .or_else(|| json_non_empty_string(&task.runtime_config, "trace_id"))
        .or_else(|| json_non_empty_string(&task.input_snapshot, "traceId"))
        .or_else(|| json_non_empty_string(&task.input_snapshot, "trace_id"))
}

fn live_rag_knowledge_dataset_id(task: &EvalTaskRecord) -> Option<i64> {
    json_positive_i64(&task.runtime_config, "knowledgeDatasetId")
        .or_else(|| json_positive_i64(&task.runtime_config, "knowledge_dataset_id"))
        .or_else(|| {
            task.runtime_config
                .get("datasetMetadata")
                .and_then(|value| json_positive_i64(value, "knowledgeDatasetId"))
        })
        .or_else(|| {
            task.runtime_config
                .get("datasetMetadata")
                .and_then(|value| json_positive_i64(value, "knowledge_dataset_id"))
        })
        .or_else(|| json_positive_i64(&task.tags_snapshot, "knowledgeDatasetId"))
        .or_else(|| json_positive_i64(&task.tags_snapshot, "knowledge_dataset_id"))
        .or_else(|| json_positive_i64(&task.expected_snapshot, "knowledgeDatasetId"))
        .or_else(|| json_positive_i64(&task.expected_snapshot, "knowledge_dataset_id"))
        .or_else(|| json_positive_i64(&task.input_snapshot, "knowledgeDatasetId"))
        .or_else(|| json_positive_i64(&task.input_snapshot, "knowledge_dataset_id"))
}

fn json_positive_i64(value: &Value, key: &str) -> Option<i64> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
}

fn json_non_empty_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn score_task_with_actual(
    task: &EvalTaskRecord,
    actual: &EvalCaseActual,
) -> Result<EvalCaseScore, AppError> {
    let expected = expected_from_task(task)?;
    let target_kind = target_kind_from_code(&task.target_kind);
    Ok(match metric_kind_from_code(&task.metric_kind) {
        EvalMetricKind::Latency => score_latency_case(
            task.case_code.clone(),
            target_kind,
            actual,
            expected_u32(&task.expected_snapshot, "maxLatencyMs", 5_000),
        ),
        EvalMetricKind::Cost => score_cost_case(
            task.case_code.clone(),
            target_kind,
            actual,
            expected_u32(&task.expected_snapshot, "maxCostCents", 0),
        ),
        EvalMetricKind::RetrievalRecall => {
            score_retrieval_recall_case(task.case_code.clone(), target_kind, &expected, actual)
        }
        EvalMetricKind::CitationAccuracy => {
            let mut score = score_rag_case(&expected, actual);
            score.case_id = task.case_code.clone();
            score.target_kind = target_kind;
            score
        }
        EvalMetricKind::IntentAccuracy => {
            let mut score = score_intent_case(&expected, actual);
            score.case_id = task.case_code.clone();
            score.target_kind = target_kind;
            score
        }
        EvalMetricKind::ToolAccuracy => {
            let mut score = score_tool_case(&expected, actual);
            score.case_id = task.case_code.clone();
            score.target_kind = target_kind;
            score
        }
        EvalMetricKind::GroundedResolution => score_customer_service_grounded_resolution_case(
            task.case_code.clone(),
            &expected,
            actual,
        ),
        EvalMetricKind::HandoffAccuracy => {
            score_customer_service_handoff_accuracy_case(task.case_code.clone(), &expected, actual)
        }
        metric => {
            let mut score = score_case(task.case_code.clone(), target_kind, &expected, actual);
            score.metric = metric;
            score
        }
    })
}

fn expected_from_task(task: &EvalTaskRecord) -> Result<EvalCaseExpected, AppError> {
    serde_json::from_value::<EvalCaseExpected>(task.expected_snapshot.clone())
        .map_err(|err| AppError::bad_request(format!("评测期望格式错误: {err}")))
}

fn result_record_from_score(
    task: &EvalTaskRecord,
    score: &EvalCaseScore,
    actual: &EvalCaseActual,
    now: chrono::NaiveDateTime,
) -> EvalResultSaveRecord {
    EvalResultSaveRecord {
        id: next_id(),
        tenant_id: task.tenant_id,
        run_id: task.run_id,
        dataset_id: task.dataset_id,
        case_id: task.case_id,
        case_code: task.case_code.clone(),
        target_kind: task.target_kind.clone(),
        metric_kind: metric_code(score.metric),
        score: score.score,
        passed: score.passed,
        expected_payload: task.expected_snapshot.clone(),
        actual_payload: serde_json::to_value(actual).unwrap_or(Value::Null),
        reason: score.reason.clone(),
        cost_cents: score.cost_cents as i32,
        latency_ms: score.latency_ms as i32,
        user_id: task.create_user.unwrap_or_default(),
        now,
    }
}

fn failed_task_score(task: &EvalTaskRecord, actual: &EvalCaseActual, error: &str) -> EvalCaseScore {
    EvalCaseScore {
        case_id: task.case_code.clone(),
        target_kind: target_kind_from_code(&task.target_kind),
        metric: metric_kind_from_code(&task.metric_kind),
        score: 0.0,
        passed: false,
        reason: error.to_owned(),
        cost_cents: actual.cost_cents,
        latency_ms: actual.latency_ms,
    }
}

fn failure_actual(error: &str) -> EvalCaseActual {
    EvalCaseActual {
        answer: Some(format!("eval task failed: {error}")),
        ..Default::default()
    }
}

fn task_success_trace_ref(actual: &EvalCaseActual) -> Value {
    json!({
        "citationCount": actual.citations.len(),
        "latencyMs": actual.latency_ms
    })
}

fn task_prompt(task: &EvalTaskRecord) -> Option<String> {
    task.input_snapshot
        .get("prompt")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn metric_breakdown_from_results(results: &[EvalResultRecord]) -> Value {
    let mut grouped: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    for result in results {
        let entry = grouped
            .entry(result.metric_kind.clone())
            .or_insert((0.0, 0usize));
        entry.0 += result.score;
        entry.1 += 1;
    }
    let mut map = serde_json::Map::new();
    for (metric, (sum, count)) in grouped {
        map.insert(
            metric,
            json!(if count == 0 { 0.0 } else { sum / count as f64 }),
        );
    }
    Value::Object(map)
}

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

fn user_id_from_task(task: &EvalTaskRecord) -> i64 {
    task.create_user.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_task_statuses_are_ignored_by_aggregation_gate() {
        assert!(is_terminal_task_status("succeeded"));
        assert!(is_terminal_task_status("dead"));
        assert!(!is_terminal_task_status("queued"));
        assert_eq!(next_failure_status(1, 3), "retry");
        assert_eq!(next_failure_status(3, 3), "dead");
    }

    #[test]
    fn live_rag_requires_knowledge_dataset_id() {
        let task = eval_task_record(json!({}), json!({}), json!({}));

        assert_eq!(live_rag_knowledge_dataset_id(&task), None);

        let task = eval_task_record(
            json!({"datasetMetadata": {"knowledgeDatasetId": 7001}}),
            json!({}),
            json!({}),
        );
        assert_eq!(live_rag_knowledge_dataset_id(&task), Some(7001));
    }

    #[test]
    fn trace_replay_reads_identity_from_task_snapshots() {
        let task = eval_task_record(
            json!({}),
            json!({"traceId": "trace-expected"}),
            json!({"agentRunId": 42}),
        );

        assert_eq!(trace_replay_agent_run_id(&task), Some(42));
        assert_eq!(
            trace_replay_trace_id(&task),
            Some("trace-expected".to_owned())
        );
    }

    #[test]
    fn successful_worker_output_converts_to_result_record() {
        let task = eval_task_record(
            json!({}),
            json!({"answerContains":["Monday"],"citations":["training-handbook:0"]}),
            json!(["rag"]),
        );
        let actual = EvalCaseActual {
            answer: Some("Training starts on Monday.".to_owned()),
            citations: vec!["training-handbook:0".to_owned()],
            latency_ms: 12,
            ..Default::default()
        };

        let score = score_task_with_actual(&task, &actual).unwrap();
        let record = result_record_from_score(&task, &score, &actual, task.create_time());

        assert!(record.passed);
        assert_eq!(record.metric_kind, "citation_accuracy");
        assert_eq!(
            record.actual_payload["answer"],
            "Training starts on Monday."
        );
    }

    #[test]
    fn customer_service_task_uses_grounded_resolution_gate() {
        let mut task = eval_task_record(
            json!({}),
            json!({
                "answerContains": ["30 days"],
                "citations": ["wrong-source:99"]
            }),
            json!(["customer-service", "citation"]),
        );
        task.case_code = "cs-missing-citation".to_owned();
        task.target_kind = "customer_service".to_owned();
        task.metric_kind = "grounded_resolution".to_owned();
        task.run_mode = "deterministic".to_owned();
        let actual = EvalCaseActual {
            answer: Some("Refunds are available within 30 days.".to_owned()),
            citations: vec!["cs-faq:refunds".to_owned()],
            latency_ms: 18,
            ..Default::default()
        };

        let score = score_task_with_actual(&task, &actual).unwrap();

        assert_eq!(score.metric, EvalMetricKind::GroundedResolution);
        assert!(!score.passed);
        assert!(score.reason.contains("missing evidence"));
    }

    #[test]
    fn aggregation_waits_until_all_tasks_are_terminal() {
        let running = vec![task_summary(1, "succeeded"), task_summary(2, "running")];
        let terminal = vec![task_summary(1, "succeeded"), task_summary(2, "dead")];

        assert_eq!(aggregate_status_from_tasks(&running), None);
        assert_eq!(
            aggregate_status_from_tasks(&terminal),
            Some("failed".to_owned())
        );
    }

    #[test]
    fn worker_does_not_dead_letter_infrastructure_errors() {
        let source = include_str!("eval_worker_runtime.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("return Err(error);"));
        assert!(
            !source.contains("mq.publish_eval_dead(&message).await?;\n            }\n        }\n        delivery"),
            "infrastructure errors must not be dead-lettered and acked as business failures"
        );
    }

    fn task_summary(id: i64, status: &str) -> EvalTaskSummaryRecord {
        EvalTaskSummaryRecord {
            id,
            status: status.to_owned(),
            attempt: 1,
            max_attempts: 3,
        }
    }

    fn eval_task_record(
        runtime_config: Value,
        expected_snapshot: Value,
        tags_snapshot: Value,
    ) -> EvalTaskRecord {
        EvalTaskRecord {
            id: 30,
            tenant_id: 1,
            run_id: 20,
            dataset_id: 10,
            case_id: 40,
            case_code: "rag-training-start".to_owned(),
            target_kind: "rag".to_owned(),
            metric_kind: "citation_accuracy".to_owned(),
            run_mode: "live_rag".to_owned(),
            status: "running".to_owned(),
            attempt: 1,
            max_attempts: 3,
            input_snapshot: json!({"prompt":"When does training start?"}),
            expected_snapshot,
            tags_snapshot,
            runtime_config,
            trace_ref: json!({}),
            last_error: String::new(),
            create_user: Some(1),
        }
    }

    trait TestTaskTime {
        fn create_time(&self) -> chrono::NaiveDateTime;
    }

    impl TestTaskTime for EvalTaskRecord {
        fn create_time(&self) -> chrono::NaiveDateTime {
            chrono::NaiveDate::from_ymd_opt(2026, 6, 10)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap()
        }
    }
}
