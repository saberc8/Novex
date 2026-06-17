use std::time::Duration;

use chrono::Utc;
use futures_lite::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions};
use sqlx::PgPool;

use crate::{
    application::ai::agent_service::{AgentRuntimeRegistry, AgentService},
    infrastructure::{
        mq::rabbitmq::{AgentQueueMessage, AgentRabbitMqClient, AgentRabbitMqConfig},
        persistence::ai_agent_repository::{
            AgentRunQueueClaimRecord, AgentRunQueueSaveRecord, AiAgentRepository,
        },
    },
    shared::{config::AppConfig, error::AppError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentQueueRuntimeConfig {
    pub enabled: bool,
    pub tick_seconds: u64,
    pub batch_size: i64,
    pub lease_seconds: u64,
    pub max_attempts: i32,
    pub worker_id: String,
}

pub fn agent_queue_from_config(config: &AppConfig) -> AgentQueueRuntimeConfig {
    AgentQueueRuntimeConfig {
        enabled: config.agent_queue_enabled,
        tick_seconds: config.agent_queue_tick_seconds,
        batch_size: config.agent_queue_batch_size,
        lease_seconds: config.agent_queue_lease_seconds,
        max_attempts: config.agent_queue_max_attempts,
        worker_id: config.agent_queue_worker_id.clone(),
    }
}

pub fn agent_rabbitmq_from_config(config: &AppConfig) -> AgentRabbitMqConfig {
    AgentRabbitMqConfig {
        url: config.rabbitmq_url.clone(),
        exchange: config.rabbitmq_agent_exchange.clone(),
        execute_queue: config.rabbitmq_agent_execute_queue.clone(),
        retry_queue: config.rabbitmq_agent_retry_queue.clone(),
        dead_queue: config.rabbitmq_agent_dead_queue.clone(),
        execute_routing_key: config.rabbitmq_agent_execute_routing_key.clone(),
        retry_routing_key: config.rabbitmq_agent_retry_routing_key.clone(),
        dead_routing_key: config.rabbitmq_agent_dead_routing_key.clone(),
        retry_ttl_ms: config.rabbitmq_agent_retry_ttl_ms,
    }
}

#[async_trait::async_trait]
pub trait AgentQueueMessagePublisher: Send + Sync {
    async fn publish_agent_execute(&self, message: &AgentQueueMessage) -> Result<(), AppError>;
}

#[async_trait::async_trait]
impl AgentQueueMessagePublisher for AgentRabbitMqClient {
    async fn publish_agent_execute(&self, message: &AgentQueueMessage) -> Result<(), AppError> {
        AgentRabbitMqClient::publish_agent_execute(self, message).await
    }
}

pub fn agent_queue_message_from_save_record(record: &AgentRunQueueSaveRecord) -> AgentQueueMessage {
    AgentQueueMessage {
        queue_id: record.id,
        tenant_id: record.tenant_id,
        run_id: record.run_id,
        event: "agent.run.queued".to_owned(),
        attempt: 0,
        max_attempts: record.max_attempts,
        source: record
            .payload
            .get("source")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("agent.queue")
            .to_owned(),
    }
}

fn agent_queue_message_from_claim_record(
    claim: &AgentRunQueueClaimRecord,
    event: &str,
) -> AgentQueueMessage {
    AgentQueueMessage {
        queue_id: claim.id,
        tenant_id: claim.tenant_id,
        run_id: claim.run_id,
        event: event.to_owned(),
        attempt: claim.attempt_count,
        max_attempts: claim.max_attempts,
        source: claim
            .payload
            .get("source")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("agent.queue")
            .to_owned(),
    }
}

#[derive(Debug, Clone)]
enum AgentQueueClaimOutcome {
    Completed,
    Retry(AgentQueueMessage),
    Dead(AgentQueueMessage),
}

pub fn spawn_agent_queue_worker(
    db: PgPool,
    runtime: AgentQueueRuntimeConfig,
    agent_runtime: AgentRuntimeRegistry,
) {
    if !runtime.enabled {
        return;
    }

    tokio::spawn(async move {
        if let Err(error) = run_agent_queue_worker(db, runtime, agent_runtime).await {
            tracing::error!(error = ?error, "agent queue worker stopped");
        }
    });
}

pub fn spawn_agent_queue_broker_consumer(
    db: PgPool,
    runtime: AgentQueueRuntimeConfig,
    rabbitmq: AgentRabbitMqConfig,
    agent_runtime: AgentRuntimeRegistry,
) {
    if !runtime.enabled {
        return;
    }

    tokio::spawn(async move {
        if let Err(error) =
            run_agent_queue_broker_consumer(db, runtime, rabbitmq, agent_runtime).await
        {
            tracing::error!(error = ?error, "agent queue broker consumer stopped");
        }
    });
}

pub async fn run_agent_queue_broker_consumer(
    db: PgPool,
    runtime: AgentQueueRuntimeConfig,
    rabbitmq: AgentRabbitMqConfig,
    agent_runtime: AgentRuntimeRegistry,
) -> Result<(), AppError> {
    if !runtime.enabled {
        return Ok(());
    }

    let mq = AgentRabbitMqClient::connect(rabbitmq).await?;
    consume_agent_execute_queue(db, runtime, mq, agent_runtime).await
}

async fn consume_agent_execute_queue(
    db: PgPool,
    runtime: AgentQueueRuntimeConfig,
    mq: AgentRabbitMqClient,
    agent_runtime: AgentRuntimeRegistry,
) -> Result<(), AppError> {
    let mut consumer = mq
        .channel()
        .basic_consume(
            &mq.config().execute_queue,
            &runtime.worker_id,
            BasicConsumeOptions::default(),
            lapin::types::FieldTable::default(),
        )
        .await
        .map_err(|error| AppError::Anyhow(anyhow::anyhow!("consume agent queue: {error}")))?;

    while let Some(delivery) = consumer.next().await {
        let delivery = delivery
            .map_err(|error| AppError::Anyhow(anyhow::anyhow!("receive agent message: {error}")))?;
        let message = match serde_json::from_slice::<AgentQueueMessage>(&delivery.data) {
            Ok(message) => message,
            Err(error) => {
                tracing::error!(error = ?error, "decode agent message failed");
                delivery
                    .ack(BasicAckOptions::default())
                    .await
                    .map_err(|error| {
                        AppError::Anyhow(anyhow::anyhow!("ack invalid agent message: {error}"))
                    })?;
                continue;
            }
        };

        let repo = AiAgentRepository::new(db.clone());
        let now = Utc::now().naive_utc();
        let lease_until = now + chrono::Duration::seconds(runtime.lease_seconds.max(1) as i64);
        let claim = repo
            .claim_agent_run_queue_by_message(
                message.queue_id,
                message.tenant_id,
                message.run_id,
                &runtime.worker_id,
                lease_until,
                0,
                now,
            )
            .await?;

        if let Some(claim) = claim {
            match execute_agent_queue_claim(db.clone(), &repo, agent_runtime.clone(), claim).await?
            {
                AgentQueueClaimOutcome::Completed => {}
                AgentQueueClaimOutcome::Retry(retry) => {
                    if let Err(error) = mq.publish_agent_retry(&retry).await {
                        tracing::error!(error = ?error, run_id = retry.run_id, "publish agent retry message failed");
                    }
                }
                AgentQueueClaimOutcome::Dead(dead) => {
                    if let Err(error) = mq.publish_agent_dead(&dead).await {
                        tracing::error!(error = ?error, run_id = dead.run_id, "publish agent dead message failed");
                    }
                }
            }
        } else {
            tracing::debug!(
                queue_id = message.queue_id,
                run_id = message.run_id,
                "agent queue broker message is stale or already claimed"
            );
        }

        delivery
            .ack(BasicAckOptions::default())
            .await
            .map_err(|error| AppError::Anyhow(anyhow::anyhow!("ack agent message: {error}")))?;
    }

    Ok(())
}

pub async fn run_agent_queue_worker(
    db: PgPool,
    runtime: AgentQueueRuntimeConfig,
    agent_runtime: AgentRuntimeRegistry,
) -> Result<(), AppError> {
    let mut interval = tokio::time::interval(Duration::from_secs(runtime.tick_seconds.max(1)));
    loop {
        interval.tick().await;
        match run_agent_queue_tick(db.clone(), runtime.clone(), agent_runtime.clone()).await {
            Ok(count) if count > 0 => {
                tracing::debug!(count, "executed queued agent runs");
            }
            Ok(_) => {}
            Err(error) => tracing::error!(error = ?error, "execute queued agent runs failed"),
        }
    }
}

pub async fn run_agent_queue_tick(
    db: PgPool,
    runtime: AgentQueueRuntimeConfig,
    agent_runtime: AgentRuntimeRegistry,
) -> Result<usize, AppError> {
    if !runtime.enabled {
        return Ok(0);
    }

    let repo = AiAgentRepository::new(db.clone());
    let now = Utc::now().naive_utc();
    let lease_until = now + chrono::Duration::seconds(runtime.lease_seconds.max(1) as i64);
    let claims = repo
        .claim_agent_run_queue(
            None,
            runtime.batch_size,
            &runtime.worker_id,
            lease_until,
            0,
            now,
        )
        .await?;
    let mut executed = 0usize;
    for claim in claims {
        match execute_agent_queue_claim(db.clone(), &repo, agent_runtime.clone(), claim).await? {
            AgentQueueClaimOutcome::Completed | AgentQueueClaimOutcome::Dead(_) => executed += 1,
            AgentQueueClaimOutcome::Retry(_) => {}
        }
    }
    Ok(executed)
}

async fn execute_agent_queue_claim(
    db: PgPool,
    repo: &AiAgentRepository,
    agent_runtime: AgentRuntimeRegistry,
    claim: AgentRunQueueClaimRecord,
) -> Result<AgentQueueClaimOutcome, AppError> {
    let service = AgentService::for_tenant_with_runtime(db, claim.tenant_id, agent_runtime.clone());
    match service
        .execute_queued_run(0, claim.run_id, claim.payload.clone())
        .await
    {
        Ok(run) if run.status == "cancelled" => {
            repo.mark_agent_run_queue_cancelled(claim.id, 0, Utc::now().naive_utc())
                .await?;
            Ok(AgentQueueClaimOutcome::Completed)
        }
        Ok(run) if run.status == "waiting_approval" => {
            repo.mark_agent_run_queue_waiting_approval(claim.id, 0, Utc::now().naive_utc())
                .await?;
            Ok(AgentQueueClaimOutcome::Completed)
        }
        Ok(_) => {
            repo.mark_agent_run_queue_succeeded(claim.id, 0, Utc::now().naive_utc())
                .await?;
            Ok(AgentQueueClaimOutcome::Completed)
        }
        Err(error) if claim.attempt_count < claim.max_attempts => {
            repo.mark_agent_run_queue_retrying(
                claim.id,
                &error.to_string(),
                0,
                Utc::now().naive_utc(),
            )
            .await?;
            Ok(AgentQueueClaimOutcome::Retry(
                agent_queue_message_from_claim_record(&claim, "agent.run.retry"),
            ))
        }
        Err(error) => {
            repo.mark_agent_run_queue_failed(
                claim.id,
                &error.to_string(),
                0,
                Utc::now().naive_utc(),
            )
            .await?;
            Ok(AgentQueueClaimOutcome::Dead(
                agent_queue_message_from_claim_record(&claim, "agent.run.dead"),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_queue_runtime_defines_worker_claim_and_completion_contract() {
        let source = include_str!("agent_queue_runtime.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("AgentQueueRuntimeConfig"));
        assert!(source.contains("agent_queue_from_config"));
        assert!(source.contains("spawn_agent_queue_worker"));
        assert!(source.contains("run_agent_queue_worker"));
        assert!(source.contains("run_agent_queue_tick"));
        assert!(source.contains("claim_agent_run_queue"));
        assert!(source.contains("execute_queued_run"));
        assert!(source.contains("mark_agent_run_queue_succeeded"));
        assert!(source.contains("mark_agent_run_queue_retrying"));
        assert!(source.contains("mark_agent_run_queue_failed"));
        assert!(source.contains("mark_agent_run_queue_cancelled"));
    }

    #[test]
    fn agent_queue_resume_requeue_worker_marks_waiting_approval_without_success() {
        let source = include_str!("agent_queue_runtime.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("run.status == \"waiting_approval\""));
        assert!(source.contains("mark_agent_run_queue_waiting_approval"));
        assert!(
            source.find("run.status == \"waiting_approval\"").unwrap()
                < source.find("mark_agent_run_queue_succeeded").unwrap()
        );
    }

    #[test]
    fn agent_queue_runtime_config_uses_agent_specific_defaults() {
        let config = AppConfig {
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
            scheduler_worker_id: "scheduler-test".to_owned(),
            scheduler_http_allowlist_mode: "default".to_owned(),
            scheduler_http_allowlist: Vec::new(),
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
            agent_queue_enabled: true,
            agent_queue_tick_seconds: 2,
            agent_queue_batch_size: 10,
            agent_queue_lease_seconds: 120,
            agent_queue_max_attempts: 3,
            agent_queue_worker_id: "agent-test".to_owned(),
            redis_url: "redis://127.0.0.1:16379/0".to_owned(),
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
        };

        let runtime = agent_queue_from_config(&config);
        let rabbitmq = agent_rabbitmq_from_config(&config);

        assert!(runtime.enabled);
        assert_eq!(runtime.tick_seconds, 2);
        assert_eq!(runtime.batch_size, 10);
        assert_eq!(runtime.lease_seconds, 120);
        assert_eq!(runtime.max_attempts, 3);
        assert_eq!(runtime.worker_id, "agent-test");
        assert_eq!(rabbitmq.exchange, "novex.agent");
        assert_eq!(rabbitmq.execute_queue, "novex.agent.execute");
        assert_eq!(rabbitmq.retry_queue, "novex.agent.retry");
        assert_eq!(rabbitmq.dead_queue, "novex.agent.dead");
        assert_eq!(rabbitmq.execute_routing_key, "agent.execute");
    }

    #[test]
    fn agent_queue_broker_wakeup_message_uses_queue_save_record_metadata() {
        let record = AgentRunQueueSaveRecord {
            id: 10,
            tenant_id: 1,
            run_id: 99,
            priority: 0,
            max_attempts: 3,
            payload: serde_json::json!({ "source": "agent.create_run" }),
            user_id: 7,
            now: chrono::NaiveDate::from_ymd_opt(2026, 6, 17)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
        };

        let message = agent_queue_message_from_save_record(&record);

        assert_eq!(message.queue_id, 10);
        assert_eq!(message.tenant_id, 1);
        assert_eq!(message.run_id, 99);
        assert_eq!(message.event, "agent.run.queued");
        assert_eq!(message.attempt, 0);
        assert_eq!(message.max_attempts, 3);
        assert_eq!(message.source, "agent.create_run");
    }

    #[tokio::test]
    async fn agent_queue_broker_wakeup_publisher_trait_supports_fake_publishers() {
        let publisher = FakeAgentQueuePublisher::default();
        let message = AgentQueueMessage {
            queue_id: 10,
            tenant_id: 1,
            run_id: 99,
            event: "agent.run.queued".to_owned(),
            attempt: 0,
            max_attempts: 3,
            source: "agent.create_run".to_owned(),
        };

        publisher.publish_agent_execute(&message).await.unwrap();

        assert_eq!(publisher.published_run_ids(), vec![99]);
    }

    #[test]
    fn agent_queue_broker_consumer_runtime_declares_execute_path() {
        let source = include_str!("agent_queue_runtime.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "spawn_agent_queue_broker_consumer",
            "run_agent_queue_broker_consumer",
            "consume_agent_execute_queue",
            "AgentRabbitMqClient::connect",
            "basic_consume",
            "AgentQueueMessage",
            "claim_agent_run_queue_by_message",
            "publish_agent_retry",
            "publish_agent_dead",
            "BasicAckOptions",
        ] {
            assert!(
                source.contains(needle),
                "{needle} missing from Agent queue broker consumer"
            );
        }
    }

    #[derive(Debug, Default)]
    struct FakeAgentQueuePublisher {
        published: std::sync::Mutex<Vec<i64>>,
    }

    impl FakeAgentQueuePublisher {
        fn published_run_ids(&self) -> Vec<i64> {
            self.published.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl AgentQueueMessagePublisher for FakeAgentQueuePublisher {
        async fn publish_agent_execute(&self, message: &AgentQueueMessage) -> Result<(), AppError> {
            self.published.lock().unwrap().push(message.run_id);
            Ok(())
        }
    }
}
