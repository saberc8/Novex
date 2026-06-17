use std::net::SocketAddr;

use backend_rust::{
    application::ai::agent_queue_runtime::{
        agent_queue_from_config, agent_rabbitmq_from_config, spawn_agent_queue_broker_consumer,
        spawn_agent_queue_worker,
    },
    application::ai::agent_service::AgentRuntimeRegistry,
    application::ai::parser_queue_runtime::{
        parser_queue_from_config, parser_rabbitmq_from_config, spawn_parser_queue_publisher,
    },
    application::scheduler::runtime::{
        http_safety_from_config, rabbitmq_from_config, spawn_scheduler_runtime,
    },
    infrastructure::{db::init_pool, security::jwt::JwtService},
    interfaces::http::build_router_with_agent_runtime_and_scheduler_http_safety,
    shared::config::AppConfig,
};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = AppConfig::from_env()?;
    let db = init_pool(&config).await?;
    tracing::info!(
        db_auto_migrate = config.db_auto_migrate,
        "postgres pool configured"
    );

    let jwt = JwtService::new(config.auth_jwt_secret.clone(), config.auth_jwt_ttl_hours);
    let scheduler_http_safety = http_safety_from_config(&config)?;
    let agent_runtime = AgentRuntimeRegistry::default();
    let agent_queue_runtime = agent_queue_from_config(&config);
    if config.scheduler_embedded {
        spawn_scheduler_runtime(
            db.clone(),
            scheduler_http_safety.clone(),
            rabbitmq_from_config(&config),
            config.scheduler_worker_id.clone(),
            config.scheduler_tick_seconds,
            config.scheduler_batch_size,
        );
    }
    spawn_agent_queue_worker(
        db.clone(),
        agent_queue_runtime.clone(),
        agent_runtime.clone(),
    );
    spawn_agent_queue_broker_consumer(
        db.clone(),
        agent_queue_runtime,
        agent_rabbitmq_from_config(&config),
        agent_runtime.clone(),
    );
    spawn_parser_queue_publisher(
        db.clone(),
        parser_queue_from_config(&config),
        parser_rabbitmq_from_config(&config),
    );
    let app = build_router_with_agent_runtime_and_scheduler_http_safety(
        db,
        &config.cors_allowed_origins,
        jwt,
        agent_runtime,
        scheduler_http_safety,
    )?;
    let addr = SocketAddr::from(([0, 0, 0, 0], config.http_port));
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "rust admin backend listening");
    axum::serve(listener, app).await?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("backend_rust=debug,tower_http=info,axum::rejection=trace")
    });

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}
