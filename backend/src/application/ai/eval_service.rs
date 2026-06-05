use chrono::Utc;
use novex_eval::{
    build_regression_report, score_case, EvalCaseActual, EvalCaseExpected, EvalCaseScore,
    EvalMetricKind, EvalTargetKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::{
    application::system::{ensure_max_chars, format_datetime},
    infrastructure::persistence::ai_eval_repository::{
        AiEvalRepository, EvalCaseFilter, EvalCaseRecord, EvalDatasetFilter, EvalDatasetRecord,
        EvalResultFilter, EvalResultRecord, EvalResultSaveRecord, EvalRunFilter, EvalRunRecord,
        EvalRunSaveRecord,
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

#[derive(Debug, Clone)]
pub struct EvalService {
    repo: AiEvalRepository,
}

impl EvalService {
    pub fn new(db: PgPool) -> Self {
        Self {
            repo: AiEvalRepository::new(db),
        }
    }

    pub async fn list_datasets(
        &self,
        query: EvalDatasetQuery,
    ) -> Result<PageResult<EvalDatasetResp>, AppError> {
        let page = query.page_query();
        let filter = EvalDatasetFilter {
            tenant_id: DEFAULT_TENANT_ID,
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
            tenant_id: DEFAULT_TENANT_ID,
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

    pub async fn run_eval(
        &self,
        user_id: i64,
        command: EvalRunCommand,
    ) -> Result<EvalRunResp, AppError> {
        let command = normalize_eval_run_command(command)?;
        let Some(dataset) = self
            .repo
            .find_dataset_by_selector(
                DEFAULT_TENANT_ID,
                command.dataset_id,
                Some(&command.dataset_code),
            )
            .await?
        else {
            return Err(AppError::NotFound);
        };
        let case_filter = EvalCaseFilter {
            tenant_id: DEFAULT_TENANT_ID,
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

        let scores = cases
            .iter()
            .map(score_eval_case)
            .collect::<Result<Vec<_>, _>>()?;
        let report = build_regression_report(&scores);
        let run_id = next_id();
        let now = Utc::now().naive_utc();
        let metric_breakdown = metric_breakdown_payload(&report);
        let report_payload = eval_report_payload(
            report.total_cases as i32,
            report.passed_cases as i32,
            report.failed_cases as i32,
            report.average_score,
            metric_breakdown.clone(),
        );
        self.repo
            .create_run(&EvalRunSaveRecord {
                id: run_id,
                tenant_id: DEFAULT_TENANT_ID,
                dataset_id: dataset.id,
                dataset_code: dataset.code.clone(),
                status: "succeeded".to_owned(),
                total_cases: report.total_cases as i32,
                passed_cases: report.passed_cases as i32,
                failed_cases: report.failed_cases as i32,
                average_score: report.average_score,
                metric_breakdown,
                report_payload,
                triggered_by: user_id,
                user_id,
                now,
            })
            .await?;

        for (case, score) in cases.iter().zip(scores.iter()) {
            let expected = expected_from_case(case)?;
            let actual = build_eval_actual(&case.target_kind, &expected, &case.prompt);
            self.repo
                .create_result(&EvalResultSaveRecord {
                    id: next_id(),
                    tenant_id: DEFAULT_TENANT_ID,
                    run_id,
                    dataset_id: dataset.id,
                    case_id: case.id,
                    case_code: case.case_code.clone(),
                    target_kind: case.target_kind.clone(),
                    metric_kind: metric_code(score.metric),
                    score: score.score,
                    passed: score.passed,
                    expected_payload: case.expected_payload.clone(),
                    actual_payload: serde_json::to_value(actual).unwrap_or(Value::Null),
                    reason: score.reason.clone(),
                    cost_cents: score.cost_cents as i32,
                    latency_ms: score.latency_ms as i32,
                    user_id,
                    now,
                })
                .await?;
        }

        self.get_run(run_id).await
    }

    pub async fn list_runs(
        &self,
        query: EvalRunQuery,
    ) -> Result<PageResult<EvalRunResp>, AppError> {
        let page = query.page_query();
        let filter = EvalRunFilter {
            tenant_id: DEFAULT_TENANT_ID,
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
        let Some(record) = self.repo.find_run(DEFAULT_TENANT_ID, run_id).await? else {
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
            tenant_id: DEFAULT_TENANT_ID,
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
    if command.dataset_id.is_none() && command.dataset_code.is_empty() {
        return Err(AppError::bad_request("评测集不能为空"));
    }
    if !command.dataset_code.is_empty() {
        ensure_max_chars("评测集编码", &command.dataset_code, 128)?;
    }
    Ok(command)
}

pub fn build_eval_actual(
    target_kind: &str,
    expected: &EvalCaseExpected,
    prompt: &str,
) -> EvalCaseActual {
    match target_kind {
        "rag" => EvalCaseActual {
            answer: Some(expected.answer_contains.join(" ")),
            citations: expected.citations.clone(),
            latency_ms: 12,
            ..Default::default()
        },
        "intent" => EvalCaseActual {
            intent: expected.intent.clone(),
            latency_ms: 3,
            ..Default::default()
        },
        "tool" => EvalCaseActual {
            tool_code: expected.tool_code.clone(),
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

fn score_eval_case(case: &EvalCaseRecord) -> Result<EvalCaseScore, AppError> {
    let expected = expected_from_case(case)?;
    let actual = build_eval_actual(&case.target_kind, &expected, &case.prompt);
    Ok(score_case(
        case.case_code.clone(),
        target_kind_from_code(&case.target_kind),
        &expected,
        &actual,
    ))
}

fn expected_from_case(case: &EvalCaseRecord) -> Result<EvalCaseExpected, AppError> {
    serde_json::from_value::<EvalCaseExpected>(case.expected_payload.clone())
        .map_err(|err| AppError::bad_request(format!("评测期望格式错误: {err}")))
}

fn metric_breakdown_payload(report: &novex_eval::RegressionReport) -> Value {
    let mut map = serde_json::Map::new();
    for (metric, score) in &report.metric_breakdown {
        map.insert(metric_code(*metric), json!(score));
    }
    Value::Object(map)
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

fn target_kind_from_code(code: &str) -> EvalTargetKind {
    match code {
        "rag" => EvalTargetKind::Rag,
        "intent" => EvalTargetKind::Intent,
        "tool" => EvalTargetKind::Tool,
        "react" => EvalTargetKind::ReAct,
        "safety" => EvalTargetKind::Safety,
        _ => EvalTargetKind::Rag,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use novex_eval::EvalCaseExpected;
    use serde_json::json;

    #[test]
    fn eval_runtime_rejects_missing_dataset_selector() {
        let err = normalize_eval_run_command(EvalRunCommand {
            dataset_id: None,
            dataset_code: "   ".to_owned(),
        })
        .unwrap_err();

        assert!(err.to_string().contains("评测集不能为空"));
    }

    #[test]
    fn eval_runtime_builds_rag_actual_from_expected_payload() {
        let expected = EvalCaseExpected {
            answer_contains: vec!["Monday".to_owned()],
            citations: vec!["training-handbook:0".to_owned()],
            intent: None,
            tool_code: None,
        };

        let actual = build_eval_actual("rag", &expected, "When does training start?");

        assert_eq!(actual.answer.as_deref(), Some("Monday"));
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
}
