use std::env;

use anyhow::{bail, Context, Result};

const DEFAULT_CORS_ALLOWED_ORIGINS: &str =
    "http://localhost:4399,http://127.0.0.1:4399,http://localhost:5173,http://127.0.0.1:5173";
const JWT_SECRET_PLACEHOLDER: &str = "dev-only-change-me";
const JWT_SECRET_MIN_LEN: usize = 32;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub http_port: u16,
    pub database_url: String,
    pub database_max_connections: u32,
    pub db_auto_migrate: bool,
    pub cors_allowed_origins: Vec<String>,
    pub auth_jwt_secret: String,
    pub auth_jwt_ttl_hours: u64,
    pub scheduler_embedded: bool,
    pub scheduler_worker_enabled: bool,
    pub scheduler_tick_seconds: u64,
    pub scheduler_batch_size: i64,
    pub scheduler_worker_id: String,
    pub scheduler_http_allowlist_mode: String,
    pub scheduler_http_allowlist: Vec<String>,
    pub rabbitmq_url: String,
    pub rabbitmq_exchange: String,
    pub rabbitmq_execute_queue: String,
    pub rabbitmq_retry_queue: String,
    pub rabbitmq_dead_queue: String,
    pub rabbitmq_execute_routing_key: String,
    pub rabbitmq_retry_routing_key: String,
    pub rabbitmq_dead_routing_key: String,
    pub rabbitmq_retry_ttl_ms: u32,
    pub parser_queue_enabled: bool,
    pub parser_queue_publisher_enabled: bool,
    pub parser_queue_tick_seconds: u64,
    pub parser_queue_batch_size: i64,
    pub agent_queue_enabled: bool,
    pub agent_queue_publisher_enabled: bool,
    pub agent_queue_tick_seconds: u64,
    pub agent_queue_batch_size: i64,
    pub agent_queue_lease_seconds: u64,
    pub agent_queue_max_attempts: i32,
    pub agent_queue_worker_id: String,
    pub eval_queue_enabled: bool,
    pub eval_queue_publisher_enabled: bool,
    pub eval_queue_tick_seconds: u64,
    pub eval_queue_batch_size: i64,
    pub eval_worker_enabled: bool,
    pub eval_worker_id: String,
    pub eval_task_timeout_seconds: u64,
    pub redis_url: String,
    pub rabbitmq_parser_exchange: String,
    pub rabbitmq_parser_execute_queue: String,
    pub rabbitmq_parser_retry_queue: String,
    pub rabbitmq_parser_dead_queue: String,
    pub rabbitmq_parser_execute_routing_key: String,
    pub rabbitmq_parser_retry_routing_key: String,
    pub rabbitmq_parser_dead_routing_key: String,
    pub rabbitmq_parser_retry_ttl_ms: u32,
    pub rabbitmq_agent_exchange: String,
    pub rabbitmq_agent_execute_queue: String,
    pub rabbitmq_agent_retry_queue: String,
    pub rabbitmq_agent_dead_queue: String,
    pub rabbitmq_agent_execute_routing_key: String,
    pub rabbitmq_agent_retry_routing_key: String,
    pub rabbitmq_agent_dead_routing_key: String,
    pub rabbitmq_agent_retry_ttl_ms: u32,
    pub rabbitmq_eval_exchange: String,
    pub rabbitmq_eval_execute_queue: String,
    pub rabbitmq_eval_retry_queue: String,
    pub rabbitmq_eval_dead_queue: String,
    pub rabbitmq_eval_execute_routing_key: String,
    pub rabbitmq_eval_retry_routing_key: String,
    pub rabbitmq_eval_dead_routing_key: String,
    pub rabbitmq_eval_retry_ttl_ms: u32,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let http_port = env::var("HTTP_PORT")
            .unwrap_or_else(|_| "4398".to_owned())
            .parse::<u16>()
            .context("HTTP_PORT must be a valid TCP port")?;

        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:15432/novex".to_owned());

        let database_max_connections = parse_database_max_connections(
            &env::var("DATABASE_MAX_CONNECTIONS").unwrap_or_else(|_| "5".to_owned()),
        )?;
        let db_auto_migrate_raw = env::var("DB_AUTO_MIGRATE").ok();
        let db_auto_migrate = parse_bool_env(db_auto_migrate_raw.as_deref(), false)?;
        let cors_allowed_origins = parse_cors_allowed_origins(
            &env::var("CORS_ALLOWED_ORIGINS")
                .unwrap_or_else(|_| DEFAULT_CORS_ALLOWED_ORIGINS.to_owned()),
        );
        let auth_jwt_secret_raw = env::var("AUTH_JWT_SECRET").ok();
        let auth_jwt_secret = parse_auth_jwt_secret(auth_jwt_secret_raw.as_deref())?;
        let auth_jwt_ttl_hours = parse_positive_u64_env(
            "AUTH_JWT_TTL_HOURS",
            &env::var("AUTH_JWT_TTL_HOURS").unwrap_or_else(|_| "24".to_owned()),
        )?;
        let scheduler_embedded =
            parse_bool_env(env::var("SCHEDULER_EMBEDDED").ok().as_deref(), false)?;
        let scheduler_worker_enabled =
            parse_bool_env(env::var("SCHEDULER_WORKER_ENABLED").ok().as_deref(), true)?;
        let scheduler_tick_seconds = parse_positive_u64_env(
            "SCHEDULER_TICK_SECONDS",
            &env::var("SCHEDULER_TICK_SECONDS").unwrap_or_else(|_| "5".to_owned()),
        )?;
        let scheduler_batch_size = parse_positive_i64_env(
            "SCHEDULER_BATCH_SIZE",
            &env::var("SCHEDULER_BATCH_SIZE").unwrap_or_else(|_| "50".to_owned()),
        )?;
        let scheduler_worker_id = env::var("SCHEDULER_WORKER_ID")
            .unwrap_or_else(|_| format!("worker-{}", std::process::id()));
        let scheduler_http_allowlist_mode =
            env::var("SCHEDULER_HTTP_ALLOWLIST_MODE").unwrap_or_else(|_| "default".to_owned());
        let scheduler_http_allowlist =
            parse_cors_allowed_origins(&env::var("SCHEDULER_HTTP_ALLOWLIST").unwrap_or_default());
        let rabbitmq_url = env::var("RABBITMQ_URL")
            .unwrap_or_else(|_| "amqp://guest:guest@127.0.0.1:5673/%2f".to_owned());
        let rabbitmq_exchange =
            env::var("RABBITMQ_EXCHANGE").unwrap_or_else(|_| "avalon.scheduler".to_owned());
        let rabbitmq_execute_queue = env::var("RABBITMQ_SCHEDULER_EXECUTE_QUEUE")
            .unwrap_or_else(|_| "avalon.scheduler.execute".to_owned());
        let rabbitmq_retry_queue = env::var("RABBITMQ_SCHEDULER_RETRY_QUEUE")
            .unwrap_or_else(|_| "avalon.scheduler.retry".to_owned());
        let rabbitmq_dead_queue = env::var("RABBITMQ_SCHEDULER_DEAD_QUEUE")
            .unwrap_or_else(|_| "avalon.scheduler.dead".to_owned());
        let rabbitmq_execute_routing_key = env::var("RABBITMQ_SCHEDULER_EXECUTE_ROUTING_KEY")
            .unwrap_or_else(|_| "scheduler.execute".to_owned());
        let rabbitmq_retry_routing_key = env::var("RABBITMQ_SCHEDULER_RETRY_ROUTING_KEY")
            .unwrap_or_else(|_| "scheduler.retry".to_owned());
        let rabbitmq_dead_routing_key = env::var("RABBITMQ_SCHEDULER_DEAD_ROUTING_KEY")
            .unwrap_or_else(|_| "scheduler.dead".to_owned());
        let rabbitmq_retry_ttl_ms = parse_positive_u32_env(
            "RABBITMQ_SCHEDULER_RETRY_TTL_MS",
            &env::var("RABBITMQ_SCHEDULER_RETRY_TTL_MS").unwrap_or_else(|_| "30000".to_owned()),
        )?;
        let parser_queue_enabled =
            parse_bool_env(env::var("PARSER_QUEUE_ENABLED").ok().as_deref(), false)?;
        let parser_queue_publisher_enabled = parse_bool_env(
            env::var("PARSER_QUEUE_PUBLISHER_ENABLED").ok().as_deref(),
            parser_queue_enabled,
        )?;
        let parser_queue_tick_seconds = parse_positive_u64_env(
            "PARSER_QUEUE_TICK_SECONDS",
            &env::var("PARSER_QUEUE_TICK_SECONDS").unwrap_or_else(|_| "5".to_owned()),
        )?;
        let parser_queue_batch_size = parse_positive_i64_env(
            "PARSER_QUEUE_BATCH_SIZE",
            &env::var("PARSER_QUEUE_BATCH_SIZE").unwrap_or_else(|_| "50".to_owned()),
        )?;
        let agent_queue_enabled =
            parse_bool_env(env::var("AGENT_QUEUE_ENABLED").ok().as_deref(), false)?;
        let agent_queue_publisher_enabled = parse_bool_env(
            env::var("AGENT_QUEUE_PUBLISHER_ENABLED").ok().as_deref(),
            agent_queue_enabled,
        )?;
        let agent_queue_tick_seconds = parse_positive_u64_env(
            "AGENT_QUEUE_TICK_SECONDS",
            &env::var("AGENT_QUEUE_TICK_SECONDS").unwrap_or_else(|_| "2".to_owned()),
        )?;
        let agent_queue_batch_size = parse_positive_i64_env(
            "AGENT_QUEUE_BATCH_SIZE",
            &env::var("AGENT_QUEUE_BATCH_SIZE").unwrap_or_else(|_| "10".to_owned()),
        )?;
        let agent_queue_lease_seconds = parse_positive_u64_env(
            "AGENT_QUEUE_LEASE_SECONDS",
            &env::var("AGENT_QUEUE_LEASE_SECONDS").unwrap_or_else(|_| "120".to_owned()),
        )?;
        let agent_queue_max_attempts = parse_positive_i64_env(
            "AGENT_QUEUE_MAX_ATTEMPTS",
            &env::var("AGENT_QUEUE_MAX_ATTEMPTS").unwrap_or_else(|_| "3".to_owned()),
        )? as i32;
        let agent_queue_worker_id = env::var("AGENT_QUEUE_WORKER_ID")
            .unwrap_or_else(|_| format!("agent-worker-{}", std::process::id()));
        let eval_queue_enabled =
            parse_bool_env(env::var("EVAL_QUEUE_ENABLED").ok().as_deref(), false)?;
        let eval_queue_publisher_enabled = parse_bool_env(
            env::var("EVAL_QUEUE_PUBLISHER_ENABLED").ok().as_deref(),
            eval_queue_enabled,
        )?;
        let eval_queue_tick_seconds = parse_positive_u64_env(
            "EVAL_QUEUE_TICK_SECONDS",
            &env::var("EVAL_QUEUE_TICK_SECONDS").unwrap_or_else(|_| "5".to_owned()),
        )?;
        let eval_queue_batch_size = parse_positive_i64_env(
            "EVAL_QUEUE_BATCH_SIZE",
            &env::var("EVAL_QUEUE_BATCH_SIZE").unwrap_or_else(|_| "50".to_owned()),
        )?;
        let eval_worker_enabled =
            parse_bool_env(env::var("EVAL_WORKER_ENABLED").ok().as_deref(), false)?;
        let eval_worker_id = env::var("EVAL_WORKER_ID")
            .unwrap_or_else(|_| format!("eval-worker-{}", std::process::id()));
        let eval_task_timeout_seconds = parse_positive_u64_env(
            "EVAL_TASK_TIMEOUT_SECONDS",
            &env::var("EVAL_TASK_TIMEOUT_SECONDS").unwrap_or_else(|_| "180".to_owned()),
        )?;
        let redis_url =
            env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:16379/0".to_owned());
        let rabbitmq_parser_exchange =
            env::var("RABBITMQ_PARSER_EXCHANGE").unwrap_or_else(|_| "novex.parser".to_owned());
        let rabbitmq_parser_execute_queue = env::var("RABBITMQ_PARSER_EXECUTE_QUEUE")
            .unwrap_or_else(|_| "novex.parser.execute".to_owned());
        let rabbitmq_parser_retry_queue = env::var("RABBITMQ_PARSER_RETRY_QUEUE")
            .unwrap_or_else(|_| "novex.parser.retry".to_owned());
        let rabbitmq_parser_dead_queue = env::var("RABBITMQ_PARSER_DEAD_QUEUE")
            .unwrap_or_else(|_| "novex.parser.dead".to_owned());
        let rabbitmq_parser_execute_routing_key = env::var("RABBITMQ_PARSER_EXECUTE_ROUTING_KEY")
            .unwrap_or_else(|_| "parser.execute".to_owned());
        let rabbitmq_parser_retry_routing_key = env::var("RABBITMQ_PARSER_RETRY_ROUTING_KEY")
            .unwrap_or_else(|_| "parser.retry".to_owned());
        let rabbitmq_parser_dead_routing_key = env::var("RABBITMQ_PARSER_DEAD_ROUTING_KEY")
            .unwrap_or_else(|_| "parser.dead".to_owned());
        let rabbitmq_parser_retry_ttl_ms = parse_positive_u32_env(
            "RABBITMQ_PARSER_RETRY_TTL_MS",
            &env::var("RABBITMQ_PARSER_RETRY_TTL_MS").unwrap_or_else(|_| "30000".to_owned()),
        )?;
        let rabbitmq_agent_exchange =
            env::var("RABBITMQ_AGENT_EXCHANGE").unwrap_or_else(|_| "novex.agent".to_owned());
        let rabbitmq_agent_execute_queue = env::var("RABBITMQ_AGENT_EXECUTE_QUEUE")
            .unwrap_or_else(|_| "novex.agent.execute".to_owned());
        let rabbitmq_agent_retry_queue = env::var("RABBITMQ_AGENT_RETRY_QUEUE")
            .unwrap_or_else(|_| "novex.agent.retry".to_owned());
        let rabbitmq_agent_dead_queue =
            env::var("RABBITMQ_AGENT_DEAD_QUEUE").unwrap_or_else(|_| "novex.agent.dead".to_owned());
        let rabbitmq_agent_execute_routing_key = env::var("RABBITMQ_AGENT_EXECUTE_ROUTING_KEY")
            .unwrap_or_else(|_| "agent.execute".to_owned());
        let rabbitmq_agent_retry_routing_key = env::var("RABBITMQ_AGENT_RETRY_ROUTING_KEY")
            .unwrap_or_else(|_| "agent.retry".to_owned());
        let rabbitmq_agent_dead_routing_key =
            env::var("RABBITMQ_AGENT_DEAD_ROUTING_KEY").unwrap_or_else(|_| "agent.dead".to_owned());
        let rabbitmq_agent_retry_ttl_ms = parse_positive_u32_env(
            "RABBITMQ_AGENT_RETRY_TTL_MS",
            &env::var("RABBITMQ_AGENT_RETRY_TTL_MS").unwrap_or_else(|_| "30000".to_owned()),
        )?;
        let rabbitmq_eval_exchange =
            env::var("RABBITMQ_EVAL_EXCHANGE").unwrap_or_else(|_| "novex.eval".to_owned());
        let rabbitmq_eval_execute_queue = env::var("RABBITMQ_EVAL_EXECUTE_QUEUE")
            .unwrap_or_else(|_| "novex.eval.execute".to_owned());
        let rabbitmq_eval_retry_queue =
            env::var("RABBITMQ_EVAL_RETRY_QUEUE").unwrap_or_else(|_| "novex.eval.retry".to_owned());
        let rabbitmq_eval_dead_queue =
            env::var("RABBITMQ_EVAL_DEAD_QUEUE").unwrap_or_else(|_| "novex.eval.dead".to_owned());
        let rabbitmq_eval_execute_routing_key = env::var("RABBITMQ_EVAL_EXECUTE_ROUTING_KEY")
            .unwrap_or_else(|_| "eval.execute".to_owned());
        let rabbitmq_eval_retry_routing_key =
            env::var("RABBITMQ_EVAL_RETRY_ROUTING_KEY").unwrap_or_else(|_| "eval.retry".to_owned());
        let rabbitmq_eval_dead_routing_key =
            env::var("RABBITMQ_EVAL_DEAD_ROUTING_KEY").unwrap_or_else(|_| "eval.dead".to_owned());
        let rabbitmq_eval_retry_ttl_ms = parse_positive_u32_env(
            "RABBITMQ_EVAL_RETRY_TTL_MS",
            &env::var("RABBITMQ_EVAL_RETRY_TTL_MS").unwrap_or_else(|_| "30000".to_owned()),
        )?;

        if cors_allowed_origins.is_empty() {
            bail!("CORS_ALLOWED_ORIGINS must include at least one origin");
        }

        Ok(Self {
            http_port,
            database_url,
            database_max_connections,
            db_auto_migrate,
            cors_allowed_origins,
            auth_jwt_secret,
            auth_jwt_ttl_hours,
            scheduler_embedded,
            scheduler_worker_enabled,
            scheduler_tick_seconds,
            scheduler_batch_size,
            scheduler_worker_id,
            scheduler_http_allowlist_mode,
            scheduler_http_allowlist,
            rabbitmq_url,
            rabbitmq_exchange,
            rabbitmq_execute_queue,
            rabbitmq_retry_queue,
            rabbitmq_dead_queue,
            rabbitmq_execute_routing_key,
            rabbitmq_retry_routing_key,
            rabbitmq_dead_routing_key,
            rabbitmq_retry_ttl_ms,
            parser_queue_enabled,
            parser_queue_publisher_enabled,
            parser_queue_tick_seconds,
            parser_queue_batch_size,
            agent_queue_enabled,
            agent_queue_publisher_enabled,
            agent_queue_tick_seconds,
            agent_queue_batch_size,
            agent_queue_lease_seconds,
            agent_queue_max_attempts,
            agent_queue_worker_id,
            eval_queue_enabled,
            eval_queue_publisher_enabled,
            eval_queue_tick_seconds,
            eval_queue_batch_size,
            eval_worker_enabled,
            eval_worker_id,
            eval_task_timeout_seconds,
            redis_url,
            rabbitmq_parser_exchange,
            rabbitmq_parser_execute_queue,
            rabbitmq_parser_retry_queue,
            rabbitmq_parser_dead_queue,
            rabbitmq_parser_execute_routing_key,
            rabbitmq_parser_retry_routing_key,
            rabbitmq_parser_dead_routing_key,
            rabbitmq_parser_retry_ttl_ms,
            rabbitmq_agent_exchange,
            rabbitmq_agent_execute_queue,
            rabbitmq_agent_retry_queue,
            rabbitmq_agent_dead_queue,
            rabbitmq_agent_execute_routing_key,
            rabbitmq_agent_retry_routing_key,
            rabbitmq_agent_dead_routing_key,
            rabbitmq_agent_retry_ttl_ms,
            rabbitmq_eval_exchange,
            rabbitmq_eval_execute_queue,
            rabbitmq_eval_retry_queue,
            rabbitmq_eval_dead_queue,
            rabbitmq_eval_execute_routing_key,
            rabbitmq_eval_retry_routing_key,
            rabbitmq_eval_dead_routing_key,
            rabbitmq_eval_retry_ttl_ms,
        })
    }
}

fn parse_database_max_connections(raw: &str) -> Result<u32> {
    let value = raw
        .parse::<u32>()
        .context("DATABASE_MAX_CONNECTIONS must be a positive integer")?;

    if value == 0 {
        bail!("DATABASE_MAX_CONNECTIONS must be a positive integer");
    }

    Ok(value)
}

fn parse_cors_allowed_origins(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_bool_env(raw: Option<&str>, default: bool) -> Result<bool> {
    let Some(raw) = raw else {
        return Ok(default);
    };

    match raw.trim().to_ascii_lowercase().as_str() {
        "" => Ok(default),
        "true" | "1" | "yes" | "y" | "on" => Ok(true),
        "false" | "0" | "no" | "n" | "off" => Ok(false),
        _ => bail!("DB_AUTO_MIGRATE must be a boolean"),
    }
}

fn parse_positive_u64_env(name: &str, raw: &str) -> Result<u64> {
    let value = raw
        .parse::<u64>()
        .with_context(|| format!("{name} must be a positive integer"))?;

    if value == 0 {
        bail!("{name} must be a positive integer");
    }

    Ok(value)
}

fn parse_positive_i64_env(name: &str, raw: &str) -> Result<i64> {
    let value = raw
        .parse::<i64>()
        .with_context(|| format!("{name} must be a positive integer"))?;

    if value <= 0 {
        bail!("{name} must be a positive integer");
    }

    Ok(value)
}

fn parse_positive_u32_env(name: &str, raw: &str) -> Result<u32> {
    let value = raw
        .parse::<u32>()
        .with_context(|| format!("{name} must be a positive integer"))?;

    if value == 0 {
        bail!("{name} must be a positive integer");
    }

    Ok(value)
}

fn parse_auth_jwt_secret(raw: Option<&str>) -> Result<String> {
    let Some(raw) = raw else {
        bail!("AUTH_JWT_SECRET must be set");
    };
    let secret = raw.trim();

    if secret.is_empty() {
        bail!("AUTH_JWT_SECRET must not be empty");
    }
    if secret == JWT_SECRET_PLACEHOLDER {
        bail!("AUTH_JWT_SECRET must not use the placeholder value");
    }
    if secret.len() < JWT_SECRET_MIN_LEN {
        bail!("AUTH_JWT_SECRET must be at least {JWT_SECRET_MIN_LEN} characters");
    }

    Ok(secret.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_max_connections_rejects_zero() {
        let err = parse_database_max_connections("0").unwrap_err();

        assert!(err.to_string().contains("positive"));
    }

    #[test]
    fn db_auto_migrate_defaults_to_false() {
        assert!(!parse_bool_env(None, false).unwrap());
    }

    #[test]
    fn db_auto_migrate_accepts_true() {
        assert!(parse_bool_env(Some("true"), false).unwrap());
        assert!(parse_bool_env(Some("1"), false).unwrap());
    }

    #[test]
    fn default_cors_allowed_origins_include_nextjs_dev_port() {
        let origins = parse_cors_allowed_origins(DEFAULT_CORS_ALLOWED_ORIGINS);

        assert!(origins.contains(&"http://localhost:4399".to_owned()));
        assert!(origins.contains(&"http://127.0.0.1:4399".to_owned()));
    }

    #[test]
    fn poc_compose_allows_codex_app_origin() {
        let compose_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../infra/docker-compose.yml");
        let compose =
            std::fs::read_to_string(compose_path).expect("read POC docker compose config");

        assert!(compose.contains("CORS_ALLOWED_ORIGINS: ${CORS_ALLOWED_ORIGINS:-"));
        assert!(compose.contains("http://localhost:${CODEX_APP_POC_PORT:-4413}"));
        assert!(compose.contains("http://127.0.0.1:${CODEX_APP_POC_PORT:-4413}"));
    }

    #[test]
    fn poc_compose_disables_login_captcha_for_dev_auto_login() {
        let compose_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../infra/docker-compose.yml");
        let compose =
            std::fs::read_to_string(compose_path).expect("read POC docker compose config");

        assert!(compose.contains("LOGIN_CAPTCHA_ENABLED: ${LOGIN_CAPTCHA_ENABLED:-false}"));
    }

    #[test]
    fn jwt_ttl_rejects_zero() {
        let err = parse_positive_u64_env("AUTH_JWT_TTL_HOURS", "0").unwrap_err();

        assert!(err.to_string().contains("positive"));
    }

    #[test]
    fn jwt_secret_rejects_missing_value() {
        let err = parse_auth_jwt_secret(None).unwrap_err();

        assert!(err.to_string().contains("AUTH_JWT_SECRET"));
    }

    #[test]
    fn jwt_secret_rejects_empty_value() {
        let err = parse_auth_jwt_secret(Some("   ")).unwrap_err();

        assert!(err.to_string().contains("AUTH_JWT_SECRET"));
    }

    #[test]
    fn jwt_secret_rejects_placeholder_value() {
        let err = parse_auth_jwt_secret(Some("dev-only-change-me")).unwrap_err();

        assert!(err.to_string().contains("placeholder"));
    }

    #[test]
    fn jwt_secret_rejects_short_value() {
        let err = parse_auth_jwt_secret(Some("short-secret")).unwrap_err();

        assert!(err.to_string().contains("at least 32"));
    }

    #[test]
    fn jwt_secret_accepts_valid_value() {
        let secret =
            parse_auth_jwt_secret(Some("local-dev-only-change-this-secret-32chars-min")).unwrap();

        assert_eq!(secret, "local-dev-only-change-this-secret-32chars-min");
    }

    #[test]
    fn scheduler_batch_size_rejects_zero() {
        let err = parse_positive_i64_env("SCHEDULER_BATCH_SIZE", "0").unwrap_err();

        assert!(err.to_string().contains("positive"));
    }

    #[test]
    fn agent_queue_publisher_config_defaults_to_queue_enabled() {
        let source = include_str!("config.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(source.contains("agent_queue_publisher_enabled"));
        assert!(source.contains("AGENT_QUEUE_PUBLISHER_ENABLED"));
        assert!(source.contains("agent_queue_enabled"));
    }

    #[test]
    fn eval_queue_config_defaults_are_safe() {
        assert!(!parse_bool_env(None, false).unwrap());
        assert!(parse_positive_u64_env("EVAL_TASK_TIMEOUT_SECONDS", "180").unwrap() > 0);
        assert!(parse_positive_i64_env("EVAL_QUEUE_BATCH_SIZE", "50").unwrap() > 0);
        assert!(parse_positive_u32_env("RABBITMQ_EVAL_RETRY_TTL_MS", "30000").unwrap() > 0);
    }
}
