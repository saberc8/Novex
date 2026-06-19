use lapin::{
    options::*,
    publisher_confirm::Confirmation,
    types::{AMQPValue, FieldTable},
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::error::AppError;

#[derive(Debug, Clone)]
pub struct RabbitMqConfig {
    pub url: String,
    pub exchange: String,
    pub execute_queue: String,
    pub retry_queue: String,
    pub dead_queue: String,
    pub execute_routing_key: String,
    pub retry_routing_key: String,
    pub dead_routing_key: String,
    pub retry_ttl_ms: u32,
}

impl Default for RabbitMqConfig {
    fn default() -> Self {
        Self {
            url: "amqp://guest:guest@127.0.0.1:5673/%2f".to_owned(),
            exchange: "avalon.scheduler".to_owned(),
            execute_queue: "avalon.scheduler.execute".to_owned(),
            retry_queue: "avalon.scheduler.retry".to_owned(),
            dead_queue: "avalon.scheduler.dead".to_owned(),
            execute_routing_key: "scheduler.execute".to_owned(),
            retry_routing_key: "scheduler.retry".to_owned(),
            dead_routing_key: "scheduler.dead".to_owned(),
            retry_ttl_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParserRabbitMqConfig {
    pub url: String,
    pub exchange: String,
    pub execute_queue: String,
    pub retry_queue: String,
    pub dead_queue: String,
    pub execute_routing_key: String,
    pub retry_routing_key: String,
    pub dead_routing_key: String,
    pub retry_ttl_ms: u32,
}

impl Default for ParserRabbitMqConfig {
    fn default() -> Self {
        Self {
            url: "amqp://guest:guest@127.0.0.1:5673/%2f".to_owned(),
            exchange: "novex.parser".to_owned(),
            execute_queue: "novex.parser.execute".to_owned(),
            retry_queue: "novex.parser.retry".to_owned(),
            dead_queue: "novex.parser.dead".to_owned(),
            execute_routing_key: "parser.execute".to_owned(),
            retry_routing_key: "parser.retry".to_owned(),
            dead_routing_key: "parser.dead".to_owned(),
            retry_ttl_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentRabbitMqConfig {
    pub url: String,
    pub exchange: String,
    pub execute_queue: String,
    pub retry_queue: String,
    pub dead_queue: String,
    pub execute_routing_key: String,
    pub retry_routing_key: String,
    pub dead_routing_key: String,
    pub retry_ttl_ms: u32,
}

impl Default for AgentRabbitMqConfig {
    fn default() -> Self {
        Self {
            url: "amqp://guest:guest@127.0.0.1:5673/%2f".to_owned(),
            exchange: "novex.agent".to_owned(),
            execute_queue: "novex.agent.execute".to_owned(),
            retry_queue: "novex.agent.retry".to_owned(),
            dead_queue: "novex.agent.dead".to_owned(),
            execute_routing_key: "agent.execute".to_owned(),
            retry_routing_key: "agent.retry".to_owned(),
            dead_routing_key: "agent.dead".to_owned(),
            retry_ttl_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EvalRabbitMqConfig {
    pub url: String,
    pub exchange: String,
    pub execute_queue: String,
    pub retry_queue: String,
    pub dead_queue: String,
    pub execute_routing_key: String,
    pub retry_routing_key: String,
    pub dead_routing_key: String,
    pub retry_ttl_ms: u32,
}

impl Default for EvalRabbitMqConfig {
    fn default() -> Self {
        Self {
            url: "amqp://guest:guest@127.0.0.1:5673/%2f".to_owned(),
            exchange: "novex.eval".to_owned(),
            execute_queue: "novex.eval.execute".to_owned(),
            retry_queue: "novex.eval.retry".to_owned(),
            dead_queue: "novex.eval.dead".to_owned(),
            execute_routing_key: "eval.execute".to_owned(),
            retry_routing_key: "eval.retry".to_owned(),
            dead_routing_key: "eval.dead".to_owned(),
            retry_ttl_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerMessage {
    pub trigger_id: i64,
    pub job_id: i64,
    pub task_type: i16,
    pub attempt: i32,
    pub max_attempts: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserJobMessage {
    pub outbox_id: i64,
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub document_id: i64,
    pub parser_job_id: i64,
    pub attempt: i32,
    pub max_attempts: i32,
    pub parser_request: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalTaskMessage {
    pub outbox_id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub task_id: i64,
    pub case_id: i64,
    pub run_mode: String,
    pub attempt: i32,
    pub max_attempts: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentQueueMessage {
    pub queue_id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub event: String,
    pub attempt: i32,
    pub max_attempts: i32,
    pub source: String,
}

#[derive(Clone)]
pub struct RabbitMqClient {
    channel: Channel,
    config: RabbitMqConfig,
}

impl RabbitMqClient {
    pub async fn connect(config: RabbitMqConfig) -> Result<Self, AppError> {
        let connection = Connection::connect(&config.url, ConnectionProperties::default())
            .await
            .map_err(|error| AppError::Anyhow(anyhow::anyhow!("connect RabbitMQ: {error}")))?;
        let channel = connection.create_channel().await.map_err(|error| {
            AppError::Anyhow(anyhow::anyhow!("create RabbitMQ channel: {error}"))
        })?;
        channel
            .confirm_select(ConfirmSelectOptions::default())
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!(
                    "enable RabbitMQ publisher confirms: {error}"
                ))
            })?;
        let client = Self { channel, config };
        client.declare_topology().await?;
        Ok(client)
    }

    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    pub fn config(&self) -> &RabbitMqConfig {
        &self.config
    }

    pub async fn publish_execute(&self, message: &SchedulerMessage) -> Result<(), AppError> {
        self.publish(&self.config.execute_routing_key, message)
            .await
    }

    pub async fn publish_retry(&self, message: &SchedulerMessage) -> Result<(), AppError> {
        self.publish(&self.config.retry_routing_key, message).await
    }

    pub async fn publish_dead(&self, message: &SchedulerMessage) -> Result<(), AppError> {
        self.publish(&self.config.dead_routing_key, message).await
    }

    async fn publish(&self, routing_key: &str, message: &SchedulerMessage) -> Result<(), AppError> {
        let payload = serde_json::to_vec(message).map_err(|error| {
            AppError::Anyhow(anyhow::anyhow!("encode scheduler message: {error}"))
        })?;
        let confirmation = self
            .channel
            .basic_publish(
                &self.config.exchange,
                routing_key,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default().with_content_type("application/json".into()),
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("publish scheduler message: {error}"))
            })?
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("confirm scheduler message: {error}"))
            })?;
        if !matches!(confirmation, Confirmation::Ack(_)) {
            return Err(AppError::Anyhow(anyhow::anyhow!(
                "RabbitMQ did not ack scheduler message"
            )));
        }
        Ok(())
    }

    async fn declare_topology(&self) -> Result<(), AppError> {
        self.channel
            .exchange_declare(
                &self.config.exchange,
                ExchangeKind::Direct,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("declare scheduler exchange: {error}"))
            })?;

        self.declare_queue(&self.config.execute_queue, FieldTable::default())
            .await?;
        self.bind_queue(&self.config.execute_queue, &self.config.execute_routing_key)
            .await?;

        let mut retry_args = FieldTable::default();
        retry_args.insert(
            "x-message-ttl".into(),
            AMQPValue::LongUInt(self.config.retry_ttl_ms),
        );
        retry_args.insert(
            "x-dead-letter-exchange".into(),
            AMQPValue::LongString(self.config.exchange.clone().into()),
        );
        retry_args.insert(
            "x-dead-letter-routing-key".into(),
            AMQPValue::LongString(self.config.execute_routing_key.clone().into()),
        );
        self.declare_queue(&self.config.retry_queue, retry_args)
            .await?;
        self.bind_queue(&self.config.retry_queue, &self.config.retry_routing_key)
            .await?;

        self.declare_queue(&self.config.dead_queue, FieldTable::default())
            .await?;
        self.bind_queue(&self.config.dead_queue, &self.config.dead_routing_key)
            .await?;
        Ok(())
    }

    async fn declare_queue(&self, queue: &str, args: FieldTable) -> Result<(), AppError> {
        self.channel
            .queue_declare(
                queue,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                args,
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("declare scheduler queue {queue}: {error}"))
            })?;
        Ok(())
    }

    async fn bind_queue(&self, queue: &str, routing_key: &str) -> Result<(), AppError> {
        self.channel
            .queue_bind(
                queue,
                &self.config.exchange,
                routing_key,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("bind scheduler queue {queue}: {error}"))
            })?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct ParserRabbitMqClient {
    channel: Channel,
    config: ParserRabbitMqConfig,
}

impl ParserRabbitMqClient {
    pub async fn connect(config: ParserRabbitMqConfig) -> Result<Self, AppError> {
        let connection = Connection::connect(&config.url, ConnectionProperties::default())
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("connect parser RabbitMQ: {error}"))
            })?;
        let channel = connection.create_channel().await.map_err(|error| {
            AppError::Anyhow(anyhow::anyhow!("create parser RabbitMQ channel: {error}"))
        })?;
        channel
            .confirm_select(ConfirmSelectOptions::default())
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!(
                    "enable parser RabbitMQ publisher confirms: {error}"
                ))
            })?;
        let client = Self { channel, config };
        client.declare_parser_topology().await?;
        Ok(client)
    }

    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    pub fn config(&self) -> &ParserRabbitMqConfig {
        &self.config
    }

    pub async fn publish_parser_execute(&self, message: &ParserJobMessage) -> Result<(), AppError> {
        self.publish(&self.config.execute_routing_key, message)
            .await
    }

    pub async fn publish_parser_retry(&self, message: &ParserJobMessage) -> Result<(), AppError> {
        self.publish(&self.config.retry_routing_key, message).await
    }

    pub async fn publish_parser_dead(&self, message: &ParserJobMessage) -> Result<(), AppError> {
        self.publish(&self.config.dead_routing_key, message).await
    }

    async fn publish(&self, routing_key: &str, message: &ParserJobMessage) -> Result<(), AppError> {
        let payload = serde_json::to_vec(message).map_err(|error| {
            AppError::Anyhow(anyhow::anyhow!("encode parser execute message: {error}"))
        })?;
        let confirmation = self
            .channel
            .basic_publish(
                &self.config.exchange,
                routing_key,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default().with_content_type("application/json".into()),
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("publish parser execute message: {error}"))
            })?
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("confirm parser execute message: {error}"))
            })?;
        if !matches!(confirmation, Confirmation::Ack(_)) {
            return Err(AppError::Anyhow(anyhow::anyhow!(
                "RabbitMQ did not ack parser execute message"
            )));
        }
        Ok(())
    }

    async fn declare_parser_topology(&self) -> Result<(), AppError> {
        self.channel
            .exchange_declare(
                &self.config.exchange,
                ExchangeKind::Direct,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("declare parser exchange: {error}"))
            })?;

        self.declare_queue(&self.config.execute_queue, FieldTable::default())
            .await?;
        self.bind_queue(&self.config.execute_queue, &self.config.execute_routing_key)
            .await?;

        let mut retry_args = FieldTable::default();
        retry_args.insert(
            "x-message-ttl".into(),
            AMQPValue::LongUInt(self.config.retry_ttl_ms),
        );
        retry_args.insert(
            "x-dead-letter-exchange".into(),
            AMQPValue::LongString(self.config.exchange.clone().into()),
        );
        retry_args.insert(
            "x-dead-letter-routing-key".into(),
            AMQPValue::LongString(self.config.execute_routing_key.clone().into()),
        );
        self.declare_queue(&self.config.retry_queue, retry_args)
            .await?;
        self.bind_queue(&self.config.retry_queue, &self.config.retry_routing_key)
            .await?;

        self.declare_queue(&self.config.dead_queue, FieldTable::default())
            .await?;
        self.bind_queue(&self.config.dead_queue, &self.config.dead_routing_key)
            .await?;
        Ok(())
    }

    async fn declare_queue(&self, queue: &str, args: FieldTable) -> Result<(), AppError> {
        self.channel
            .queue_declare(
                queue,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                args,
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("declare parser queue {queue}: {error}"))
            })?;
        Ok(())
    }

    async fn bind_queue(&self, queue: &str, routing_key: &str) -> Result<(), AppError> {
        self.channel
            .queue_bind(
                queue,
                &self.config.exchange,
                routing_key,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("bind parser queue {queue}: {error}"))
            })?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct AgentRabbitMqClient {
    channel: Channel,
    config: AgentRabbitMqConfig,
}

impl AgentRabbitMqClient {
    pub async fn connect(config: AgentRabbitMqConfig) -> Result<Self, AppError> {
        let connection = Connection::connect(&config.url, ConnectionProperties::default())
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("connect agent RabbitMQ: {error}"))
            })?;
        let channel = connection.create_channel().await.map_err(|error| {
            AppError::Anyhow(anyhow::anyhow!("create agent RabbitMQ channel: {error}"))
        })?;
        channel
            .confirm_select(ConfirmSelectOptions::default())
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!(
                    "enable agent RabbitMQ publisher confirms: {error}"
                ))
            })?;
        let client = Self { channel, config };
        client.declare_agent_topology().await?;
        Ok(client)
    }

    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    pub fn config(&self) -> &AgentRabbitMqConfig {
        &self.config
    }

    pub async fn publish_agent_execute(&self, message: &AgentQueueMessage) -> Result<(), AppError> {
        self.publish(&self.config.execute_routing_key, message)
            .await
    }

    pub async fn publish_agent_retry(&self, message: &AgentQueueMessage) -> Result<(), AppError> {
        self.publish(&self.config.retry_routing_key, message).await
    }

    pub async fn publish_agent_dead(&self, message: &AgentQueueMessage) -> Result<(), AppError> {
        self.publish(&self.config.dead_routing_key, message).await
    }

    async fn publish(
        &self,
        routing_key: &str,
        message: &AgentQueueMessage,
    ) -> Result<(), AppError> {
        let payload = serde_json::to_vec(message).map_err(|error| {
            AppError::Anyhow(anyhow::anyhow!("encode agent execute message: {error}"))
        })?;
        let confirmation = self
            .channel
            .basic_publish(
                &self.config.exchange,
                routing_key,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default().with_content_type("application/json".into()),
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("publish agent execute message: {error}"))
            })?
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("confirm agent execute message: {error}"))
            })?;
        if !matches!(confirmation, Confirmation::Ack(_)) {
            return Err(AppError::Anyhow(anyhow::anyhow!(
                "RabbitMQ did not ack agent execute message"
            )));
        }
        Ok(())
    }

    async fn declare_agent_topology(&self) -> Result<(), AppError> {
        self.channel
            .exchange_declare(
                &self.config.exchange,
                ExchangeKind::Direct,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("declare agent exchange: {error}"))
            })?;

        self.declare_queue(&self.config.execute_queue, FieldTable::default())
            .await?;
        self.bind_queue(&self.config.execute_queue, &self.config.execute_routing_key)
            .await?;

        let mut retry_args = FieldTable::default();
        retry_args.insert(
            "x-message-ttl".into(),
            AMQPValue::LongUInt(self.config.retry_ttl_ms),
        );
        retry_args.insert(
            "x-dead-letter-exchange".into(),
            AMQPValue::LongString(self.config.exchange.clone().into()),
        );
        retry_args.insert(
            "x-dead-letter-routing-key".into(),
            AMQPValue::LongString(self.config.execute_routing_key.clone().into()),
        );
        self.declare_queue(&self.config.retry_queue, retry_args)
            .await?;
        self.bind_queue(&self.config.retry_queue, &self.config.retry_routing_key)
            .await?;

        self.declare_queue(&self.config.dead_queue, FieldTable::default())
            .await?;
        self.bind_queue(&self.config.dead_queue, &self.config.dead_routing_key)
            .await?;
        Ok(())
    }

    async fn declare_queue(&self, queue: &str, args: FieldTable) -> Result<(), AppError> {
        self.channel
            .queue_declare(
                queue,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                args,
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("declare agent queue {queue}: {error}"))
            })?;
        Ok(())
    }

    async fn bind_queue(&self, queue: &str, routing_key: &str) -> Result<(), AppError> {
        self.channel
            .queue_bind(
                queue,
                &self.config.exchange,
                routing_key,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("bind agent queue {queue}: {error}"))
            })?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct EvalRabbitMqClient {
    channel: Channel,
    config: EvalRabbitMqConfig,
}

impl EvalRabbitMqClient {
    pub async fn connect(config: EvalRabbitMqConfig) -> Result<Self, AppError> {
        let connection = Connection::connect(&config.url, ConnectionProperties::default())
            .await
            .map_err(|error| AppError::Anyhow(anyhow::anyhow!("connect eval RabbitMQ: {error}")))?;
        let channel = connection.create_channel().await.map_err(|error| {
            AppError::Anyhow(anyhow::anyhow!("create eval RabbitMQ channel: {error}"))
        })?;
        channel
            .confirm_select(ConfirmSelectOptions::default())
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!(
                    "enable eval RabbitMQ publisher confirms: {error}"
                ))
            })?;
        let client = Self { channel, config };
        client.declare_eval_topology().await?;
        Ok(client)
    }

    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    pub fn config(&self) -> &EvalRabbitMqConfig {
        &self.config
    }

    pub async fn publish_eval_execute(&self, message: &EvalTaskMessage) -> Result<(), AppError> {
        self.publish(&self.config.execute_routing_key, message)
            .await
    }

    pub async fn publish_eval_retry(&self, message: &EvalTaskMessage) -> Result<(), AppError> {
        self.publish(&self.config.retry_routing_key, message).await
    }

    pub async fn publish_eval_dead(&self, message: &EvalTaskMessage) -> Result<(), AppError> {
        self.publish(&self.config.dead_routing_key, message).await
    }

    async fn publish(&self, routing_key: &str, message: &EvalTaskMessage) -> Result<(), AppError> {
        let payload = serde_json::to_vec(message).map_err(|error| {
            AppError::Anyhow(anyhow::anyhow!("encode eval task message: {error}"))
        })?;
        let confirmation = self
            .channel
            .basic_publish(
                &self.config.exchange,
                routing_key,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default().with_content_type("application/json".into()),
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("publish eval task message: {error}"))
            })?
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("confirm eval task message: {error}"))
            })?;
        if !matches!(confirmation, Confirmation::Ack(_)) {
            return Err(AppError::Anyhow(anyhow::anyhow!(
                "RabbitMQ did not ack eval task message"
            )));
        }
        Ok(())
    }

    async fn declare_eval_topology(&self) -> Result<(), AppError> {
        self.channel
            .exchange_declare(
                &self.config.exchange,
                ExchangeKind::Direct,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|error| AppError::Anyhow(anyhow::anyhow!("declare eval exchange: {error}")))?;

        self.declare_queue(&self.config.execute_queue, FieldTable::default())
            .await?;
        self.bind_queue(&self.config.execute_queue, &self.config.execute_routing_key)
            .await?;

        let mut retry_args = FieldTable::default();
        retry_args.insert(
            "x-message-ttl".into(),
            AMQPValue::LongUInt(self.config.retry_ttl_ms),
        );
        retry_args.insert(
            "x-dead-letter-exchange".into(),
            AMQPValue::LongString(self.config.exchange.clone().into()),
        );
        retry_args.insert(
            "x-dead-letter-routing-key".into(),
            AMQPValue::LongString(self.config.execute_routing_key.clone().into()),
        );
        self.declare_queue(&self.config.retry_queue, retry_args)
            .await?;
        self.bind_queue(&self.config.retry_queue, &self.config.retry_routing_key)
            .await?;

        self.declare_queue(&self.config.dead_queue, FieldTable::default())
            .await?;
        self.bind_queue(&self.config.dead_queue, &self.config.dead_routing_key)
            .await?;
        Ok(())
    }

    async fn declare_queue(&self, queue: &str, args: FieldTable) -> Result<(), AppError> {
        self.channel
            .queue_declare(
                queue,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                args,
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("declare eval queue {queue}: {error}"))
            })?;
        Ok(())
    }

    async fn bind_queue(&self, queue: &str, routing_key: &str) -> Result<(), AppError> {
        self.channel
            .queue_bind(
                queue,
                &self.config.exchange,
                routing_key,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|error| {
                AppError::Anyhow(anyhow::anyhow!("bind eval queue {queue}: {error}"))
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_message_serializes_with_camel_case_fields() {
        let message = SchedulerMessage {
            trigger_id: 1,
            job_id: 2,
            task_type: 1,
            attempt: 1,
            max_attempts: 3,
        };

        let value = serde_json::to_value(message).unwrap();

        assert_eq!(value["triggerId"], 1);
        assert_eq!(value["jobId"], 2);
        assert_eq!(value["maxAttempts"], 3);
    }

    #[test]
    fn parser_message_serializes_with_camel_case_fields() {
        let message = ParserJobMessage {
            outbox_id: 1,
            tenant_id: 2,
            dataset_id: 3,
            document_id: 4,
            parser_job_id: 5,
            attempt: 0,
            max_attempts: 5,
            parser_request: serde_json::json!({"source": {"name": "handbook.md"}}),
        };

        let value = serde_json::to_value(message).unwrap();

        assert_eq!(value["outboxId"], 1);
        assert_eq!(value["tenantId"], 2);
        assert_eq!(value["datasetId"], 3);
        assert_eq!(value["documentId"], 4);
        assert_eq!(value["parserJobId"], 5);
        assert_eq!(value["maxAttempts"], 5);
        assert_eq!(value["parserRequest"]["source"]["name"], "handbook.md");
    }

    #[test]
    fn eval_task_message_serializes_with_camel_case_fields() {
        let message = EvalTaskMessage {
            outbox_id: 10,
            tenant_id: 1,
            run_id: 20,
            task_id: 30,
            case_id: 40,
            run_mode: "live_rag".to_owned(),
            attempt: 0,
            max_attempts: 3,
        };

        let value = serde_json::to_value(message).unwrap();

        assert_eq!(value["outboxId"], 10);
        assert_eq!(value["tenantId"], 1);
        assert_eq!(value["runId"], 20);
        assert_eq!(value["taskId"], 30);
        assert_eq!(value["caseId"], 40);
        assert_eq!(value["runMode"], "live_rag");
        assert_eq!(value["maxAttempts"], 3);
    }

    #[test]
    fn agent_queue_broker_wakeup_message_serializes_with_camel_case_fields() {
        let message = AgentQueueMessage {
            queue_id: 1,
            tenant_id: 2,
            run_id: 3,
            event: "agent.run.queued".to_owned(),
            attempt: 0,
            max_attempts: 3,
            source: "agent.create_run".to_owned(),
        };

        let value = serde_json::to_value(message).unwrap();

        assert_eq!(value["queueId"], 1);
        assert_eq!(value["tenantId"], 2);
        assert_eq!(value["runId"], 3);
        assert_eq!(value["event"], "agent.run.queued");
        assert_eq!(value["maxAttempts"], 3);
        assert_eq!(value["source"], "agent.create_run");
    }

    #[test]
    fn parser_rabbitmq_config_defaults_to_dedicated_topology() {
        let config = ParserRabbitMqConfig::default();

        assert_eq!(config.exchange, "novex.parser");
        assert_eq!(config.execute_queue, "novex.parser.execute");
        assert_eq!(config.retry_queue, "novex.parser.retry");
        assert_eq!(config.dead_queue, "novex.parser.dead");
        assert_eq!(config.execute_routing_key, "parser.execute");
        assert_eq!(config.retry_routing_key, "parser.retry");
        assert_eq!(config.dead_routing_key, "parser.dead");
        assert_eq!(config.retry_ttl_ms, 30_000);
    }

    #[test]
    fn agent_queue_broker_wakeup_config_defaults_to_dedicated_topology() {
        let config = AgentRabbitMqConfig::default();

        assert_eq!(config.exchange, "novex.agent");
        assert_eq!(config.execute_queue, "novex.agent.execute");
        assert_eq!(config.retry_queue, "novex.agent.retry");
        assert_eq!(config.dead_queue, "novex.agent.dead");
        assert_eq!(config.execute_routing_key, "agent.execute");
        assert_eq!(config.retry_routing_key, "agent.retry");
        assert_eq!(config.dead_routing_key, "agent.dead");
        assert_eq!(config.retry_ttl_ms, 30_000);
    }

    #[test]
    fn eval_rabbitmq_config_defaults_to_dedicated_topology() {
        let config = EvalRabbitMqConfig::default();

        assert_eq!(config.exchange, "novex.eval");
        assert_eq!(config.execute_queue, "novex.eval.execute");
        assert_eq!(config.retry_queue, "novex.eval.retry");
        assert_eq!(config.dead_queue, "novex.eval.dead");
        assert_eq!(config.execute_routing_key, "eval.execute");
        assert_eq!(config.retry_routing_key, "eval.retry");
        assert_eq!(config.dead_routing_key, "eval.dead");
        assert_eq!(config.retry_ttl_ms, 30_000);
    }

    #[test]
    fn rabbitmq_module_defines_parser_client_publish_path() {
        let source = include_str!("rabbitmq.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "ParserRabbitMqClient",
            "publish_parser_execute",
            "declare_parser_topology",
            "parser execute message",
            "novex.parser.execute",
        ] {
            assert!(
                source.contains(needle),
                "{needle} missing from RabbitMQ module"
            );
        }
    }

    #[test]
    fn agent_queue_broker_wakeup_module_defines_publish_path() {
        let source = include_str!("rabbitmq.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "AgentRabbitMqClient",
            "publish_agent_execute",
            "declare_agent_topology",
            "agent execute message",
            "novex.agent.execute",
        ] {
            assert!(
                source.contains(needle),
                "{needle} missing from RabbitMQ module"
            );
        }
    }

    #[test]
    fn eval_rabbitmq_module_defines_publish_path() {
        let source = include_str!("rabbitmq.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        for needle in [
            "EvalRabbitMqClient",
            "publish_eval_execute",
            "declare_eval_topology",
            "eval task message",
            "novex.eval.execute",
        ] {
            assert!(
                source.contains(needle),
                "{needle} missing from RabbitMQ module"
            );
        }
    }

    #[test]
    fn rabbitmq_clients_enable_publisher_confirms_before_publishing() {
        let source = include_str!("rabbitmq.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        let confirm_select_calls = source.matches(".confirm_select(").count();

        assert!(
            confirm_select_calls >= 4,
            "scheduler, parser, agent, and eval RabbitMQ clients must enable publisher confirms"
        );
    }
}
