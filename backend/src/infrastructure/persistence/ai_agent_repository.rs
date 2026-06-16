use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};

use crate::shared::error::AppError;

#[derive(Debug, Clone)]
pub struct AiAgentRepository {
    db: PgPool,
}

#[derive(Debug, Clone)]
pub struct RunSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_type: String,
    pub status: String,
    pub source_type: String,
    pub source_id: Option<String>,
    pub trace_id: String,
    pub input_payload: Value,
    pub output_payload: Value,
    pub budget_policy: Value,
    pub created_by: i64,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct AgentRunSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub intent: String,
    pub loop_kind: String,
    pub selected_tool_code: Option<String>,
    pub status: String,
    pub pause_reason: Option<String>,
    pub task_budget: Value,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct AgentTraceSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub trace_id: String,
    pub event_snapshot: Value,
    pub model_route_snapshot: Value,
    pub tool_snapshot: Value,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct AgentRolloutSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub trace_id: String,
    pub event_bundle: Value,
    pub summary_payload: Value,
    pub source: String,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct RunStepSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub parent_step_id: Option<i64>,
    pub step_type: String,
    pub status: String,
    pub sequence_no: i64,
    pub input_payload: Value,
    pub output_payload: Value,
    pub tool_call_audit_id: Option<i64>,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct RunStatusUpdate<'a> {
    pub tenant_id: i64,
    pub run_id: i64,
    pub status: &'a str,
    pub output_payload: &'a Value,
    pub finished: bool,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct AgentRunStatusUpdate<'a> {
    pub tenant_id: i64,
    pub run_id: i64,
    pub status: &'a str,
    pub final_output: Option<&'a str>,
    pub pause_reason: Option<&'a str>,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct RunEventSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub step_id: Option<i64>,
    pub event_type: String,
    pub sequence_no: i64,
    pub status: String,
    pub payload: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct RunPauseSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub step_id: Option<i64>,
    pub pause_reason: String,
    pub requested_input_schema: Value,
    pub resume_token_hash: Option<String>,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct AgentRunFilter<'a> {
    pub tenant_id: i64,
    pub status: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct RunEventFilter {
    pub tenant_id: i64,
    pub run_id: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct RunEventCursorFilter {
    pub tenant_id: i64,
    pub run_id: i64,
    pub after_sequence_no: i64,
    pub limit: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct AgentRunRecord {
    pub run_id: i64,
    pub trace_id: String,
    pub status: String,
    pub intent: String,
    pub loop_kind: String,
    pub selected_tool_code: Option<String>,
    pub pause_reason: Option<String>,
    pub final_output: Option<String>,
    pub task_budget: Value,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, FromRow)]
pub struct RunEventRecord {
    pub id: i64,
    pub run_id: i64,
    pub step_id: Option<i64>,
    pub event_type: String,
    pub sequence_no: i64,
    pub status: String,
    pub payload: Value,
    pub create_time: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct RunPauseRecord {
    pub id: i64,
    pub step_id: Option<i64>,
    pub pause_reason: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct AgentRolloutRecord {
    pub id: i64,
    pub run_id: i64,
    pub trace_id: String,
    pub event_bundle: Value,
    pub summary_payload: Value,
    pub source: String,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

impl AiAgentRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn create_run(&self, record: &RunSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_run (
    id, tenant_id, run_type, status, source_type, source_id, trace_id,
    input_payload, output_payload, budget_policy, created_by, started_at,
    create_user, create_time, update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $13, $14);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(&record.run_type)
        .bind(&record.status)
        .bind(&record.source_type)
        .bind(&record.source_id)
        .bind(&record.trace_id)
        .bind(&record.input_payload)
        .bind(&record.output_payload)
        .bind(&record.budget_policy)
        .bind(record.created_by)
        .bind(record.now)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn create_agent_run(&self, record: &AgentRunSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_agent_run (
    id, tenant_id, run_id, intent, loop_kind, selected_tool_code, status,
    pause_reason, task_budget, metadata, create_user, create_time, update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $11, $12);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.run_id)
        .bind(&record.intent)
        .bind(&record.loop_kind)
        .bind(&record.selected_tool_code)
        .bind(&record.status)
        .bind(&record.pause_reason)
        .bind(&record.task_budget)
        .bind(&record.metadata)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn create_agent_trace(&self, record: &AgentTraceSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_agent_trace (
    id, tenant_id, run_id, trace_id, event_snapshot, model_route_snapshot,
    tool_snapshot, metadata, create_user, create_time, update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $9, $10);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.run_id)
        .bind(&record.trace_id)
        .bind(&record.event_snapshot)
        .bind(&record.model_route_snapshot)
        .bind(&record.tool_snapshot)
        .bind(&record.metadata)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn create_step(&self, record: &RunStepSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_run_step (
    id, tenant_id, run_id, parent_step_id, step_type, status, sequence_no,
    input_payload, output_payload, tool_call_audit_id, create_user, create_time,
    update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $11, $12);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.run_id)
        .bind(record.parent_step_id)
        .bind(&record.step_type)
        .bind(&record.status)
        .bind(record.sequence_no)
        .bind(&record.input_payload)
        .bind(&record.output_payload)
        .bind(record.tool_call_audit_id)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn create_event(&self, record: &RunEventSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_run_event (
    id, tenant_id, run_id, step_id, event_type, sequence_no, status,
    payload, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.run_id)
        .bind(record.step_id)
        .bind(&record.event_type)
        .bind(record.sequence_no)
        .bind(&record.status)
        .bind(&record.payload)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn create_pause(&self, record: &RunPauseSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_run_pause (
    id, tenant_id, run_id, step_id, pause_reason, status, requested_input_schema,
    resume_token_hash, create_user, create_time, update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, 'active', $6, $7, $8, $9, $8, $9);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.run_id)
        .bind(record.step_id)
        .bind(&record.pause_reason)
        .bind(&record.requested_input_schema)
        .bind(&record.resume_token_hash)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn update_run_status(&self, update: &RunStatusUpdate<'_>) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_run
SET status = $3,
    output_payload = $4,
    finished_at = CASE WHEN $5 THEN $7 ELSE finished_at END,
    update_user = $6,
    update_time = $7
WHERE tenant_id = $1 AND id = $2;
"#,
        )
        .bind(update.tenant_id)
        .bind(update.run_id)
        .bind(update.status)
        .bind(update.output_payload)
        .bind(update.finished)
        .bind(update.user_id)
        .bind(update.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn update_agent_run_status(
        &self,
        update: &AgentRunStatusUpdate<'_>,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_agent_run
SET status = $3,
    final_output = COALESCE($4, final_output),
    pause_reason = $5,
    update_user = $6,
    update_time = $7
WHERE tenant_id = $1 AND run_id = $2;
"#,
        )
        .bind(update.tenant_id)
        .bind(update.run_id)
        .bind(update.status)
        .bind(update.final_output)
        .bind(update.pause_reason)
        .bind(update.user_id)
        .bind(update.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn update_trace_snapshot(
        &self,
        tenant_id: i64,
        run_id: i64,
        event_snapshot: &Value,
        tool_snapshot: &Value,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_agent_trace
SET event_snapshot = $3,
    tool_snapshot = $4,
    update_user = $5,
    update_time = $6
WHERE tenant_id = $1 AND run_id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .bind(event_snapshot)
        .bind(tool_snapshot)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn upsert_rollout_bundle(
        &self,
        record: &AgentRolloutSaveRecord,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_rollout (
    id, tenant_id, run_id, trace_id, event_bundle, summary_payload, source,
    create_user, create_time, update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $8, $9)
ON CONFLICT (run_id, source)
DO UPDATE SET
    trace_id = EXCLUDED.trace_id,
    event_bundle = EXCLUDED.event_bundle,
    summary_payload = EXCLUDED.summary_payload,
    update_user = EXCLUDED.update_user,
    update_time = EXCLUDED.update_time;
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.run_id)
        .bind(&record.trace_id)
        .bind(&record.event_bundle)
        .bind(&record.summary_payload)
        .bind(&record.source)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn complete_pause(
        &self,
        tenant_id: i64,
        pause_id: i64,
        status: &str,
        payload: &Value,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_run_pause
SET status = $3,
    resume_payload = $4,
    resumed_at = $6,
    update_user = $5,
    update_time = $6
WHERE tenant_id = $1 AND id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(pause_id)
        .bind(status)
        .bind(payload)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn cancel_active_pauses(
        &self,
        tenant_id: i64,
        run_id: i64,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_run_pause
SET status = 'cancelled',
    update_user = $3,
    update_time = $4
WHERE tenant_id = $1 AND run_id = $2 AND status = 'active';
"#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn find_run(
        &self,
        tenant_id: i64,
        run_id: i64,
    ) -> Result<Option<AgentRunRecord>, AppError> {
        let sql = agent_run_select_sql_with_where("WHERE r.tenant_id = $1 AND r.id = $2");
        Ok(sqlx::query_as::<_, AgentRunRecord>(&sql)
            .bind(tenant_id)
            .bind(run_id)
            .fetch_optional(&self.db)
            .await?)
    }

    pub async fn count_runs(&self, filter: &AgentRunFilter<'_>) -> Result<i64, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT COUNT(*) FROM ai_run AS r INNER JOIN ai_agent_run AS ar ON ar.run_id = r.id",
        );
        query
            .push(" WHERE r.tenant_id = ")
            .push_bind(filter.tenant_id);
        if let Some(status) = non_empty(filter.status) {
            query.push(" AND r.status = ").push_bind(status.to_owned());
        }

        Ok(query
            .build_query_scalar::<i64>()
            .fetch_one(&self.db)
            .await?)
    }

    pub async fn list_runs(
        &self,
        filter: &AgentRunFilter<'_>,
    ) -> Result<Vec<AgentRunRecord>, AppError> {
        let mut query = QueryBuilder::<Postgres>::new(agent_run_select_sql());
        query
            .push(" WHERE r.tenant_id = ")
            .push_bind(filter.tenant_id);
        if let Some(status) = non_empty(filter.status) {
            query.push(" AND r.status = ").push_bind(status.to_owned());
        }
        query
            .push(" ORDER BY r.create_time DESC, r.id DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);

        Ok(query
            .build_query_as::<AgentRunRecord>()
            .fetch_all(&self.db)
            .await?)
    }

    pub async fn count_events(&self, filter: &RunEventFilter) -> Result<i64, AppError> {
        Ok(sqlx::query_scalar::<_, i64>(
            r#"
SELECT COUNT(*)
FROM ai_run_event
WHERE tenant_id = $1 AND run_id = $2;
"#,
        )
        .bind(filter.tenant_id)
        .bind(filter.run_id)
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn list_events(
        &self,
        filter: &RunEventFilter,
    ) -> Result<Vec<RunEventRecord>, AppError> {
        Ok(sqlx::query_as::<_, RunEventRecord>(
            r#"
SELECT id, run_id, step_id, event_type, sequence_no, status, payload, create_time
FROM ai_run_event
WHERE tenant_id = $1 AND run_id = $2
ORDER BY sequence_no ASC
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

    pub async fn list_events_after_sequence(
        &self,
        filter: &RunEventCursorFilter,
    ) -> Result<Vec<RunEventRecord>, AppError> {
        Ok(sqlx::query_as::<_, RunEventRecord>(
            r#"
SELECT id, run_id, step_id, event_type, sequence_no, status, payload, create_time
FROM ai_run_event
WHERE tenant_id = $1 AND run_id = $2 AND sequence_no > $3
ORDER BY sequence_no ASC
LIMIT $4;
"#,
        )
        .bind(filter.tenant_id)
        .bind(filter.run_id)
        .bind(filter.after_sequence_no)
        .bind(filter.limit)
        .fetch_all(&self.db)
        .await?)
    }

    pub async fn find_rollout_by_run_id(
        &self,
        tenant_id: i64,
        run_id: i64,
    ) -> Result<Option<AgentRolloutRecord>, AppError> {
        Ok(sqlx::query_as::<_, AgentRolloutRecord>(
            r#"
SELECT id, run_id, trace_id, event_bundle, summary_payload, source, create_time, update_time
FROM ai_rollout
WHERE tenant_id = $1 AND run_id = $2
ORDER BY COALESCE(update_time, create_time) DESC, id DESC
LIMIT 1;
"#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn find_rollout_by_trace_id(
        &self,
        tenant_id: i64,
        trace_id: &str,
    ) -> Result<Option<AgentRolloutRecord>, AppError> {
        Ok(sqlx::query_as::<_, AgentRolloutRecord>(
            r#"
SELECT id, run_id, trace_id, event_bundle, summary_payload, source, create_time, update_time
FROM ai_rollout
WHERE tenant_id = $1 AND trace_id = $2
ORDER BY COALESCE(update_time, create_time) DESC, id DESC
LIMIT 1;
"#,
        )
        .bind(tenant_id)
        .bind(trace_id)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn next_event_sequence(&self, tenant_id: i64, run_id: i64) -> Result<i64, AppError> {
        Ok(sqlx::query_scalar::<_, i64>(
            r#"
SELECT COALESCE(MAX(sequence_no), 0) + 1
FROM ai_run_event
WHERE tenant_id = $1 AND run_id = $2;
"#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn find_active_pause(
        &self,
        tenant_id: i64,
        run_id: i64,
    ) -> Result<Option<RunPauseRecord>, AppError> {
        Ok(sqlx::query_as::<_, RunPauseRecord>(
            r#"
SELECT id, step_id, pause_reason
FROM ai_run_pause
WHERE tenant_id = $1 AND run_id = $2 AND status = 'active'
ORDER BY create_time DESC, id DESC
LIMIT 1;
"#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .fetch_optional(&self.db)
        .await?)
    }
}

fn agent_run_select_sql() -> &'static str {
    r#"
SELECT
    r.id AS run_id,
    r.trace_id,
    r.status,
    ar.intent,
    ar.loop_kind,
    ar.selected_tool_code,
    ar.pause_reason,
    ar.final_output,
    ar.task_budget,
    r.create_time,
    r.update_time
FROM ai_run AS r
INNER JOIN ai_agent_run AS ar ON ar.run_id = r.id
"#
}

fn agent_run_select_sql_with_where(where_clause: &'static str) -> String {
    format!("{} {}", agent_run_select_sql(), where_clause)
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    #[test]
    fn agent_event_stream_repository_uses_sequence_cursor() {
        let source = include_str!("ai_agent_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("RunEventCursorFilter"));
        assert!(source.contains("list_events_after_sequence"));
        assert!(source.contains("sequence_no > $3"));
        assert!(source.contains("ORDER BY sequence_no ASC"));
        assert!(source.contains("LIMIT $4"));
    }
}
