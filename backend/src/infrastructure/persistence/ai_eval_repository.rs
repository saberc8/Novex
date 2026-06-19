use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};

use crate::shared::error::AppError;

#[derive(Debug, Clone)]
pub struct AiEvalRepository {
    db: PgPool,
}

#[derive(Debug, Clone)]
pub struct EvalDatasetFilter<'a> {
    pub tenant_id: i64,
    pub status: Option<i16>,
    pub code: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct EvalCaseFilter<'a> {
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub status: Option<i16>,
    pub target_kind: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct EvalRunFilter<'a> {
    pub tenant_id: i64,
    pub dataset_code: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct EvalResultFilter {
    pub tenant_id: i64,
    pub run_id: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct EvalTaskSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub dataset_id: i64,
    pub case_id: i64,
    pub case_code: String,
    pub target_kind: String,
    pub metric_kind: String,
    pub run_mode: String,
    pub status: String,
    pub attempt: i32,
    pub max_attempts: i32,
    pub input_snapshot: Value,
    pub expected_snapshot: Value,
    pub tags_snapshot: Value,
    pub runtime_config: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct EvalOutboxSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub task_id: i64,
    pub event_type: String,
    pub payload: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct EvalRunSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub dataset_code: String,
    pub status: String,
    pub total_cases: i32,
    pub passed_cases: i32,
    pub failed_cases: i32,
    pub average_score: f64,
    pub metric_breakdown: Value,
    pub report_payload: Value,
    pub triggered_by: i64,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct EvalResultSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub dataset_id: i64,
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
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct EvalCaseSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub case_code: String,
    pub target_kind: String,
    pub metric_kind: String,
    pub prompt: String,
    pub expected_payload: Value,
    pub tags: Value,
    pub status: i16,
    pub sort: i32,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct EvalDatasetRecord {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub target_scope: String,
    pub status: i16,
    pub metadata: Value,
    pub case_count: i64,
    pub create_time: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct EvalCaseRecord {
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
    pub create_time: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct EvalRunRecord {
    pub id: i64,
    pub dataset_id: i64,
    pub dataset_code: String,
    pub status: String,
    pub total_cases: i32,
    pub passed_cases: i32,
    pub failed_cases: i32,
    pub average_score: f64,
    pub metric_breakdown: Value,
    pub report_payload: Value,
    pub create_time: NaiveDateTime,
    pub finished_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, FromRow)]
pub struct EvalResultRecord {
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
    pub create_time: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct EvalTaskRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub dataset_id: i64,
    pub case_id: i64,
    pub case_code: String,
    pub target_kind: String,
    pub metric_kind: String,
    pub run_mode: String,
    pub status: String,
    pub attempt: i32,
    pub max_attempts: i32,
    pub input_snapshot: Value,
    pub expected_snapshot: Value,
    pub tags_snapshot: Value,
    pub runtime_config: Value,
    pub trace_ref: Value,
    pub last_error: String,
    pub create_user: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
pub struct EvalOutboxRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub task_id: i64,
    pub event_type: String,
    pub payload: Value,
    pub status: i16,
    pub attempt_count: i32,
}

#[derive(Debug, Clone, FromRow)]
pub struct EvalTaskSummaryRecord {
    pub id: i64,
    pub status: String,
    pub attempt: i32,
    pub max_attempts: i32,
}

impl AiEvalRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn count_datasets(&self, filter: &EvalDatasetFilter<'_>) -> Result<i64, AppError> {
        let mut query = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ai_eval_dataset AS d");
        query
            .push(" WHERE d.tenant_id = ")
            .push_bind(filter.tenant_id);
        push_dataset_filters(&mut query, filter);
        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_datasets(
        &self,
        filter: &EvalDatasetFilter<'_>,
    ) -> Result<Vec<EvalDatasetRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(dataset_select_sql());
        query
            .push(" WHERE d.tenant_id = ")
            .push_bind(filter.tenant_id);
        push_dataset_filters(&mut query, filter);
        query
            .push(" GROUP BY d.id ORDER BY d.create_time DESC, d.id DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);
        Ok(query
            .build_query_as::<EvalDatasetRecord>()
            .fetch_all(&self.db)
            .await?)
    }

    pub async fn find_dataset_by_selector(
        &self,
        tenant_id: i64,
        dataset_id: Option<i64>,
        dataset_code: Option<&str>,
    ) -> Result<Option<EvalDatasetRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(dataset_select_sql());
        query.push(" WHERE d.tenant_id = ").push_bind(tenant_id);
        if let Some(id) = dataset_id {
            query.push(" AND d.id = ").push_bind(id);
        } else if let Some(code) = non_empty(dataset_code) {
            query.push(" AND d.code = ").push_bind(code.to_owned());
        } else {
            query.push(" AND FALSE");
        }
        query.push(" GROUP BY d.id LIMIT 1");
        Ok(query
            .build_query_as::<EvalDatasetRecord>()
            .fetch_optional(&self.db)
            .await?)
    }

    pub async fn count_cases(&self, filter: &EvalCaseFilter<'_>) -> Result<i64, AppError> {
        let mut query = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ai_eval_case AS c");
        query
            .push(" WHERE c.tenant_id = ")
            .push_bind(filter.tenant_id)
            .push(" AND c.dataset_id = ")
            .push_bind(filter.dataset_id);
        push_case_filters(&mut query, filter);
        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_cases(
        &self,
        filter: &EvalCaseFilter<'_>,
    ) -> Result<Vec<EvalCaseRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(case_select_sql());
        query
            .push(" WHERE c.tenant_id = ")
            .push_bind(filter.tenant_id)
            .push(" AND c.dataset_id = ")
            .push_bind(filter.dataset_id);
        push_case_filters(&mut query, filter);
        query
            .push(" ORDER BY c.sort ASC, c.id ASC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);
        Ok(query
            .build_query_as::<EvalCaseRecord>()
            .fetch_all(&self.db)
            .await?)
    }

    pub async fn create_run(&self, record: &EvalRunSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_eval_run (
    id, tenant_id, dataset_id, dataset_code, status, total_cases, passed_cases,
    failed_cases, average_score, metric_breakdown, report_payload, triggered_by,
    started_at, finished_at, create_user, create_time, update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $14, $14, $13, $14, $13, $14);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.dataset_id)
        .bind(&record.dataset_code)
        .bind(&record.status)
        .bind(record.total_cases)
        .bind(record.passed_cases)
        .bind(record.failed_cases)
        .bind(record.average_score)
        .bind(&record.metric_breakdown)
        .bind(&record.report_payload)
        .bind(record.triggered_by)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn create_run_with_tasks_and_outbox(
        &self,
        run: &EvalRunSaveRecord,
        tasks: &[EvalTaskSaveRecord],
        outbox_records: &[EvalOutboxSaveRecord],
    ) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;

        sqlx::query(
            r#"
INSERT INTO ai_eval_run (
    id, tenant_id, dataset_id, dataset_code, status, total_cases, passed_cases,
    failed_cases, average_score, metric_breakdown, report_payload, triggered_by,
    started_at, finished_at, create_user, create_time, update_user, update_time
)
VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
    CASE WHEN $5 = 'queued' THEN NULL ELSE $14 END,
    CASE WHEN $5 IN ('succeeded', 'failed', 'cancelled') THEN $14 ELSE NULL END,
    $13, $14, $13, $14
);
"#,
        )
        .bind(run.id)
        .bind(run.tenant_id)
        .bind(run.dataset_id)
        .bind(&run.dataset_code)
        .bind(&run.status)
        .bind(run.total_cases)
        .bind(run.passed_cases)
        .bind(run.failed_cases)
        .bind(run.average_score)
        .bind(&run.metric_breakdown)
        .bind(&run.report_payload)
        .bind(run.triggered_by)
        .bind(run.user_id)
        .bind(run.now)
        .execute(&mut *tx)
        .await?;

        for task in tasks {
            sqlx::query(
                r#"
INSERT INTO ai_eval_task (
    id, tenant_id, run_id, dataset_id, case_id, case_code, target_kind, metric_kind,
    run_mode, status, attempt, max_attempts, scheduled_at, input_snapshot,
    expected_snapshot, tags_snapshot, runtime_config, create_user, create_time,
    update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $16, $13, $14, $15, $17, $18, $16, $18, $16);
"#,
            )
            .bind(task.id)
            .bind(task.tenant_id)
            .bind(task.run_id)
            .bind(task.dataset_id)
            .bind(task.case_id)
            .bind(&task.case_code)
            .bind(&task.target_kind)
            .bind(&task.metric_kind)
            .bind(&task.run_mode)
            .bind(&task.status)
            .bind(task.attempt)
            .bind(task.max_attempts)
            .bind(&task.input_snapshot)
            .bind(&task.expected_snapshot)
            .bind(&task.tags_snapshot)
            .bind(task.now)
            .bind(&task.runtime_config)
            .bind(task.user_id)
            .execute(&mut *tx)
            .await?;
        }

        for record in outbox_records {
            sqlx::query(
                r#"
INSERT INTO ai_eval_outbox (
    id, tenant_id, run_id, task_id, event_type, payload, status, attempt_count,
    create_user, create_time, update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, $6, 1, 0, $7, $8, $7, $8)
ON CONFLICT (tenant_id, task_id, event_type) DO NOTHING;
"#,
            )
            .bind(record.id)
            .bind(record.tenant_id)
            .bind(record.run_id)
            .bind(record.task_id)
            .bind(&record.event_type)
            .bind(&record.payload)
            .bind(record.user_id)
            .bind(record.now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn create_result(&self, record: &EvalResultSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_eval_result (
    id, tenant_id, run_id, dataset_id, case_id, case_code, target_kind, metric_kind,
    score, passed, expected_payload, actual_payload, reason, cost_cents, latency_ms,
    create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.run_id)
        .bind(record.dataset_id)
        .bind(record.case_id)
        .bind(&record.case_code)
        .bind(&record.target_kind)
        .bind(&record.metric_kind)
        .bind(record.score)
        .bind(record.passed)
        .bind(&record.expected_payload)
        .bind(&record.actual_payload)
        .bind(&record.reason)
        .bind(record.cost_cents)
        .bind(record.latency_ms)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn upsert_case(&self, record: &EvalCaseSaveRecord) -> Result<i64, AppError> {
        Ok(sqlx::query_scalar::<_, i64>(
            r#"
INSERT INTO ai_eval_case (
    id, tenant_id, dataset_id, case_code, target_kind, metric_kind, prompt,
    expected_payload, tags, status, sort, create_user, create_time, update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $12, $13)
ON CONFLICT (dataset_id, case_code)
DO UPDATE SET
    target_kind = EXCLUDED.target_kind,
    metric_kind = EXCLUDED.metric_kind,
    prompt = EXCLUDED.prompt,
    expected_payload = EXCLUDED.expected_payload,
    tags = EXCLUDED.tags,
    status = EXCLUDED.status,
    sort = EXCLUDED.sort,
    update_user = EXCLUDED.update_user,
    update_time = EXCLUDED.update_time
RETURNING id;
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.dataset_id)
        .bind(&record.case_code)
        .bind(&record.target_kind)
        .bind(&record.metric_kind)
        .bind(&record.prompt)
        .bind(&record.expected_payload)
        .bind(&record.tags)
        .bind(record.status)
        .bind(record.sort)
        .bind(record.user_id)
        .bind(record.now)
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn create_task(&self, record: &EvalTaskSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_eval_task (
    id, tenant_id, run_id, dataset_id, case_id, case_code, target_kind, metric_kind,
    run_mode, status, attempt, max_attempts, scheduled_at, input_snapshot,
    expected_snapshot, tags_snapshot, runtime_config, create_user, create_time,
    update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $16, $13, $14, $15, $17, $18, $16, $18, $16);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.run_id)
        .bind(record.dataset_id)
        .bind(record.case_id)
        .bind(&record.case_code)
        .bind(&record.target_kind)
        .bind(&record.metric_kind)
        .bind(&record.run_mode)
        .bind(&record.status)
        .bind(record.attempt)
        .bind(record.max_attempts)
        .bind(&record.input_snapshot)
        .bind(&record.expected_snapshot)
        .bind(&record.tags_snapshot)
        .bind(record.now)
        .bind(&record.runtime_config)
        .bind(record.user_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn create_outbox(&self, record: &EvalOutboxSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_eval_outbox (
    id, tenant_id, run_id, task_id, event_type, payload, status, attempt_count,
    create_user, create_time, update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, $6, 1, 0, $7, $8, $7, $8)
ON CONFLICT (tenant_id, task_id, event_type) DO NOTHING;
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.run_id)
        .bind(record.task_id)
        .bind(&record.event_type)
        .bind(&record.payload)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn list_pending_eval_outbox(
        &self,
        limit: i64,
    ) -> Result<Vec<EvalOutboxRecord>, AppError> {
        Ok(sqlx::query_as::<_, EvalOutboxRecord>(
            r#"
SELECT id, tenant_id, run_id, task_id, event_type, payload, status, attempt_count
FROM ai_eval_outbox
WHERE status IN (1, 3)
ORDER BY create_time ASC, id ASC
LIMIT $1;
"#,
        )
        .bind(limit)
        .fetch_all(&self.db)
        .await?)
    }

    pub async fn mark_eval_outbox_published(
        &self,
        outbox_id: i64,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_eval_outbox
SET status = 2,
    published_time = $3,
    update_user = $2,
    update_time = $3
WHERE id = $1;
"#,
        )
        .bind(outbox_id)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn mark_eval_outbox_publish_failed(
        &self,
        outbox_id: i64,
        error: &str,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_eval_outbox
SET status = 3,
    attempt_count = attempt_count + 1,
    last_error = $2,
    update_user = $3,
    update_time = $4
WHERE id = $1;
"#,
        )
        .bind(outbox_id)
        .bind(error)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn try_start_task(
        &self,
        tenant_id: i64,
        task_id: i64,
        worker_id: &str,
        lease_until: NaiveDateTime,
        now: NaiveDateTime,
    ) -> Result<Option<EvalTaskRecord>, AppError> {
        Ok(sqlx::query_as::<_, EvalTaskRecord>(&format!(
            r#"
UPDATE ai_eval_task
SET status = 'running',
    attempt = attempt + 1,
    lease_owner = $3,
    lease_until = $4,
    started_at = COALESCE(started_at, $5),
    update_user = 0,
    update_time = $5
WHERE tenant_id = $1
  AND id = $2
  AND (
      status IN ('queued', 'retry')
      OR (status = 'running' AND lease_owner = $3)
      OR (status = 'running' AND lease_until IS NOT NULL AND lease_until < $5)
  )
RETURNING {}
"#,
            task_select_columns()
        ))
        .bind(tenant_id)
        .bind(task_id)
        .bind(worker_id)
        .bind(lease_until)
        .bind(now)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn complete_task(
        &self,
        tenant_id: i64,
        task_id: i64,
        trace_ref: &Value,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_eval_task
SET status = 'succeeded',
    trace_ref = $3,
    lease_owner = NULL,
    lease_until = NULL,
    finished_at = $5,
    update_user = $4,
    update_time = $5
WHERE tenant_id = $1 AND id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(task_id)
        .bind(trace_ref)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn fail_task(
        &self,
        tenant_id: i64,
        task_id: i64,
        status: &str,
        error: &str,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_eval_task
SET status = $3,
    last_error = $4,
    lease_owner = NULL,
    lease_until = NULL,
    finished_at = CASE WHEN $3 IN ('failed', 'dead') THEN $6 ELSE finished_at END,
    update_user = $5,
    update_time = $6
WHERE tenant_id = $1 AND id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(task_id)
        .bind(status)
        .bind(error)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn find_task(
        &self,
        tenant_id: i64,
        task_id: i64,
    ) -> Result<Option<EvalTaskRecord>, AppError> {
        Ok(sqlx::query_as::<_, EvalTaskRecord>(&format!(
            "SELECT {} FROM ai_eval_task WHERE tenant_id = $1 AND id = $2",
            task_select_columns()
        ))
        .bind(tenant_id)
        .bind(task_id)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn list_run_tasks(
        &self,
        tenant_id: i64,
        run_id: i64,
    ) -> Result<Vec<EvalTaskSummaryRecord>, AppError> {
        Ok(sqlx::query_as::<_, EvalTaskSummaryRecord>(
            r#"
SELECT id, status, attempt, max_attempts
FROM ai_eval_task
WHERE tenant_id = $1 AND run_id = $2
ORDER BY id ASC;
"#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .fetch_all(&self.db)
        .await?)
    }

    pub async fn count_runs(&self, filter: &EvalRunFilter<'_>) -> Result<i64, AppError> {
        let mut query = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ai_eval_run AS r");
        query
            .push(" WHERE r.tenant_id = ")
            .push_bind(filter.tenant_id);
        if let Some(code) = non_empty(filter.dataset_code) {
            query
                .push(" AND r.dataset_code = ")
                .push_bind(code.to_owned());
        }
        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_runs(
        &self,
        filter: &EvalRunFilter<'_>,
    ) -> Result<Vec<EvalRunRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(run_select_sql());
        query
            .push(" WHERE r.tenant_id = ")
            .push_bind(filter.tenant_id);
        if let Some(code) = non_empty(filter.dataset_code) {
            query
                .push(" AND r.dataset_code = ")
                .push_bind(code.to_owned());
        }
        query
            .push(" ORDER BY r.create_time DESC, r.id DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);
        Ok(query
            .build_query_as::<EvalRunRecord>()
            .fetch_all(&self.db)
            .await?)
    }

    pub async fn find_run(
        &self,
        tenant_id: i64,
        run_id: i64,
    ) -> Result<Option<EvalRunRecord>, AppError> {
        Ok(sqlx::query_as::<_, EvalRunRecord>(&format!(
            "{} WHERE r.tenant_id = $1 AND r.id = $2",
            run_select_sql()
        ))
        .bind(tenant_id)
        .bind(run_id)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn update_run_status(
        &self,
        tenant_id: i64,
        run_id: i64,
        status: &str,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_eval_run
SET status = $3,
    started_at = COALESCE(started_at, $5),
    update_user = $4,
    update_time = $5
WHERE tenant_id = $1 AND id = $2 AND status IN ('queued', 'running');
"#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .bind(status)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn update_run_summary(
        &self,
        tenant_id: i64,
        run_id: i64,
        status: &str,
        total_cases: i32,
        passed_cases: i32,
        failed_cases: i32,
        average_score: f64,
        metric_breakdown: &Value,
        report_payload: &Value,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_eval_run
SET status = $3,
    total_cases = $4,
    passed_cases = $5,
    failed_cases = $6,
    average_score = $7,
    metric_breakdown = $8,
    report_payload = $9,
    finished_at = $11,
    update_user = $10,
    update_time = $11
WHERE tenant_id = $1 AND id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .bind(status)
        .bind(total_cases)
        .bind(passed_cases)
        .bind(failed_cases)
        .bind(average_score)
        .bind(metric_breakdown)
        .bind(report_payload)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn count_results(&self, filter: &EvalResultFilter) -> Result<i64, AppError> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM ai_eval_result WHERE tenant_id = $1 AND run_id = $2",
        )
        .bind(filter.tenant_id)
        .bind(filter.run_id)
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn list_results(
        &self,
        filter: &EvalResultFilter,
    ) -> Result<Vec<EvalResultRecord>, AppError> {
        Ok(sqlx::query_as::<_, EvalResultRecord>(
            r#"
SELECT
    id, run_id, case_id, case_code, target_kind, metric_kind, score, passed,
    expected_payload, actual_payload, COALESCE(reason, '') AS reason,
    cost_cents, latency_ms, create_time
FROM ai_eval_result
WHERE tenant_id = $1 AND run_id = $2
ORDER BY id ASC
LIMIT $3 OFFSET $4;
"#,
        )
        .bind(filter.tenant_id)
        .bind(filter.run_id)
        .bind(filter.limit)
        .bind(filter.offset)
        .fetch_all(&self.db)
        .await?)
    }
}

fn dataset_select_sql() -> &'static str {
    r#"
SELECT
    d.id,
    d.code,
    d.name,
    COALESCE(d.description, '') AS description,
    d.target_scope,
    d.status,
    d.metadata,
    COUNT(c.id) AS case_count,
    d.create_time
FROM ai_eval_dataset AS d
LEFT JOIN ai_eval_case AS c ON c.dataset_id = d.id AND c.status = 1
"#
}

fn case_select_sql() -> &'static str {
    r#"
SELECT
    c.id,
    c.dataset_id,
    c.case_code,
    c.target_kind,
    c.metric_kind,
    c.prompt,
    c.expected_payload,
    c.tags,
    c.status,
    c.sort,
    c.create_time
FROM ai_eval_case AS c
"#
}

fn run_select_sql() -> &'static str {
    r#"
SELECT
    r.id,
    r.dataset_id,
    r.dataset_code,
    r.status,
    r.total_cases,
    r.passed_cases,
    r.failed_cases,
    r.average_score,
    r.metric_breakdown,
    r.report_payload,
    r.create_time,
    r.finished_at
FROM ai_eval_run AS r
"#
}

fn task_select_columns() -> &'static str {
    r#"
id,
tenant_id,
run_id,
dataset_id,
case_id,
case_code,
target_kind,
metric_kind,
run_mode,
status,
attempt,
max_attempts,
input_snapshot,
expected_snapshot,
tags_snapshot,
runtime_config,
trace_ref,
COALESCE(last_error, '') AS last_error,
create_user
"#
}

fn push_dataset_filters(query: &mut QueryBuilder<'_, Postgres>, filter: &EvalDatasetFilter<'_>) {
    if let Some(status) = filter.status.filter(|value| *value > 0) {
        query.push(" AND d.status = ").push_bind(status);
    }
    if let Some(code) = non_empty(filter.code) {
        query.push(" AND d.code = ").push_bind(code.to_owned());
    }
}

fn push_case_filters(query: &mut QueryBuilder<'_, Postgres>, filter: &EvalCaseFilter<'_>) {
    if let Some(status) = filter.status.filter(|value| *value > 0) {
        query.push(" AND c.status = ").push_bind(status);
    }
    if let Some(target_kind) = non_empty(filter.target_kind) {
        query
            .push(" AND c.target_kind = ")
            .push_bind(target_kind.to_owned());
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    #[test]
    fn eval_repository_exposes_task_queue_methods() {
        let source = include_str!("ai_eval_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "create_task",
            "create_outbox",
            "create_run_with_tasks_and_outbox",
            "list_pending_eval_outbox",
            "mark_eval_outbox_published",
            "try_start_task",
            "complete_task",
            "fail_task",
            "list_run_tasks",
            "update_run_status",
            "update_run_summary",
        ] {
            assert!(
                source.contains(needle),
                "{needle} missing from eval repository"
            );
        }
    }

    #[test]
    fn eval_outbox_scan_retries_publish_failures() {
        let source = include_str!("ai_eval_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(
            source.contains("WHERE status IN (1, 3)"),
            "eval outbox scanner must retry records that failed to publish"
        );
    }

    #[test]
    fn eval_task_lease_allows_same_worker_reentry() {
        let source = include_str!("ai_eval_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(
            source.contains("OR (status = 'running' AND lease_owner = $3)"),
            "same worker must be able to resume its own running lease after message redelivery"
        );
    }
}
