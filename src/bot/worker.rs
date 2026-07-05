use anyhow::{Result, anyhow};
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, MaybeInaccessibleMessage};

use crate::bot::media;
use crate::config::Config;
use crate::db::models::{NewMessage, Proposal};
use crate::db::repository::Database;
use crate::locales::{L10n, Locale};

#[derive(Clone)]
pub struct WorkerState {
    pub bot_config: crate::db::models::BotConfig,
    pub db: Database,
    pub master_config: Arc<Config>,
}

pub async fn run_worker_bot(
    bot: Bot,
    bot_config: crate::db::models::BotConfig,
    db: Database,
    master_config: Arc<Config>,
) -> Result<()> {
    let state = WorkerState {
        bot_config,
        db,
        master_config,
    };

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handle_message))
        .branch(Update::filter_callback_query().endpoint(handle_callback));

    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .build();

    dispatcher.dispatch().await;
    Ok(())
}

type R = Result<()>;
type ResponseResult = std::result::Result<(), teloxide::RequestError>;

async fn handle_message(bot: Bot, msg: Message, state: WorkerState) -> ResponseResult {
    if let Err(e) = dispatch_message(&bot, &msg, &state).await {
        tracing::error!(error = %e, "Worker bot message handler error");
    }
    Ok(())
}

async fn handle_callback(bot: Bot, q: CallbackQuery, state: WorkerState) -> ResponseResult {
    if let Err(e) = handle_callback_query(&bot, &q, &state).await {
        tracing::error!(error = %e, "Worker bot callback handler error");
    }
    Ok(())
}

async fn dispatch_message(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let Some(from) = &msg.from else {
        return Ok(());
    };
    let user_id = from.id.0 as i64;
    let bot_id = state.bot_config.id;

    if msg.text().is_some_and(|t| t.starts_with('/')) {
        return dispatch_command(bot, msg, state).await;
    }

    if let Some(us) = state.db.get_user_state(bot_id, user_id).await? {
        match us.state.as_str() {
            "reply_mode" => return handle_reply_content(bot, msg, state, us.temp_target_id).await,
            "reason" => return handle_send_reason(bot, msg, state, us.temp_target_id).await,
            "ban_reason" => {
                return handle_send_ban_reason(bot, msg, state, us.temp_target_id).await;
            }
            _ => {}
        }
    }

    handle_proposal(bot, msg, state).await
}

async fn dispatch_command(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let text = msg.text().unwrap_or("");
    let mut parts = text.splitn(2, char::is_whitespace);
    let cmd = parts
        .next()
        .unwrap_or("")
        .trim_start_matches('/')
        .to_lowercase();
    let args = parts.next().unwrap_or("").trim();

    match cmd.as_str() {
        "start" => handle_start(bot, msg, state, args).await,
        "proposals" => handle_proposals(bot, msg, state).await,
        "addadmin" => handle_add_admin(bot, msg, state, args).await,
        "removeadmin" => handle_remove_admin(bot, msg, state, args).await,
        "admins" => handle_admins(bot, msg, state).await,
        "banned" => handle_banned(bot, msg, state).await,
        "pardon" => handle_pardon(bot, msg, state, args).await,
        "reply" => handle_reply_command(bot, msg, state, args).await,
        "lang" => handle_set_language(bot, msg, state, args).await,
        _ => Ok(()),
    }
}

// --- PROPOSALS LOGIC ---
async fn handle_start(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let chat_id = msg.chat.id;
    let lang = state.db.get_language(state.bot_config.id).await?;

    if let Some(parent_id_str) = args.strip_prefix("reply_") {
        if let Ok(parent_id) = parent_id_str.parse::<i64>() {
            if state
                .db
                .get_message_by_id(state.bot_config.id, parent_id)
                .await?
                .is_some()
            {
                state
                    .db
                    .set_user_state(state.bot_config.id, user_id, "reply_mode", parent_id)
                    .await?;
                bot.send_message(chat_id, L10n::send_reply_to_post(lang))
                    .await?;
                return Ok(());
            }
        }
        bot.send_message(chat_id, L10n::invalid_reply_link(lang))
            .await?;
        return Ok(());
    }

    if state.db.is_banned(state.bot_config.id, user_id).await? {
        bot.send_message(chat_id, L10n::user_banned(lang)).await?;
        return Ok(());
    }

    // ПРОВЕРКА РОЛИ
    if state.bot_config.client_tg_id.unwrap_or(0) == user_id {
        bot.send_message(chat_id, L10n::owner_panel(lang)).await?;
    } else if state.db.is_admin(state.bot_config.id, user_id).await? {
        bot.send_message(chat_id, L10n::mod_panel(lang)).await?;
    } else {
        bot.send_message(chat_id, L10n::welcome(lang)).await?;
    }
    Ok(())
}

async fn handle_reply_command(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_config.id).await?;
    let parent_id: i64 = match args.trim().parse() {
        Ok(id) if id > 0 => id,
        _ => {
            bot.send_message(msg.chat.id, L10n::reply_usage(lang))
                .await?;
            return Ok(());
        }
    };

    if state
        .db
        .get_message_by_id(state.bot_config.id, parent_id)
        .await?
        .is_none()
    {
        bot.send_message(msg.chat.id, L10n::post_not_found(lang))
            .await?;
        return Ok(());
    }

    state
        .db
        .set_user_state(state.bot_config.id, user_id, "reply_mode", parent_id)
        .await?;
    bot.send_message(msg.chat.id, L10n::send_reply_to_post(lang))
        .await?;
    Ok(())
}

async fn handle_proposal(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let chat_id = msg.chat.id;
    let lang = state.db.get_language(state.bot_config.id).await?;

    if state.db.is_banned(state.bot_config.id, user_id).await? {
        bot.send_message(chat_id, L10n::user_banned(lang)).await?;
        return Ok(());
    }
    if msg.chat.is_group() || msg.chat.is_supergroup() {
        return Ok(());
    }
    if !has_content(msg) {
        return Ok(());
    }
    if state
        .db
        .message_exists(state.bot_config.id, chat_id.0, msg.id.0 as i32)
        .await?
    {
        return Ok(());
    }

    let (media_type, media_file_id) = media::extract_media_info(msg, lang);
    let message_text = media::extract_message_text(msg, lang);
    let media_group_id = msg.media_group_id().map(|s| s.to_string());

    let proposal_group_id = media_group_id
        .clone()
        .unwrap_or_else(|| format!("single_{}", uuid::Uuid::new_v4()));

    let new_msg = NewMessage {
        bot_id: state.bot_config.id,
        chat_id: chat_id.0,
        telegram_message_id: msg.id.0 as i32,
        sender_id: user_id,
        message_text,
        media_type,
        media_file_id,
        media_group_id,
        proposal_group_id,
        parent_message_id: None,
    };

    let inserted = state.db.save_message(&new_msg).await?;
    if inserted {
        bot.send_message(chat_id, L10n::proposal_accepted(lang))
            .await?;
        notify_admins(bot, state, &new_msg, lang).await?;
    }
    Ok(())
}

async fn handle_reply_content(bot: &Bot, msg: &Message, state: &WorkerState, parent_id: i64) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let chat_id = msg.chat.id;
    let lang = state.db.get_language(state.bot_config.id).await?;

    if state
        .db
        .message_exists(state.bot_config.id, chat_id.0, msg.id.0 as i32)
        .await?
    {
        state
            .db
            .clear_user_state(state.bot_config.id, user_id)
            .await?;
        return Ok(());
    }

    let (media_type, media_file_id) = media::extract_media_info(msg, lang);
    let message_text = media::extract_message_text(msg, lang);
    let media_group_id = msg.media_group_id().map(|s| s.to_string());

    let proposal_group_id = media_group_id
        .clone()
        .unwrap_or_else(|| format!("single_{}", uuid::Uuid::new_v4()));

    let new_msg = NewMessage {
        bot_id: state.bot_config.id,
        chat_id: chat_id.0,
        telegram_message_id: msg.id.0 as i32,
        sender_id: user_id,
        message_text,
        media_type,
        media_file_id,
        media_group_id,
        proposal_group_id,
        parent_message_id: Some(parent_id),
    };

    match state.db.save_message(&new_msg).await {
        Ok(true) => {
            state
                .db
                .clear_user_state(state.bot_config.id, user_id)
                .await?;
            bot.send_message(chat_id, L10n::reply_accepted(lang))
                .await?;
            notify_admins(bot, state, &new_msg, lang).await?;
        }
        Ok(false) => {}
        Err(e) => {
            tracing::error!(error = %e, "Failed to save reply");
            bot.send_message(chat_id, L10n::error_sending_reply(lang))
                .await?;
            state
                .db
                .clear_user_state(state.bot_config.id, user_id)
                .await?;
        }
    }
    Ok(())
}

async fn handle_send_reason(
    bot: &Bot,
    msg: &Message,
    state: &WorkerState,
    target_user_id: i64,
) -> R {
    let admin_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_config.id).await?;
    let reason = msg.text().unwrap_or("No reason provided");

    let _ = bot
        .send_message(ChatId(target_user_id), L10n::rejected_reason(lang, reason))
        .await;
    state
        .db
        .clear_user_state(state.bot_config.id, admin_id)
        .await?;
    bot.send_message(msg.chat.id, L10n::reason_sent(lang))
        .await?;
    Ok(())
}

async fn handle_send_ban_reason(
    bot: &Bot,
    msg: &Message,
    state: &WorkerState,
    target_user_id: i64,
) -> R {
    let admin_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_config.id).await?;
    let reason = msg.text().unwrap_or("No reason provided");

    match state
        .db
        .create_ban_record(state.bot_config.id, target_user_id, reason)
        .await
    {
        Ok(ban_id) => {
            state
                .db
                .ban_user(state.bot_config.id, target_user_id)
                .await?;
            state
                .db
                .clear_user_state(state.bot_config.id, admin_id)
                .await?;

            let _ = bot
                .send_message(
                    ChatId(target_user_id),
                    L10n::user_banned_appeal(lang, &ban_id),
                )
                .await;
            bot.send_message(msg.chat.id, L10n::user_banned_success(lang))
                .await?;
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to ban user");
            bot.send_message(msg.chat.id, L10n::error_banning_user(lang))
                .await?;
            state
                .db
                .clear_user_state(state.bot_config.id, admin_id)
                .await?;
        }
    }
    Ok(())
}

fn has_content(msg: &Message) -> bool {
    msg.text().is_some()
        || msg.photo().is_some()
        || msg.document().is_some()
        || msg.video().is_some()
        || msg.video_note().is_some()
        || msg.audio().is_some()
        || msg.voice().is_some()
        || msg.sticker().is_some()
}

async fn notify_admins(bot: &Bot, state: &WorkerState, msg: &NewMessage, lang: Locale) -> R {
    let admins = state.db.get_admins(state.bot_config.id).await?;
    let notification = L10n::new_proposal_notif(
        lang,
        &msg.proposal_group_id,
        &msg.message_text,
        &msg.media_type,
    );
    for admin in admins {
        let _ = bot.send_message(ChatId(admin.user_id), &notification).await;
    }
    Ok(())
}

// --- MODERATION LOGIC ---
async fn handle_proposals(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_config.id).await?;

    if !is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, L10n::no_access(lang)).await?;
        return Ok(());
    }
    show_next_proposal(bot, msg.chat.id, state, lang).await
}

async fn handle_pardon(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_config.id).await?;

    if !is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, L10n::no_access(lang)).await?;
        return Ok(());
    }

    let ban_id = args.trim();
    if ban_id.is_empty() {
        bot.send_message(msg.chat.id, L10n::pardon_usage(lang))
            .await?;
        return Ok(());
    }

    match state.db.get_ban_record(state.bot_config.id, ban_id).await? {
        None => {
            bot.send_message(msg.chat.id, L10n::ban_not_found(lang))
                .await?;
        }
        Some(record) => {
            state
                .db
                .pardon_user(state.bot_config.id, record.user_id)
                .await?;
            state.db.deactivate_ban(state.bot_config.id, ban_id).await?;
            let _ = bot
                .send_message(ChatId(record.user_id), L10n::access_restored(lang))
                .await;
            bot.send_message(msg.chat.id, L10n::ban_deactivated(lang, ban_id))
                .await?;
        }
    }
    Ok(())
}

async fn handle_callback_query(bot: &Bot, q: &CallbackQuery, state: &WorkerState) -> R {
    let user_id = q.from.id.0 as i64;
    let lang = state.db.get_language(state.bot_config.id).await?;

    if !is_authorized(state, user_id).await? {
        bot.answer_callback_query(q.id.clone())
            .text(L10n::no_access(lang))
            .await?;
        return Ok(());
    }

    let (chat_id, _message_id) = match &q.message {
        Some(MaybeInaccessibleMessage::Regular(msg)) => (msg.chat.id, msg.id),
        _ => return Ok(()),
    };

    let data = q.data.as_deref().unwrap_or("");

    if data == "next" {
        show_next_proposal(bot, chat_id, state, lang).await?;
        bot.answer_callback_query(q.id.clone())
            .text(L10n::next_btn(lang))
            .await?;
        return Ok(());
    }

    if let Some(id_str) = data.strip_prefix("approve_") {
        let id: i64 = id_str.parse().map_err(|_| anyhow!("Invalid callback id"))?;
        handle_approve(bot, chat_id, id, q, state, lang).await?;
    } else if let Some(id_str) = data.strip_prefix("reject_") {
        let id: i64 = id_str.parse().map_err(|_| anyhow!("Invalid callback id"))?;
        handle_reject(bot, chat_id, id, q, state, lang).await?;
    } else if let Some(id_str) = data.strip_prefix("reason_") {
        let sender_id: i64 = id_str.parse().map_err(|_| anyhow!("Invalid callback id"))?;
        handle_reason(bot, chat_id, sender_id, q, state, lang).await?;
    } else if let Some(id_str) = data.strip_prefix("ban_reason_") {
        let sender_id: i64 = id_str.parse().map_err(|_| anyhow!("Invalid callback id"))?;
        handle_ban_reason(bot, chat_id, sender_id, q, state, lang).await?;
    }
    Ok(())
}

async fn is_authorized(state: &WorkerState, user_id: i64) -> Result<bool> {
    Ok(state.bot_config.client_tg_id.unwrap_or(0) == user_id
        || state.db.is_admin(state.bot_config.id, user_id).await?)
}

async fn show_next_proposal(bot: &Bot, chat_id: ChatId, state: &WorkerState, lang: Locale) -> R {
    match state
        .db
        .get_next_pending_proposal(state.bot_config.id)
        .await?
    {
        None => {
            bot.send_message(chat_id, L10n::no_new_proposals(lang))
                .await?;
        }
        Some((gid,)) => {
            let messages = state
                .db
                .get_proposal_by_group_id(state.bot_config.id, &gid)
                .await?;
            let proposal = Proposal {
                group_id: gid,
                messages,
            };

            bot.send_message(
                chat_id,
                L10n::proposal_items_count(lang, proposal.messages.len()),
            )
            .await?;
            if let Err(e) = media::send_for_moderation(bot, chat_id, &proposal, lang).await {
                bot.send_message(chat_id, L10n::failed_display_media(lang, &e.to_string()))
                    .await?;
            }

            let first = proposal.first();
            let text = L10n::proposal_action_header(
                lang,
                first.id,
                &first.created_at.format("%d.%m.%Y %H:%M").to_string(),
            );
            let kb = InlineKeyboardMarkup::new(vec![vec![
                InlineKeyboardButton::callback(
                    L10n::approve_btn(lang),
                    format!("approve_{}", first.id),
                ),
                InlineKeyboardButton::callback(
                    L10n::reject_btn(lang),
                    format!("reject_{}", first.id),
                ),
            ]]);
            bot.send_message(chat_id, text).reply_markup(kb).await?;
        }
    }
    Ok(())
}

async fn handle_approve(
    bot: &Bot,
    chat_id: ChatId,
    msg_id: i64,
    q: &CallbackQuery,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let msg = state
        .db
        .get_message_by_id(state.bot_config.id, msg_id)
        .await?
        .ok_or_else(|| anyhow!("Message not found"))?;
    let group_id = msg.proposal_group_id.clone();
    let messages = state
        .db
        .get_proposal_by_group_id(state.bot_config.id, &group_id)
        .await?;
    let proposal = Proposal {
        group_id: group_id.clone(),
        messages,
    };

    let reply_to = if let Some(parent_id) = proposal.first().parent_message_id {
        match state
            .db
            .get_message_by_id(state.bot_config.id, parent_id)
            .await?
        {
            Some(parent) if parent.channel_message_id.is_some() => parent.channel_message_id,
            _ => None,
        }
    } else {
        None
    };

    // Проверяем тариф клиента
    let plan = state
        .db
        .get_client_plan_by_bot_id(state.bot_config.id)
        .await?;
    let is_pro = plan == "pro";

    match media::publish(
        bot,
        state.bot_config.channel_id,
        &proposal,
        &state.bot_config.bot_username,
        &state.master_config.watermark_username,
        is_pro, // Передаем флаг Pro-подписки
        reply_to,
        lang,
    )
    .await
    {
        Ok(Some(channel_msg_id)) => {
            state
                .db
                .update_channel_message_id(state.bot_config.id, &group_id, channel_msg_id)
                .await?;
            state
                .db
                .update_proposal_status(state.bot_config.id, &group_id, "approved")
                .await?;
            bot.answer_callback_query(q.id.clone())
                .text(L10n::published(lang))
                .await?;
            delete_callback_message(bot, chat_id, q).await?;
            show_next_proposal(bot, chat_id, state, lang).await?;
        }
        Ok(None) => {
            bot.answer_callback_query(q.id.clone())
                .text(L10n::published_no_id(lang))
                .await?;
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to publish proposal");
            bot.answer_callback_query(q.id.clone())
                .text(L10n::failed_publish(lang))
                .await?;
        }
    }
    Ok(())
}

async fn handle_reject(
    bot: &Bot,
    chat_id: ChatId,
    msg_id: i64,
    q: &CallbackQuery,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let msg = state
        .db
        .get_message_by_id(state.bot_config.id, msg_id)
        .await?
        .ok_or_else(|| anyhow!("Message not found"))?;
    let sender_id = msg.sender_id;
    let group_id = msg.proposal_group_id.clone();

    state
        .db
        .update_proposal_status(state.bot_config.id, &group_id, "rejected")
        .await?;
    state
        .db
        .delete_proposal(state.bot_config.id, &group_id)
        .await?;

    bot.answer_callback_query(q.id.clone())
        .text(L10n::rejected(lang))
        .await?;
    delete_callback_message(bot, chat_id, q).await?;

    let kb = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(L10n::reason_btn(lang), format!("reason_{sender_id}")),
        InlineKeyboardButton::callback(L10n::next_btn(lang), "next"),
        InlineKeyboardButton::callback(L10n::ban_btn(lang), format!("ban_reason_{sender_id}")),
    ]]);

    bot.send_message(chat_id, L10n::choose_action(lang))
        .reply_markup(kb)
        .await?;
    Ok(())
}

async fn handle_reason(
    bot: &Bot,
    chat_id: ChatId,
    sender_id: i64,
    q: &CallbackQuery,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let admin_id = q.from.id.0 as i64;
    bot.send_message(chat_id, L10n::enter_rejection_reason(lang))
        .await?;
    state
        .db
        .set_user_state(state.bot_config.id, admin_id, "reason", sender_id)
        .await?;
    bot.answer_callback_query(q.id.clone())
        .text(L10n::enter_reason_callback(lang))
        .await?;
    Ok(())
}

async fn handle_ban_reason(
    bot: &Bot,
    chat_id: ChatId,
    sender_id: i64,
    q: &CallbackQuery,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let admin_id = q.from.id.0 as i64;
    bot.send_message(chat_id, L10n::enter_ban_reason(lang))
        .await?;
    state
        .db
        .set_user_state(state.bot_config.id, admin_id, "ban_reason", sender_id)
        .await?;
    bot.answer_callback_query(q.id.clone())
        .text(L10n::enter_reason_callback(lang))
        .await?;
    Ok(())
}

async fn delete_callback_message(bot: &Bot, chat_id: ChatId, q: &CallbackQuery) -> R {
    if let Some(MaybeInaccessibleMessage::Regular(msg)) = &q.message {
        bot.delete_message(chat_id, msg.id).await?;
    }
    Ok(())
}

// --- ADMIN LOGIC ---
async fn handle_set_language(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_config.id).await?;

    if !is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, L10n::no_access(lang)).await?;
        return Ok(());
    }

    match Locale::parse(args.trim()) {
        Some(new_lang) => {
            state.db.set_language(state.bot_config.id, new_lang).await?;
            bot.send_message(msg.chat.id, L10n::lang_updated(new_lang))
                .await?;
        }
        None => {
            bot.send_message(msg.chat.id, L10n::lang_invalid(lang))
                .await?;
        }
    }
    Ok(())
}

async fn handle_add_admin(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_config.id).await?;

    if !is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, L10n::only_owner_add_admins(lang))
            .await?;
        return Ok(());
    }

    let target_id: i64 = match args.trim().parse() {
        Ok(id) if id > 0 => id,
        _ => {
            bot.send_message(msg.chat.id, L10n::add_admin_usage(lang))
                .await?;
            return Ok(());
        }
    };

    // ПРОВЕРКА ЛИМИТОВ ТАРИФА
    let plan = state
        .db
        .get_client_plan_by_bot_id(state.bot_config.id)
        .await?;
    let admin_count = state.db.count_admins(state.bot_config.id).await?;

    if plan == "free" && admin_count >= 1 {
        bot.send_message(msg.chat.id, L10n::free_limit_admins(lang))
            .await?;
        return Ok(());
    }

    if state.db.is_admin(state.bot_config.id, target_id).await? {
        bot.send_message(msg.chat.id, L10n::admin_already_exists(lang, target_id))
            .await?;
        return Ok(());
    }

    let user_name = resolve_user_name(bot, target_id).await;
    state
        .db
        .add_admin(state.bot_config.id, target_id, &user_name)
        .await?;
    bot.send_message(msg.chat.id, L10n::admin_added(lang, &user_name))
        .await?;
    let _ = bot
        .send_message(ChatId(target_id), L10n::admin_added_notification(lang))
        .await;
    Ok(())
}

async fn handle_remove_admin(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_config.id).await?;

    if !is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, L10n::only_owner_remove_admins(lang))
            .await?;
        return Ok(());
    }

    let target_id: i64 = match args.trim().parse() {
        Ok(id) if id > 0 => id,
        _ => {
            bot.send_message(msg.chat.id, L10n::remove_admin_usage(lang))
                .await?;
            return Ok(());
        }
    };

    if !state.db.is_admin(state.bot_config.id, target_id).await? {
        bot.send_message(msg.chat.id, L10n::admin_not_found(lang))
            .await?;
        return Ok(());
    }

    state
        .db
        .remove_admin(state.bot_config.id, target_id)
        .await?;
    bot.send_message(msg.chat.id, L10n::admin_removed(lang, target_id))
        .await?;
    let _ = bot
        .send_message(ChatId(target_id), L10n::admin_removed_notification(lang))
        .await;
    Ok(())
}

async fn handle_admins(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_config.id).await?;

    if !is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, L10n::only_owner_list_admins(lang))
            .await?;
        return Ok(());
    }

    let admins = state.db.get_admins(state.bot_config.id).await?;
    if admins.is_empty() {
        bot.send_message(msg.chat.id, L10n::no_admins(lang)).await?;
        return Ok(());
    }

    let owner_id = state.bot_config.client_tg_id.unwrap_or(0);
    let mut list = L10n::admins_list_header(lang, owner_id);
    for (i, admin) in admins.iter().enumerate() {
        if admin.user_id == owner_id {
            list.push_str(&format!(
                "{}. {} (ID: {}) 👑\n",
                i + 1,
                admin.user_name,
                admin.user_id
            ));
        } else {
            list.push_str(&format!(
                "{}. {} (ID: {})\n",
                i + 1,
                admin.user_name,
                admin.user_id
            ));
        }
    }
    bot.send_message(msg.chat.id, list).await?;
    Ok(())
}

async fn handle_banned(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_config.id).await?;

    if !is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, L10n::no_access(lang)).await?;
        return Ok(());
    }

    let records = state.db.get_active_ban_records(state.bot_config.id).await?;
    if records.is_empty() {
        bot.send_message(msg.chat.id, L10n::no_active_bans(lang))
            .await?;
        return Ok(());
    }

    let mut list = L10n::active_bans_header(lang).to_string();
    for (i, record) in records.iter().enumerate() {
        list.push_str(&L10n::ban_record_entry(
            lang,
            i + 1,
            &record.ban_id,
            &record.reason,
            &record.created_at.format("%d.%m.%Y %H:%M").to_string(),
        ));
    }
    bot.send_message(msg.chat.id, list).await?;
    Ok(())
}

async fn resolve_user_name(bot: &Bot, user_id: i64) -> String {
    match bot.get_chat(ChatId(user_id)).await {
        Ok(chat) => chat
            .username()
            .or_else(|| chat.first_name())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("user_{user_id}")),
        Err(_) => format!("user_{user_id}"),
    }
}
