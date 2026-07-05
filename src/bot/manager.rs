use super::worker::run_worker_bot;
use crate::config::Config;
use crate::db::repository::Database;
use std::collections::HashMap;
use std::sync::Arc;
use teloxide::Bot;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

pub struct BotManager {
    pub db: Database,
    pub config: Arc<Config>,
    pub workers: RwLock<HashMap<i32, JoinHandle<()>>>,
}

impl BotManager {
    pub fn new(db: Database, config: Arc<Config>) -> Arc<Self> {
        Arc::new(Self {
            db,
            config,
            workers: RwLock::new(HashMap::new()),
        })
    }

    pub async fn start_all(&self) -> anyhow::Result<()> {
        let bots = self.db.get_all_bots().await?;
        for bot in bots {
            self.start_worker(bot).await;
        }
        Ok(())
    }

    pub async fn start_worker(&self, bot_config: crate::db::models::BotConfig) {
        let mut workers = self.workers.write().await;
        if workers.contains_key(&bot_config.id) {
            return;
        }

        // Сохраняем ID до того, как переменная будет перемещена в замыкание
        let bot_id = bot_config.id;

        let bot = Bot::new(&bot_config.token);
        let db = self.db.clone();
        let config = self.config.clone();

        // Клонируем конфиг, так как оригинал будет перемещен в замыкание
        let bot_config_clone = bot_config.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = run_worker_bot(bot, bot_config_clone, db, config).await {
                tracing::error!(error = %e, "Worker bot crashed");
            }
        });

        workers.insert(bot_id, handle);
        tracing::info!("Started worker for bot_id: {}", bot_id);
    }

    pub async fn stop_worker(&self, bot_id: i32) {
        let mut workers = self.workers.write().await;
        if let Some(handle) = workers.remove(&bot_id) {
            handle.abort();
            tracing::info!("Stopped worker for bot_id: {}", bot_id);
        }
    }
}
