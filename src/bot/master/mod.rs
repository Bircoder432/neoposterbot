use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use teloxide::prelude::*;

use crate::bot::manager::BotManager;
use crate::config::Config;
use crate::db::repository::Database;
use crate::locales::Locale;

mod callbacks;
mod messages;
mod ui;

pub struct MasterState {
    pub db: Database,
    pub manager: Arc<BotManager>,
    pub config: Arc<Config>,
    pub flow_states: DashMap<i64, FlowState>,
}

#[derive(Clone)]
pub enum FlowState {
    AwaitingToken,
    AwaitingBotCheck,
}

pub async fn run_master_bot(state: Arc<MasterState>) -> Result<()> {
    let bot = Bot::new(&state.config.master_bot_token);

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handle_master_message))
        .branch(Update::filter_callback_query().endpoint(handle_master_callback))
        .branch(Update::filter_pre_checkout_query().endpoint(handle_pre_checkout));

    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build();

    dispatcher.dispatch().await;
    Ok(())
}

type ResponseResult = std::result::Result<(), teloxide::RequestError>;

async fn handle_pre_checkout(bot: Bot, q: PreCheckoutQuery) -> ResponseResult {
    if let Err(e) = process_pre_checkout(&bot, &q).await {
        tracing::error!(error = %e, "Pre-checkout error");
    }
    Ok(())
}

async fn handle_master_message(bot: Bot, msg: Message, state: Arc<MasterState>) -> ResponseResult {
    if let Err(e) = messages::process_master_message(&bot, &msg, &state).await {
        tracing::error!(error = %e, "Master message handler error");
    }
    Ok(())
}

async fn handle_master_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<MasterState>,
) -> ResponseResult {
    if let Err(e) = callbacks::process_master_callback(&bot, &q, &state).await {
        tracing::error!(error = %e, "Master callback handler error");
    }
    Ok(())
}

async fn process_pre_checkout(bot: &Bot, q: &PreCheckoutQuery) -> Result<()> {
    bot.answer_pre_checkout_query(q.id.clone(), true).await?;
    Ok(())
}

pub(super) async fn get_locale(state: &Arc<MasterState>, user_id: i64) -> Locale {
    state
        .db
        .get_client_lang(user_id)
        .await
        .unwrap_or(Locale::Ru)
}
