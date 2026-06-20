use std::time::Duration;

use chrono::Utc;
use sqlx::PgPool;

use crate::{
    infrastructure::{
        mq::rabbitmq::{EvalRabbitMqClient, EvalRabbitMqConfig, EvalTaskMessage},
        persistence::ai_eval_repository::{AiEvalRepository, EvalOutboxRecord},
    },
    shared::{config::AppConfig, error::AppError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalQueueRuntimeConfig {
    pub enabled: bool,
    pub publisher_enabled: bool,
    pub tick_seconds: u64,
    pub batch_size: i64,
}

pub fn eval_queue_from_config(config: &AppConfig) -> EvalQueueRuntimeConfig {
    EvalQueueRuntimeConfig {
        enabled: config.eval_queue_enabled,
        publisher_enabled: config.eval_queue_publisher_enabled,
        tick_seconds: config.eval_queue_tick_seconds,
        batch_size: config.eval_queue_batch_size,
    }
}

pub fn eval_rabbitmq_from_config(config: &AppConfig) -> EvalRabbitMqConfig {
    EvalRabbitMqConfig {
        url: config.rabbitmq_url.clone(),
        exchange: config.rabbitmq_eval_exchange.clone(),
        execute_queue: config.rabbitmq_eval_execute_queue.clone(),
        retry_queue: config.rabbitmq_eval_retry_queue.clone(),
        dead_queue: config.rabbitmq_eval_dead_queue.clone(),
        execute_routing_key: config.rabbitmq_eval_execute_routing_key.clone(),
        retry_routing_key: config.rabbitmq_eval_retry_routing_key.clone(),
        dead_routing_key: config.rabbitmq_eval_dead_routing_key.clone(),
        retry_ttl_ms: config.rabbitmq_eval_retry_ttl_ms,
    }
}

#[async_trait::async_trait]
pub trait EvalTaskPublisher: Send + Sync {
    async fn publish_eval_execute(&self, message: &EvalTaskMessage) -> Result<(), AppError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalOutboxPublishOutcome {
    pub outbox_id: i64,
    pub task_id: i64,
    pub published: bool,
    pub error: Option<String>,
}

pub fn eval_message_from_outbox(record: &EvalOutboxRecord) -> EvalTaskMessage {
    EvalTaskMessage {
        outbox_id: record.id,
        tenant_id: record.tenant_id,
        run_id: record.run_id,
        task_id: record.task_id,
        case_id: record
            .payload
            .get("caseId")
            .and_then(|value| value.as_i64())
            .unwrap_or_default(),
        run_mode: record
            .payload
            .get("runMode")
            .and_then(|value| value.as_str())
            .unwrap_or("deterministic")
            .to_owned(),
        attempt: record
            .payload
            .get("attempt")
            .and_then(|value| value.as_i64())
            .unwrap_or(0) as i32,
        max_attempts: record
            .payload
            .get("maxAttempts")
            .and_then(|value| value.as_i64())
            .unwrap_or(3) as i32,
    }
}

pub async fn publish_eval_outbox_records<P>(
    records: Vec<EvalOutboxRecord>,
    publisher: &P,
) -> Vec<EvalOutboxPublishOutcome>
where
    P: EvalTaskPublisher,
{
    let mut outcomes = Vec::with_capacity(records.len());
    for record in records {
        let message = eval_message_from_outbox(&record);
        match publisher.publish_eval_execute(&message).await {
            Ok(()) => outcomes.push(EvalOutboxPublishOutcome {
                outbox_id: record.id,
                task_id: record.task_id,
                published: true,
                error: None,
            }),
            Err(error) => outcomes.push(EvalOutboxPublishOutcome {
                outbox_id: record.id,
                task_id: record.task_id,
                published: false,
                error: Some(error.to_string()),
            }),
        }
    }
    outcomes
}

#[async_trait::async_trait]
impl EvalTaskPublisher for EvalRabbitMqClient {
    async fn publish_eval_execute(&self, message: &EvalTaskMessage) -> Result<(), AppError> {
        EvalRabbitMqClient::publish_eval_execute(self, message).await
    }
}

pub async fn publish_pending_eval_tasks<P>(
    repo: &AiEvalRepository,
    publisher: &P,
    batch_size: i64,
    user_id: i64,
) -> Result<usize, AppError>
where
    P: EvalTaskPublisher,
{
    let records = repo.list_pending_eval_outbox(batch_size).await?;
    let outcomes = publish_eval_outbox_records(records, publisher).await;
    let now = Utc::now().naive_utc();
    let mut published = 0usize;
    for outcome in outcomes {
        if outcome.published {
            repo.mark_eval_outbox_published(outcome.outbox_id, user_id, now)
                .await?;
            published += 1;
        } else {
            repo.mark_eval_outbox_publish_failed(
                outcome.outbox_id,
                outcome
                    .error
                    .as_deref()
                    .unwrap_or("eval queue publish failed"),
                user_id,
                now,
            )
            .await?;
        }
    }
    Ok(published)
}

pub fn spawn_eval_queue_publisher(
    db: PgPool,
    runtime: EvalQueueRuntimeConfig,
    rabbitmq: EvalRabbitMqConfig,
) {
    if !runtime.enabled || !runtime.publisher_enabled {
        return;
    }

    tokio::spawn(async move {
        if let Err(error) = run_eval_queue_publisher(db, runtime, rabbitmq).await {
            tracing::error!(error = ?error, "eval queue publisher stopped");
        }
    });
}

pub async fn run_eval_queue_publisher(
    db: PgPool,
    runtime: EvalQueueRuntimeConfig,
    rabbitmq: EvalRabbitMqConfig,
) -> Result<(), AppError> {
    let publisher = EvalRabbitMqClient::connect(rabbitmq).await?;
    let repo = AiEvalRepository::new(db);
    let mut interval = tokio::time::interval(Duration::from_secs(runtime.tick_seconds.max(1)));
    loop {
        interval.tick().await;
        match publish_pending_eval_tasks(&repo, &publisher, runtime.batch_size, 0).await {
            Ok(count) if count > 0 => tracing::debug!(count, "published eval outbox tasks"),
            Ok(_) => {}
            Err(error) => tracing::error!(error = ?error, "publish eval outbox tasks failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn eval_queue_runtime_config_uses_dedicated_eval_topology() {
        let app = test_app_config();

        let config = eval_queue_from_config(&app);
        let rabbitmq = eval_rabbitmq_from_config(&app);

        assert!(!config.enabled);
        assert!(!config.publisher_enabled);
        assert_eq!(config.tick_seconds, 5);
        assert_eq!(config.batch_size, 50);
        assert_eq!(rabbitmq.exchange, "novex.eval");
        assert_eq!(rabbitmq.execute_queue, "novex.eval.execute");
        assert_eq!(rabbitmq.retry_queue, "novex.eval.retry");
        assert_eq!(rabbitmq.dead_queue, "novex.eval.dead");
        assert_eq!(rabbitmq.execute_routing_key, "eval.execute");
        assert_eq!(rabbitmq.retry_routing_key, "eval.retry");
        assert_eq!(rabbitmq.dead_routing_key, "eval.dead");
    }

    #[test]
    fn eval_message_uses_payload_identity_from_outbox() {
        let record = eval_outbox_record();

        let message = eval_message_from_outbox(&record);

        assert_eq!(message.outbox_id, 10);
        assert_eq!(message.tenant_id, 1);
        assert_eq!(message.run_id, 20);
        assert_eq!(message.task_id, 30);
        assert_eq!(message.case_id, 40);
        assert_eq!(message.run_mode, "live_rag");
        assert_eq!(message.attempt, 0);
        assert_eq!(message.max_attempts, 3);
    }

    #[tokio::test]
    async fn publish_outbox_records_reports_success_and_failures() {
        let success = eval_outbox_record();
        let mut failure = eval_outbox_record();
        failure.id = 11;
        failure.task_id = 31;
        let publisher = FakeEvalPublisher::failing_on(31);

        let outcomes = publish_eval_outbox_records(vec![success, failure], &publisher).await;

        assert_eq!(publisher.published_task_ids(), vec![30, 31]);
        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0].outbox_id, 10);
        assert!(outcomes[0].published);
        assert_eq!(outcomes[1].outbox_id, 11);
        assert!(!outcomes[1].published);
        assert!(outcomes[1]
            .error
            .as_deref()
            .unwrap()
            .contains("fake publish failure"));
    }

    fn eval_outbox_record() -> EvalOutboxRecord {
        EvalOutboxRecord {
            id: 10,
            tenant_id: 1,
            run_id: 20,
            task_id: 30,
            event_type: "eval.task.requested".to_owned(),
            payload: serde_json::json!({
                "taskId": 30,
                "runId": 20,
                "tenantId": 1,
                "caseId": 40,
                "runMode": "live_rag",
                "attempt": 0,
                "maxAttempts": 3
            }),
            status: 1,
            attempt_count: 0,
        }
    }

    fn test_app_config() -> AppConfig {
        AppConfig {
            http_port: 62601,
            database_url: "postgres://postgres:postgres@localhost:5432/avalon_admin".to_owned(),
            database_max_connections: 5,
            db_auto_migrate: false,
            cors_allowed_origins: vec!["http://localhost:62602".to_owned()],
            auth_jwt_secret: "local-dev-only-change-this-secret-32chars-min".to_owned(),
            auth_jwt_ttl_hours: 24,
            scheduler_embedded: false,
            scheduler_worker_enabled: true,
            scheduler_tick_seconds: 5,
            scheduler_batch_size: 50,
            scheduler_worker_id: "worker-1".to_owned(),
            scheduler_http_allowlist_mode: "default".to_owned(),
            scheduler_http_allowlist: Vec::new(),
            rabbitmq_url: "amqp://guest:guest@127.0.0.1:5672/%2f".to_owned(),
            rabbitmq_exchange: "avalon.scheduler".to_owned(),
            rabbitmq_execute_queue: "avalon.scheduler.execute".to_owned(),
            rabbitmq_retry_queue: "avalon.scheduler.retry".to_owned(),
            rabbitmq_dead_queue: "avalon.scheduler.dead".to_owned(),
            rabbitmq_execute_routing_key: "scheduler.execute".to_owned(),
            rabbitmq_retry_routing_key: "scheduler.retry".to_owned(),
            rabbitmq_dead_routing_key: "scheduler.dead".to_owned(),
            rabbitmq_retry_ttl_ms: 30_000,
            parser_queue_enabled: false,
            parser_queue_publisher_enabled: false,
            parser_queue_tick_seconds: 5,
            parser_queue_batch_size: 50,
            agent_queue_enabled: false,
            agent_queue_publisher_enabled: false,
            agent_queue_tick_seconds: 2,
            agent_queue_batch_size: 10,
            agent_queue_lease_seconds: 120,
            agent_queue_max_attempts: 3,
            agent_queue_worker_id: "agent-worker-1".to_owned(),
            eval_queue_enabled: false,
            eval_queue_publisher_enabled: false,
            eval_queue_tick_seconds: 5,
            eval_queue_batch_size: 50,
            eval_worker_enabled: false,
            eval_worker_id: "eval-worker-1".to_owned(),
            eval_task_timeout_seconds: 180,
            redis_url: "redis://127.0.0.1:6379/0".to_owned(),
            rabbitmq_parser_exchange: "novex.parser".to_owned(),
            rabbitmq_parser_execute_queue: "novex.parser.execute".to_owned(),
            rabbitmq_parser_retry_queue: "novex.parser.retry".to_owned(),
            rabbitmq_parser_dead_queue: "novex.parser.dead".to_owned(),
            rabbitmq_parser_execute_routing_key: "parser.execute".to_owned(),
            rabbitmq_parser_retry_routing_key: "parser.retry".to_owned(),
            rabbitmq_parser_dead_routing_key: "parser.dead".to_owned(),
            rabbitmq_parser_retry_ttl_ms: 30_000,
            rabbitmq_agent_exchange: "novex.agent".to_owned(),
            rabbitmq_agent_execute_queue: "novex.agent.execute".to_owned(),
            rabbitmq_agent_retry_queue: "novex.agent.retry".to_owned(),
            rabbitmq_agent_dead_queue: "novex.agent.dead".to_owned(),
            rabbitmq_agent_execute_routing_key: "agent.execute".to_owned(),
            rabbitmq_agent_retry_routing_key: "agent.retry".to_owned(),
            rabbitmq_agent_dead_routing_key: "agent.dead".to_owned(),
            rabbitmq_agent_retry_ttl_ms: 30_000,
            rabbitmq_eval_exchange: "novex.eval".to_owned(),
            rabbitmq_eval_execute_queue: "novex.eval.execute".to_owned(),
            rabbitmq_eval_retry_queue: "novex.eval.retry".to_owned(),
            rabbitmq_eval_dead_queue: "novex.eval.dead".to_owned(),
            rabbitmq_eval_execute_routing_key: "eval.execute".to_owned(),
            rabbitmq_eval_retry_routing_key: "eval.retry".to_owned(),
            rabbitmq_eval_dead_routing_key: "eval.dead".to_owned(),
            rabbitmq_eval_retry_ttl_ms: 30_000,
        }
    }

    #[derive(Debug, Clone)]
    struct FakeEvalPublisher {
        failing_task_id: i64,
        published: Arc<Mutex<Vec<i64>>>,
    }

    impl FakeEvalPublisher {
        fn failing_on(task_id: i64) -> Self {
            Self {
                failing_task_id: task_id,
                published: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn published_task_ids(&self) -> Vec<i64> {
            self.published.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl EvalTaskPublisher for FakeEvalPublisher {
        async fn publish_eval_execute(&self, message: &EvalTaskMessage) -> Result<(), AppError> {
            self.published.lock().unwrap().push(message.task_id);
            if message.task_id == self.failing_task_id {
                return Err(AppError::Anyhow(anyhow::anyhow!("fake publish failure")));
            }
            Ok(())
        }
    }
}
