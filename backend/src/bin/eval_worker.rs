use backend_rust::{
    application::ai::{
        eval_queue_runtime::eval_rabbitmq_from_config, eval_worker_runtime::run_eval_worker_runtime,
    },
    infrastructure::db::init_pool,
    shared::config::AppConfig,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = AppConfig::from_env()?;
    if !config.eval_worker_enabled {
        tracing::info!("eval worker disabled by EVAL_WORKER_ENABLED");
        return Ok(());
    }

    let db = init_pool(&config).await?;
    run_eval_worker_runtime(
        db,
        eval_rabbitmq_from_config(&config),
        config.eval_worker_id,
        config.eval_task_timeout_seconds,
    )
    .await?;

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
