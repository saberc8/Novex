use std::time::Duration;

use chrono::Utc;
use sqlx::PgPool;

use crate::{
    infrastructure::{
        mq::rabbitmq::{ParserJobMessage, ParserRabbitMqClient, ParserRabbitMqConfig},
        persistence::ai_knowledge_repository::{AiKnowledgeRepository, ParserOutboxRecord},
    },
    shared::config::AppConfig,
    shared::error::AppError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserQueueRuntimeConfig {
    pub enabled: bool,
    pub publisher_enabled: bool,
    pub tick_seconds: u64,
    pub batch_size: i64,
    pub redis_url: String,
}

pub fn parser_queue_from_config(config: &AppConfig) -> ParserQueueRuntimeConfig {
    ParserQueueRuntimeConfig {
        enabled: config.parser_queue_enabled,
        publisher_enabled: config.parser_queue_publisher_enabled,
        tick_seconds: config.parser_queue_tick_seconds,
        batch_size: config.parser_queue_batch_size,
        redis_url: config.redis_url.clone(),
    }
}

pub fn parser_rabbitmq_from_config(config: &AppConfig) -> ParserRabbitMqConfig {
    ParserRabbitMqConfig {
        url: config.rabbitmq_url.clone(),
        exchange: config.rabbitmq_parser_exchange.clone(),
        execute_queue: config.rabbitmq_parser_execute_queue.clone(),
        retry_queue: config.rabbitmq_parser_retry_queue.clone(),
        dead_queue: config.rabbitmq_parser_dead_queue.clone(),
        execute_routing_key: config.rabbitmq_parser_execute_routing_key.clone(),
        retry_routing_key: config.rabbitmq_parser_retry_routing_key.clone(),
        dead_routing_key: config.rabbitmq_parser_dead_routing_key.clone(),
        retry_ttl_ms: config.rabbitmq_parser_retry_ttl_ms,
    }
}

#[async_trait::async_trait]
pub trait ParserMessagePublisher: Send + Sync {
    async fn publish_parser_execute(&self, message: &ParserJobMessage) -> Result<(), AppError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserOutboxPublishOutcome {
    pub outbox_id: i64,
    pub parser_job_id: i64,
    pub published: bool,
    pub error: Option<String>,
}

pub fn parser_message_from_outbox(record: &ParserOutboxRecord) -> ParserJobMessage {
    let attempt = record
        .payload
        .get("attempt")
        .and_then(|value| value.as_i64())
        .unwrap_or(1)
        .max(1) as i32;

    ParserJobMessage {
        outbox_id: record.id,
        tenant_id: record.tenant_id,
        dataset_id: record.dataset_id,
        document_id: record.document_id,
        parser_job_id: record.parser_job_id,
        attempt,
        max_attempts: record
            .payload
            .get("maxAttempts")
            .and_then(|value| value.as_i64())
            .unwrap_or(5) as i32,
        parser_request: record
            .payload
            .get("parserRequest")
            .cloned()
            .unwrap_or_else(|| record.payload.clone()),
    }
}

pub async fn publish_parser_outbox_records<P>(
    records: Vec<ParserOutboxRecord>,
    publisher: &P,
) -> Vec<ParserOutboxPublishOutcome>
where
    P: ParserMessagePublisher,
{
    let mut outcomes = Vec::with_capacity(records.len());
    for record in records {
        let message = parser_message_from_outbox(&record);
        match publisher.publish_parser_execute(&message).await {
            Ok(()) => outcomes.push(ParserOutboxPublishOutcome {
                outbox_id: record.id,
                parser_job_id: record.parser_job_id,
                published: true,
                error: None,
            }),
            Err(error) => outcomes.push(ParserOutboxPublishOutcome {
                outbox_id: record.id,
                parser_job_id: record.parser_job_id,
                published: false,
                error: Some(error.to_string()),
            }),
        }
    }
    outcomes
}

#[async_trait::async_trait]
impl ParserMessagePublisher for ParserRabbitMqClient {
    async fn publish_parser_execute(&self, message: &ParserJobMessage) -> Result<(), AppError> {
        ParserRabbitMqClient::publish_parser_execute(self, message).await
    }
}

pub async fn publish_pending_parser_jobs<P>(
    repo: &AiKnowledgeRepository,
    publisher: &P,
    batch_size: i64,
    user_id: i64,
) -> Result<usize, AppError>
where
    P: ParserMessagePublisher,
{
    let records = repo.list_pending_parser_outbox(batch_size).await?;
    let outcomes = publish_parser_outbox_records(records, publisher).await;
    let now = Utc::now().naive_utc();
    let mut published = 0usize;
    for outcome in outcomes {
        if outcome.published {
            repo.mark_parser_outbox_published(outcome.outbox_id, user_id, now)
                .await?;
            published += 1;
        } else {
            repo.mark_parser_outbox_publish_failed(
                outcome.outbox_id,
                outcome
                    .error
                    .as_deref()
                    .unwrap_or("parser queue publish failed"),
                user_id,
                now,
            )
            .await?;
        }
    }
    Ok(published)
}

pub fn spawn_parser_queue_publisher(
    db: PgPool,
    runtime: ParserQueueRuntimeConfig,
    rabbitmq: ParserRabbitMqConfig,
) {
    if !runtime.enabled || !runtime.publisher_enabled {
        return;
    }

    tokio::spawn(async move {
        if let Err(error) = run_parser_queue_publisher(db, runtime, rabbitmq).await {
            tracing::error!(error = ?error, "parser queue publisher stopped");
        }
    });
}

pub async fn run_parser_queue_publisher(
    db: PgPool,
    runtime: ParserQueueRuntimeConfig,
    rabbitmq: ParserRabbitMqConfig,
) -> Result<(), AppError> {
    let publisher = ParserRabbitMqClient::connect(rabbitmq).await?;
    let repo = AiKnowledgeRepository::new(db);
    let mut interval = tokio::time::interval(Duration::from_secs(runtime.tick_seconds.max(1)));
    loop {
        interval.tick().await;
        match publish_pending_parser_jobs(&repo, &publisher, runtime.batch_size, 0).await {
            Ok(count) if count > 0 => {
                tracing::debug!(count, "published parser outbox jobs");
            }
            Ok(_) => {}
            Err(error) => tracing::error!(error = ?error, "publish parser outbox jobs failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn parser_queue_runtime_config_uses_dedicated_parser_topology() {
        let app = test_app_config();

        let config = parser_queue_from_config(&app);
        let rabbitmq = parser_rabbitmq_from_config(&app);

        assert!(!config.enabled);
        assert!(!config.publisher_enabled);
        assert_eq!(config.tick_seconds, 5);
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.redis_url, "redis://127.0.0.1:16379/0");
        assert_eq!(rabbitmq.exchange, "novex.parser");
        assert_eq!(rabbitmq.execute_queue, "novex.parser.execute");
        assert_eq!(rabbitmq.retry_queue, "novex.parser.retry");
        assert_eq!(rabbitmq.dead_queue, "novex.parser.dead");
        assert_eq!(rabbitmq.execute_routing_key, "parser.execute");
        assert_eq!(rabbitmq.retry_routing_key, "parser.retry");
        assert_eq!(rabbitmq.dead_routing_key, "parser.dead");
    }

    #[test]
    fn parser_message_starts_worker_attempt_at_one() {
        let record = parser_outbox_record();

        let message = parser_message_from_outbox(&record);

        assert_eq!(message.outbox_id, 10);
        assert_eq!(message.tenant_id, 1);
        assert_eq!(message.dataset_id, 7);
        assert_eq!(message.document_id, 42);
        assert_eq!(message.parser_job_id, 99);
        assert_eq!(message.attempt, 1);
        assert_eq!(message.max_attempts, 5);
        assert_eq!(message.parser_request["source"]["name"], "handbook.md");
    }

    #[tokio::test]
    async fn publish_outbox_records_reports_success_and_failures() {
        let success = parser_outbox_record();
        let mut failure = parser_outbox_record();
        failure.id = 11;
        failure.parser_job_id = 100;
        let publisher = FakeParserPublisher::failing_on(100);

        let outcomes = publish_parser_outbox_records(vec![success, failure], &publisher).await;

        assert_eq!(publisher.published_parser_job_ids(), vec![99, 100]);
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

    #[test]
    fn runtime_publishes_pending_outbox_and_records_state() {
        let source = include_str!("parser_queue_runtime.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "publish_pending_parser_jobs",
            "list_pending_parser_outbox",
            "mark_parser_outbox_published",
            "mark_parser_outbox_publish_failed",
            "publish_parser_outbox_records",
        ] {
            assert!(
                source.contains(needle),
                "{needle} missing from parser queue runtime"
            );
        }
    }

    fn parser_outbox_record() -> ParserOutboxRecord {
        ParserOutboxRecord {
            id: 10,
            tenant_id: 1,
            dataset_id: 7,
            document_id: 42,
            parser_job_id: 99,
            event_type: "parser.job.requested".to_owned(),
            payload: serde_json::json!({
                "attempt": 0,
                "maxAttempts": 5,
                "parserRequest": {
                    "source": {
                        "name": "handbook.md"
                    }
                }
            }),
            status: 1,
            attempt_count: 0,
        }
    }

    #[derive(Debug, Clone)]
    struct FakeParserPublisher {
        failing_parser_job_id: i64,
        published: Arc<Mutex<Vec<i64>>>,
    }

    impl FakeParserPublisher {
        fn failing_on(parser_job_id: i64) -> Self {
            Self {
                failing_parser_job_id: parser_job_id,
                published: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn published_parser_job_ids(&self) -> Vec<i64> {
            self.published.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl ParserMessagePublisher for FakeParserPublisher {
        async fn publish_parser_execute(
            &self,
            message: &crate::infrastructure::mq::rabbitmq::ParserJobMessage,
        ) -> Result<(), crate::shared::error::AppError> {
            self.published.lock().unwrap().push(message.parser_job_id);
            if message.parser_job_id == self.failing_parser_job_id {
                return Err(crate::shared::error::AppError::Anyhow(anyhow::anyhow!(
                    "fake publish failure"
                )));
            }
            Ok(())
        }
    }

    fn test_app_config() -> AppConfig {
        AppConfig {
            http_port: 4398,
            database_url: "postgres://postgres:postgres@127.0.0.1:15432/novex".to_owned(),
            database_max_connections: 5,
            db_auto_migrate: false,
            cors_allowed_origins: vec!["http://localhost:4399".to_owned()],
            auth_jwt_secret: "local-dev-only-change-this-secret-32chars-min".to_owned(),
            auth_jwt_ttl_hours: 24,
            scheduler_embedded: false,
            scheduler_worker_enabled: true,
            scheduler_tick_seconds: 5,
            scheduler_batch_size: 50,
            scheduler_worker_id: "worker-1".to_owned(),
            scheduler_http_allowlist_mode: "default".to_owned(),
            scheduler_http_allowlist: vec![],
            rabbitmq_url: "amqp://guest:guest@127.0.0.1:5673/%2f".to_owned(),
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
            agent_queue_tick_seconds: 2,
            agent_queue_batch_size: 10,
            agent_queue_lease_seconds: 120,
            agent_queue_max_attempts: 3,
            agent_queue_worker_id: "agent-worker-1".to_owned(),
            redis_url: "redis://127.0.0.1:16379/0".to_owned(),
            rabbitmq_parser_exchange: "novex.parser".to_owned(),
            rabbitmq_parser_execute_queue: "novex.parser.execute".to_owned(),
            rabbitmq_parser_retry_queue: "novex.parser.retry".to_owned(),
            rabbitmq_parser_dead_queue: "novex.parser.dead".to_owned(),
            rabbitmq_parser_execute_routing_key: "parser.execute".to_owned(),
            rabbitmq_parser_retry_routing_key: "parser.retry".to_owned(),
            rabbitmq_parser_dead_routing_key: "parser.dead".to_owned(),
            rabbitmq_parser_retry_ttl_ms: 30_000,
        }
    }
}
