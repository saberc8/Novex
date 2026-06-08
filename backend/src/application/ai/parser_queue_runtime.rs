use crate::{
    infrastructure::mq::rabbitmq::ParserRabbitMqConfig,
    shared::config::AppConfig,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_queue_runtime_config_uses_dedicated_parser_topology() {
        let app = test_app_config();

        let config = parser_queue_from_config(&app);
        let rabbitmq = parser_rabbitmq_from_config(&app);

        assert!(!config.enabled);
        assert!(!config.publisher_enabled);
        assert_eq!(config.tick_seconds, 5);
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.redis_url, "redis://127.0.0.1:6379/0");
        assert_eq!(rabbitmq.exchange, "novex.parser");
        assert_eq!(rabbitmq.execute_queue, "novex.parser.execute");
        assert_eq!(rabbitmq.retry_queue, "novex.parser.retry");
        assert_eq!(rabbitmq.dead_queue, "novex.parser.dead");
        assert_eq!(rabbitmq.execute_routing_key, "parser.execute");
        assert_eq!(rabbitmq.retry_routing_key, "parser.retry");
        assert_eq!(rabbitmq.dead_routing_key, "parser.dead");
    }

    fn test_app_config() -> AppConfig {
        AppConfig {
            http_port: 4398,
            database_url: "postgres://postgres:postgres@localhost:5432/avalon_admin".to_owned(),
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
            redis_url: "redis://127.0.0.1:6379/0".to_owned(),
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
