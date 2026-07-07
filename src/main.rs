use std::sync::Arc;

use anyhow::Context;
use posterbot_constructor::{
    bot::{manager::BotManager, master::MasterState},
    config::Config,
    db::Database,
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env().context("Failed to load configuration")?;
    let config = std::sync::Arc::new(config);

    let db = Database::connect(&config.database_url)
        .await
        .context("Failed to connect to database")?;
    db.migrate().await.context("Failed to run migrations")?;

    let manager = BotManager::new(db.clone(), config.clone());
    manager.start_all().await?;

    let state = Arc::new(MasterState {
        db,
        manager,
        config: config.clone(),
        flow_states: dashmap::DashMap::new(),
    });

    tracing::info!("Starting master bot");
    posterbot_constructor::bot::master::run_master_bot(state).await
}
