use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};

use crate::shared::error::AppError;

#[derive(Debug, Clone)]
pub struct AiAgentRepository {
    db: PgPool,
}

pub const AGENT_RUN_QUEUE_STATUS_PENDING: &str = "pending";
pub const AGENT_RUN_QUEUE_STATUS_RUNNING: &str = "running";
pub const AGENT_RUN_QUEUE_STATUS_RETRYING: &str = "retrying";
pub const AGENT_RUN_QUEUE_STATUS_WAITING_APPROVAL: &str = "waiting_approval";
pub const AGENT_RUN_QUEUE_STATUS_SUCCEEDED: &str = "succeeded";
pub const AGENT_RUN_QUEUE_STATUS_FAILED: &str = "failed";
pub const AGENT_RUN_QUEUE_STATUS_CANCELLED: &str = "cancelled";

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
    pub started_at: Option<NaiveDateTime>,
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
pub struct AgentTurnItemSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub step_id: Option<i64>,
    pub source_event_id: i64,
    pub sequence_no: i64,
    pub item_type: String,
    pub call_id: Option<String>,
    pub tool_code: Option<String>,
    pub item_payload: Value,
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
pub struct AgentRunQueueSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub priority: i32,
    pub max_attempts: i32,
    pub payload: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct AgentQueueOutboxSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub queue_id: i64,
    pub run_id: i64,
    pub event_type: String,
    pub max_attempts: i32,
    pub payload: Value,
    pub status: i16,
    pub attempt_count: i32,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct AgentRunQueueClaimRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub payload: Value,
}

#[derive(Debug, Clone, FromRow)]
pub struct AgentQueueOutboxRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub queue_id: i64,
    pub run_id: i64,
    pub event_type: String,
    pub max_attempts: i32,
    pub payload: Value,
    pub status: i16,
    pub attempt_count: i32,
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
pub struct AgentTurnItemFilter {
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
pub struct AgentTurnItemRecord {
    pub id: i64,
    pub run_id: i64,
    pub step_id: Option<i64>,
    pub source_event_id: i64,
    pub sequence_no: i64,
    pub item_type: String,
    pub call_id: Option<String>,
    pub tool_code: Option<String>,
    pub item_payload: Value,
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
        .bind(record.started_at)
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
        self.create_event_with_turn_item(record, None).await
    }

    pub async fn create_event_with_turn_item(
        &self,
        record: &RunEventSaveRecord,
        turn_item: Option<&AgentTurnItemSaveRecord>,
    ) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;
        insert_run_event(&mut tx, record).await?;
        if let Some(turn_item) = turn_item {
            insert_agent_turn_item(&mut tx, turn_item).await?;
        }
        tx.commit().await?;
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

    pub async fn enqueue_agent_run(
        &self,
        record: &AgentRunQueueSaveRecord,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_agent_run_queue (
    id, tenant_id, run_id, queue_status, priority, attempt_count, max_attempts,
    payload, queued_at, create_user, create_time, update_user, update_time
)
VALUES ($1, $2, $3, 'pending', $4, 0, $5, $6, $8, $7, $8, $7, $8);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.run_id)
        .bind(record.priority)
        .bind(record.max_attempts)
        .bind(&record.payload)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn enqueue_agent_run_with_outbox(
        &self,
        record: &AgentRunQueueSaveRecord,
        outbox: &AgentQueueOutboxSaveRecord,
    ) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
INSERT INTO ai_agent_run_queue (
    id, tenant_id, run_id, queue_status, priority, attempt_count, max_attempts,
    payload, queued_at, create_user, create_time, update_user, update_time
)
VALUES ($1, $2, $3, 'pending', $4, 0, $5, $6, $8, $7, $8, $7, $8);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.run_id)
        .bind(record.priority)
        .bind(record.max_attempts)
        .bind(&record.payload)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&mut *tx)
        .await?;

        insert_agent_queue_outbox(&mut tx, outbox).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn claim_agent_run_queue(
        &self,
        tenant_id: Option<i64>,
        limit: i64,
        worker_id: &str,
        lease_until: NaiveDateTime,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<Vec<AgentRunQueueClaimRecord>, AppError> {
        Ok(sqlx::query_as::<_, AgentRunQueueClaimRecord>(
            r#"
WITH candidate AS (
    SELECT id
    FROM ai_agent_run_queue
    WHERE queue_status IN ('pending', 'retrying')
      AND ($1::BIGINT IS NULL OR tenant_id = $1)
      AND (locked_until IS NULL OR locked_until <= $3)
      AND attempt_count < max_attempts
    ORDER BY priority DESC, queued_at ASC, id ASC
    LIMIT $2
    FOR UPDATE SKIP LOCKED
)
UPDATE ai_agent_run_queue AS q
SET queue_status = 'running',
    attempt_count = q.attempt_count + 1,
    locked_by = $4,
    locked_until = $5,
    started_at = COALESCE(q.started_at, $3),
    update_user = $6,
    update_time = $3
FROM candidate
WHERE q.id = candidate.id
RETURNING q.id, q.tenant_id, q.run_id, q.attempt_count, q.max_attempts, q.payload;
"#,
        )
        .bind(tenant_id)
        .bind(limit)
        .bind(now)
        .bind(worker_id)
        .bind(lease_until)
        .bind(user_id)
        .fetch_all(&self.db)
        .await?)
    }

    pub async fn claim_agent_run_queue_by_message(
        &self,
        queue_id: i64,
        tenant_id: i64,
        run_id: i64,
        worker_id: &str,
        lease_until: NaiveDateTime,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<Option<AgentRunQueueClaimRecord>, AppError> {
        Ok(sqlx::query_as::<_, AgentRunQueueClaimRecord>(
            r#"
WITH candidate AS (
    SELECT id
    FROM ai_agent_run_queue
    WHERE id = $1
      AND tenant_id = $2
      AND run_id = $3
      AND queue_status IN ('pending', 'retrying')
      AND (locked_until IS NULL OR locked_until <= $4)
      AND attempt_count < max_attempts
    FOR UPDATE SKIP LOCKED
)
UPDATE ai_agent_run_queue AS q
SET queue_status = 'running',
    attempt_count = q.attempt_count + 1,
    locked_by = $5,
    locked_until = $6,
    started_at = COALESCE(q.started_at, $4),
    update_user = $7,
    update_time = $4
FROM candidate
WHERE q.id = candidate.id
RETURNING q.id, q.tenant_id, q.run_id, q.attempt_count, q.max_attempts, q.payload;
"#,
        )
        .bind(queue_id)
        .bind(tenant_id)
        .bind(run_id)
        .bind(now)
        .bind(worker_id)
        .bind(lease_until)
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn mark_agent_run_queue_succeeded(
        &self,
        queue_id: i64,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        self.mark_agent_run_queue_terminal(
            queue_id,
            AGENT_RUN_QUEUE_STATUS_SUCCEEDED,
            None,
            user_id,
            now,
        )
        .await
    }

    pub async fn mark_agent_run_queue_failed(
        &self,
        queue_id: i64,
        last_error: &str,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        self.mark_agent_run_queue_terminal(
            queue_id,
            AGENT_RUN_QUEUE_STATUS_FAILED,
            Some(last_error),
            user_id,
            now,
        )
        .await
    }

    pub async fn mark_agent_run_queue_cancelled(
        &self,
        queue_id: i64,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        self.mark_agent_run_queue_terminal(
            queue_id,
            AGENT_RUN_QUEUE_STATUS_CANCELLED,
            None,
            user_id,
            now,
        )
        .await
    }

    pub async fn mark_agent_run_queue_waiting_approval(
        &self,
        queue_id: i64,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_agent_run_queue
SET queue_status = $2,
    locked_by = NULL,
    locked_until = NULL,
    update_user = $3,
    update_time = $4
WHERE id = $1;
"#,
        )
        .bind(queue_id)
        .bind(AGENT_RUN_QUEUE_STATUS_WAITING_APPROVAL)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn cancel_agent_run_queue_for_run(
        &self,
        tenant_id: i64,
        run_id: i64,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<u64, AppError> {
        let result = sqlx::query(
            r#"
UPDATE ai_agent_run_queue
SET queue_status = $3,
    locked_by = NULL,
    locked_until = NULL,
    last_error = COALESCE(last_error, $4),
    finished_at = $6,
    update_user = $5,
    update_time = $6
WHERE tenant_id = $1
  AND run_id = $2
  AND queue_status IN ('pending', 'retrying');
"#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .bind(AGENT_RUN_QUEUE_STATUS_CANCELLED)
        .bind("run cancelled before claim")
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn requeue_agent_run_for_resume(
        &self,
        tenant_id: i64,
        run_id: i64,
        payload: &Value,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<u64, AppError> {
        let result = sqlx::query(
            r#"
UPDATE ai_agent_run_queue
SET queue_status = $3,
    attempt_count = 0,
    locked_by = NULL,
    locked_until = NULL,
    last_error = NULL,
    payload = $4,
    queued_at = $6,
    started_at = NULL,
    finished_at = NULL,
    update_user = $5,
    update_time = $6
WHERE tenant_id = $1
  AND run_id = $2
  AND queue_status IN ('waiting_approval', 'succeeded');
"#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .bind(AGENT_RUN_QUEUE_STATUS_PENDING)
        .bind(payload)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn requeue_agent_run_for_resume_with_outbox(
        &self,
        tenant_id: i64,
        run_id: i64,
        payload: &Value,
        outbox: &AgentQueueOutboxSaveRecord,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<u64, AppError> {
        let result = sqlx::query(
            r#"
WITH updated AS (
    UPDATE ai_agent_run_queue
    SET queue_status = $3,
        attempt_count = 0,
        locked_by = NULL,
        locked_until = NULL,
        last_error = NULL,
        payload = $4,
        queued_at = $6,
        started_at = NULL,
        finished_at = NULL,
        update_user = $5,
        update_time = $6
    WHERE tenant_id = $1
      AND run_id = $2
      AND queue_status IN ('waiting_approval', 'succeeded')
    RETURNING id, max_attempts
)
INSERT INTO ai_agent_queue_outbox (
    id, tenant_id, queue_id, run_id, event_type, max_attempts, payload,
    status, attempt_count, create_user, create_time
)
SELECT $7, $1, updated.id, $2, $8, updated.max_attempts, $9, $10, $11, $5, $6
FROM updated
ON CONFLICT (tenant_id, queue_id, event_type) DO UPDATE
SET max_attempts = EXCLUDED.max_attempts,
    payload = EXCLUDED.payload,
    status = EXCLUDED.status,
    attempt_count = EXCLUDED.attempt_count,
    last_error = NULL,
    published_time = NULL,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .bind(AGENT_RUN_QUEUE_STATUS_PENDING)
        .bind(payload)
        .bind(user_id)
        .bind(now)
        .bind(outbox.id)
        .bind(&outbox.event_type)
        .bind(&outbox.payload)
        .bind(outbox.status)
        .bind(outbox.attempt_count)
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_pending_agent_queue_outbox(
        &self,
        limit: i64,
    ) -> Result<Vec<AgentQueueOutboxRecord>, AppError> {
        Ok(sqlx::query_as::<_, AgentQueueOutboxRecord>(
            r#"
SELECT
    id,
    tenant_id,
    queue_id,
    run_id,
    event_type,
    max_attempts,
    payload,
    status,
    attempt_count
FROM ai_agent_queue_outbox
WHERE status = 1
ORDER BY create_time ASC, id ASC
LIMIT $1;
"#,
        )
        .bind(limit.max(1))
        .fetch_all(&self.db)
        .await?)
    }

    pub async fn mark_agent_queue_outbox_published(
        &self,
        outbox_id: i64,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_agent_queue_outbox
SET status = 2,
    published_time = $1,
    last_error = NULL,
    update_user = $2,
    update_time = $1
WHERE id = $3;
"#,
        )
        .bind(now)
        .bind(user_id)
        .bind(outbox_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn mark_agent_queue_outbox_publish_failed(
        &self,
        outbox_id: i64,
        error: &str,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_agent_queue_outbox
SET status = 1,
    attempt_count = attempt_count + 1,
    last_error = $1,
    update_user = $2,
    update_time = $3
WHERE id = $4;
"#,
        )
        .bind(error)
        .bind(user_id)
        .bind(now)
        .bind(outbox_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn mark_agent_run_queue_retrying(
        &self,
        queue_id: i64,
        last_error: &str,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_agent_run_queue
SET queue_status = $2,
    locked_by = NULL,
    locked_until = NULL,
    last_error = $3,
    update_user = $4,
    update_time = $5
WHERE id = $1;
"#,
        )
        .bind(queue_id)
        .bind(AGENT_RUN_QUEUE_STATUS_RETRYING)
        .bind(last_error)
        .bind(user_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn mark_agent_run_queue_terminal(
        &self,
        queue_id: i64,
        status: &str,
        last_error: Option<&str>,
        user_id: i64,
        now: NaiveDateTime,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
UPDATE ai_agent_run_queue
SET queue_status = $2,
    locked_by = NULL,
    locked_until = NULL,
    last_error = COALESCE($3, last_error),
    finished_at = $5,
    update_user = $4,
    update_time = $5
WHERE id = $1;
"#,
        )
        .bind(queue_id)
        .bind(status)
        .bind(last_error)
        .bind(user_id)
        .bind(now)
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

    pub async fn list_turn_items(
        &self,
        filter: &AgentTurnItemFilter,
    ) -> Result<Vec<AgentTurnItemRecord>, AppError> {
        Ok(sqlx::query_as::<_, AgentTurnItemRecord>(
            r#"
SELECT
    id, run_id, step_id, source_event_id, sequence_no, item_type,
    call_id, tool_code, item_payload, create_time
FROM ai_agent_turn_item
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

async fn insert_run_event(
    tx: &mut Transaction<'_, Postgres>,
    record: &RunEventSaveRecord,
) -> Result<(), AppError> {
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
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_agent_turn_item(
    tx: &mut Transaction<'_, Postgres>,
    record: &AgentTurnItemSaveRecord,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
INSERT INTO ai_agent_turn_item (
    id, tenant_id, run_id, step_id, source_event_id, sequence_no,
    item_type, call_id, tool_code, item_payload, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12);
"#,
    )
    .bind(record.id)
    .bind(record.tenant_id)
    .bind(record.run_id)
    .bind(record.step_id)
    .bind(record.source_event_id)
    .bind(record.sequence_no)
    .bind(&record.item_type)
    .bind(&record.call_id)
    .bind(&record.tool_code)
    .bind(&record.item_payload)
    .bind(record.user_id)
    .bind(record.now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_agent_queue_outbox(
    tx: &mut Transaction<'_, Postgres>,
    outbox: &AgentQueueOutboxSaveRecord,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
INSERT INTO ai_agent_queue_outbox (
    id, tenant_id, queue_id, run_id, event_type, max_attempts, payload,
    status, attempt_count, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
ON CONFLICT (tenant_id, queue_id, event_type) DO UPDATE
SET max_attempts = EXCLUDED.max_attempts,
    payload = EXCLUDED.payload,
    status = EXCLUDED.status,
    attempt_count = EXCLUDED.attempt_count,
    last_error = NULL,
    published_time = NULL,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
    )
    .bind(outbox.id)
    .bind(outbox.tenant_id)
    .bind(outbox.queue_id)
    .bind(outbox.run_id)
    .bind(&outbox.event_type)
    .bind(outbox.max_attempts)
    .bind(&outbox.payload)
    .bind(outbox.status)
    .bind(outbox.attempt_count)
    .bind(outbox.user_id)
    .bind(outbox.now)
    .execute(&mut **tx)
    .await?;
    Ok(())
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
    fn agent_turn_item_migration_defines_response_item_ledger_contract() {
        let migration_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/202606170009_create_ai_agent_turn_item.sql"
        );
        let migration = std::fs::read_to_string(migration_path)
            .expect("missing AI agent turn item ledger migration");

        for needle in [
            "CREATE TABLE IF NOT EXISTS ai_agent_turn_item",
            "source_event_id",
            "sequence_no",
            "item_type",
            "call_id",
            "tool_code",
            "item_payload JSONB",
            "uk_ai_agent_turn_item_run_sequence",
            "uk_ai_agent_turn_item_event",
            "idx_ai_agent_turn_item_run_id",
            "idx_ai_agent_turn_item_type",
            "idx_ai_agent_turn_item_call_id",
        ] {
            assert!(
                migration.contains(needle),
                "{needle} missing from agent turn item ledger migration"
            );
        }
    }

    #[test]
    fn agent_turn_item_repository_exposes_transactional_event_item_ledger() {
        let source = include_str!("ai_agent_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "AgentTurnItemSaveRecord",
            "AgentTurnItemRecord",
            "AgentTurnItemFilter",
            "create_event_with_turn_item",
            "list_turn_items",
            "INSERT INTO ai_agent_turn_item",
            "FROM ai_agent_turn_item",
            "ORDER BY sequence_no ASC",
            "Transaction<'_, Postgres>",
            "source_event_id",
        ] {
            assert!(
                source.contains(needle),
                "{needle} missing from agent turn item repository contract"
            );
        }
    }

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

    #[test]
    fn agent_run_queue_migration_defines_durable_queue_contract() {
        let migration =
            include_str!("../../../migrations/202606170006_create_ai_agent_run_queue.sql");

        assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_agent_run_queue"));
        assert!(migration.contains("queue_status"));
        assert!(migration.contains("locked_by"));
        assert!(migration.contains("locked_until"));
        assert!(migration.contains("attempt_count"));
        assert!(migration.contains("max_attempts"));
        assert!(migration.contains("payload JSONB"));
        assert!(migration.contains("tenant_id, run_id"));
    }

    #[test]
    fn agent_run_queue_repository_exposes_claim_and_status_contract() {
        let source = include_str!("ai_agent_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("AgentRunQueueSaveRecord"));
        assert!(source.contains("AgentRunQueueClaimRecord"));
        assert!(source.contains("AGENT_RUN_QUEUE_STATUS_PENDING"));
        assert!(source.contains("AGENT_RUN_QUEUE_STATUS_RUNNING"));
        assert!(source.contains("AGENT_RUN_QUEUE_STATUS_RETRYING"));
        assert!(source.contains("AGENT_RUN_QUEUE_STATUS_SUCCEEDED"));
        assert!(source.contains("AGENT_RUN_QUEUE_STATUS_FAILED"));
        assert!(source.contains("AGENT_RUN_QUEUE_STATUS_CANCELLED"));
        assert!(source.contains("FOR UPDATE SKIP LOCKED"));
        assert!(source.contains("enqueue_agent_run"));
        assert!(source.contains("claim_agent_run_queue"));
        assert!(source.contains("mark_agent_run_queue_succeeded"));
        assert!(source.contains("mark_agent_run_queue_retrying"));
        assert!(source.contains("mark_agent_run_queue_failed"));
        assert!(source.contains("mark_agent_run_queue_cancelled"));
    }

    #[test]
    fn agent_queue_cancel_sync_repository_cancels_unclaimed_rows_by_run() {
        let source = include_str!("ai_agent_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("cancel_agent_run_queue_for_run"));
        assert!(source.contains("queue_status = $3"));
        assert!(source.contains("AGENT_RUN_QUEUE_STATUS_CANCELLED"));
        assert!(source.contains("queue_status IN ('pending', 'retrying')"));
        assert!(source.contains("locked_by = NULL"));
        assert!(source.contains("locked_until = NULL"));
        assert!(source.contains("finished_at ="));
        assert!(source.contains("rows_affected"));
    }

    #[test]
    fn agent_queue_resume_requeue_repository_tracks_waiting_approval_rows() {
        let source = include_str!("ai_agent_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("AGENT_RUN_QUEUE_STATUS_WAITING_APPROVAL"));
        assert!(source.contains("mark_agent_run_queue_waiting_approval"));
        assert!(source.contains("requeue_agent_run_for_resume"));
        assert!(source.contains("queue_status = $3"));
        assert!(source.contains("attempt_count = 0"));
        assert!(source.contains("payload = $4"));
        assert!(source.contains("locked_by = NULL"));
        assert!(source.contains("locked_until = NULL"));
        assert!(source.contains("finished_at = NULL"));
        assert!(source.contains("queue_status IN ('waiting_approval', 'succeeded')"));
    }

    #[test]
    fn agent_queue_broker_consumer_claims_exact_message_row() {
        let source = include_str!("ai_agent_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("claim_agent_run_queue_by_message"));
        assert!(source.contains("WHERE id = $1"));
        assert!(source.contains("tenant_id = $2"));
        assert!(source.contains("run_id = $3"));
        assert!(source.contains("queue_status IN ('pending', 'retrying')"));
        assert!(source.contains("FOR UPDATE SKIP LOCKED"));
    }

    #[test]
    fn agent_queue_outbox_migration_defines_durable_publish_contract() {
        let migration_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/202606170007_create_ai_agent_queue_outbox.sql"
        );
        let migration = std::fs::read_to_string(migration_path)
            .expect("missing AI agent queue outbox migration");

        for needle in [
            "CREATE TABLE IF NOT EXISTS ai_agent_queue_outbox",
            "queue_id",
            "tenant_id",
            "run_id",
            "event_type",
            "max_attempts",
            "payload JSONB",
            "status",
            "attempt_count",
            "last_error",
            "published_time",
            "uq_ai_agent_queue_outbox_queue_event",
            "idx_ai_agent_queue_outbox_status",
        ] {
            assert!(
                migration.contains(needle),
                "{needle} missing from agent queue outbox migration"
            );
        }
    }

    #[test]
    fn agent_queue_outbox_repository_exposes_transaction_and_publish_state() {
        let source = include_str!("ai_agent_repository.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "AgentQueueOutboxSaveRecord",
            "AgentQueueOutboxRecord",
            "enqueue_agent_run_with_outbox",
            "requeue_agent_run_for_resume_with_outbox",
            "INSERT INTO ai_agent_queue_outbox",
            "ON CONFLICT (tenant_id, queue_id, event_type) DO UPDATE",
            "list_pending_agent_queue_outbox",
            "mark_agent_queue_outbox_published",
            "mark_agent_queue_outbox_publish_failed",
            "FROM ai_agent_queue_outbox",
            "status = 1",
            "status = 2",
            "published_time",
            "attempt_count = attempt_count + 1",
        ] {
            assert!(
                source.contains(needle),
                "{needle} missing from agent queue outbox repository"
            );
        }
    }
}
