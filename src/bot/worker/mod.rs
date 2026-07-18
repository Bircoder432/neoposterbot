use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::db::repository::Database;

mod admins;
mod callbacks;
mod commands;
mod proposals;
mod setup;
mod utils;

type R = Result<()>;
type ResponseResult = std::result::Result<(), teloxide::RequestError>;

#[derive(Clone)]
pub struct WorkerState {
    pub bot_id: i32,
    pub client_tg_id: i64,
    pub config: Arc<RwLock<crate::db::models::BotConfig>>,
    pub db: Database,
    pub master_config: Arc<Config>,
}

pub async fn run_worker_bot(
    bot: Bot,
    bot_config: crate::db::models::BotConfig,
    db: Database,
    master_config: Arc<Config>,
) -> R {
    let bot_id = bot_config.id;
    let client_tg_id = bot_config.client_tg_id.unwrap_or(0);

    let state = WorkerState {
        bot_id,
        client_tg_id,
        config: Arc::new(RwLock::new(bot_config)),
        db,
        master_config,
    };

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handle_message))
        .branch(Update::filter_channel_post().endpoint(handle_channel_post))
        .branch(Update::filter_callback_query().endpoint(handle_callback));

    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .build();

    dispatcher.dispatch().await;
    Ok(())
}

async fn handle_message(bot: Bot, msg: Message, state: WorkerState) -> ResponseResult {
    if let Err(e) = dispatch_message(&bot, &msg, &state).await {
        tracing::error!(error = %e, "Worker bot message handler error");
    }
    Ok(())
}

async fn handle_callback(bot: Bot, q: CallbackQuery, state: WorkerState) -> ResponseResult {
    let _ = state
        .db
        .check_plan_limits(state.bot_id, state.client_tg_id)
        .await;
    if let Err(e) = callbacks::handle_callback_query(&bot, &q, &state).await {
        tracing::error!(error = %e, "Worker bot callback handler error");
    }
    Ok(())
}

async fn handle_channel_post(bot: Bot, post: Message, state: WorkerState) -> ResponseResult {
    if let Err(e) = setup::process_channel_post(&bot, &post, &state).await {
        tracing::error!(error = %e, "Channel post handler error");
    }
    Ok(())
}

async fn dispatch_message(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let Some(from) = &msg.from else {
        return Ok(());
    };
    let user_id = from.id.0 as i64;
    let bot_id = state.bot_id;

    let _ = state.db.check_plan_limits(bot_id, state.client_tg_id).await;

    let setup_complete = state.db.is_setup_complete(bot_id).await?;

    if !setup_complete {
        if user_id != state.client_tg_id {
            return Ok(());
        }
        return setup::handle_setup(bot, msg, state).await;
    }

    if msg.text().is_some_and(|t| t.starts_with('/')) {
        return commands::dispatch_command(bot, msg, state).await;
    }

    if let Some(us) = state.db.get_user_state(bot_id, user_id).await? {
        match us.state.as_str() {
            "reply_mode" => {
                return proposals::handle_reply_content(bot, msg, state, us.temp_target_id).await;
            }
            "reason" => {
                return proposals::handle_send_reason(bot, msg, state, us.temp_target_id).await;
            }
            "ban_reason" => {
                return proposals::handle_send_ban_reason(bot, msg, state, us.temp_target_id).await;
            }
            _ => {}
        }
    }

    proposals::handle_proposal(bot, msg, state).await
}
