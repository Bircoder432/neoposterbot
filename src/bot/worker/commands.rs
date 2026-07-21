use teloxide::prelude::*;

use super::admins;
use super::proposals;
use super::utils;
use crate::bot::worker::WorkerState;
use crate::locales::{L10n, Locale};

type R = anyhow::Result<()>;

pub(super) async fn dispatch_command(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
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
        "addadmin" => admins::handle_add_admin(bot, msg, state).await,
        "admins" => admins::handle_admins(bot, msg, state).await,
        "banned" => handle_banned(bot, msg, state).await,
        "pardon" => handle_pardon(bot, msg, state, args).await,
        "reply" => handle_reply_command(bot, msg, state, args).await,
        "lang" => handle_set_language(bot, msg, state, args).await,
        _ => Ok(()),
    }
}

async fn handle_set_language(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;

    if !utils::is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, L10n::no_access(lang)).await?;
        return Ok(());
    }

    match Locale::parse(args.trim()) {
        Some(new_lang) => {
            state.db.set_language(state.bot_id, new_lang).await?;

            let mut cfg = state.config.write().await;
            cfg.lang = new_lang.as_str().to_string();
            drop(cfg);

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

// Замените функцию handle_start на эту:
// Замените функцию handle_start на эту:
async fn handle_start(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let chat_id = msg.chat.id;
    let lang = state.db.get_language(state.bot_id).await?;

    // ── Admin invite deeplink ──
    if let Some(code) = args.strip_prefix("admin_") {
        return admins::handle_admin_invite(bot, chat_id, user_id, code, state, lang).await;
    }

    if let Some(parent_id_str) = args.strip_prefix("reply_") {
        if let Ok(parent_id) = parent_id_str.parse::<i64>() {
            if state
                .db
                .get_message_by_id(state.bot_id, parent_id)
                .await?
                .is_some()
            {
                state
                    .db
                    .set_user_state(state.bot_id, user_id, "reply_mode", parent_id)
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

    if state.db.is_banned(state.bot_id, user_id).await? {
        bot.send_message(chat_id, L10n::user_banned(lang)).await?;
        return Ok(());
    }

    // Логика разделения панелей
    if state.client_tg_id == user_id {
        bot.send_message(chat_id, L10n::owner_panel(lang)).await?;
    } else if utils::is_authorized(state, user_id).await? {
        // Это модератор (не владелец)
        if let Some(true) = state.db.get_admin_status(state.bot_id, user_id).await? {
            bot.send_message(chat_id, L10n::mod_frozen_panel(lang))
                .await?;
        } else {
            bot.send_message(chat_id, L10n::mod_panel(lang)).await?;
        }
    } else {
        // Обычный пользователь
        bot.send_message(chat_id, L10n::welcome(lang)).await?;
    }

    Ok(())
}

// Замените функцию handle_reply_command на эту:
async fn handle_reply_command(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;

    if let Some(true) = state.db.get_admin_status(state.bot_id, user_id).await? {
        bot.send_message(msg.chat.id, L10n::mod_frozen_action(lang))
            .await?;
        return Ok(());
    }

    if !utils::is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, L10n::no_access(lang)).await?;
        return Ok(());
    }

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
        .get_message_by_id(state.bot_id, parent_id)
        .await?
        .is_none()
    {
        bot.send_message(msg.chat.id, L10n::post_not_found(lang))
            .await?;
        return Ok(());
    }

    state
        .db
        .set_user_state(state.bot_id, user_id, "reply_mode", parent_id)
        .await?;
    bot.send_message(msg.chat.id, L10n::send_reply_to_post(lang))
        .await?;
    Ok(())
}

async fn handle_proposals(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;

    if let Some(true) = state.db.get_admin_status(state.bot_id, user_id).await? {
        bot.send_message(msg.chat.id, L10n::mod_frozen_action(lang))
            .await?;
        return Ok(());
    }

    if !utils::is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, L10n::no_access(lang)).await?;
        return Ok(());
    }
    proposals::show_next_proposal(bot, msg.chat.id, state, lang).await
}

async fn handle_pardon(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;

    if let Some(true) = state.db.get_admin_status(state.bot_id, user_id).await? {
        bot.send_message(msg.chat.id, L10n::mod_frozen_action(lang))
            .await?;
        return Ok(());
    }

    if !utils::is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, L10n::no_access(lang)).await?;
        return Ok(());
    }

    let ban_id = args.trim();
    if ban_id.is_empty() {
        bot.send_message(msg.chat.id, L10n::pardon_usage(lang))
            .await?;
        return Ok(());
    }

    match state.db.get_ban_record(state.bot_id, ban_id).await? {
        None => {
            bot.send_message(msg.chat.id, L10n::ban_not_found(lang))
                .await?;
        }
        Some(record) => {
            state.db.pardon_user(state.bot_id, record.user_id).await?;
            state.db.deactivate_ban(state.bot_id, ban_id).await?;
            let _ = bot
                .send_message(ChatId(record.user_id), L10n::access_restored(lang))
                .await;
            bot.send_message(msg.chat.id, L10n::ban_deactivated(lang, ban_id))
                .await?;
        }
    }
    Ok(())
}

async fn handle_banned(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;

    if let Some(true) = state.db.get_admin_status(state.bot_id, user_id).await? {
        bot.send_message(msg.chat.id, L10n::mod_frozen_action(lang))
            .await?;
        return Ok(());
    }

    if !utils::is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, L10n::no_access(lang)).await?;
        return Ok(());
    }

    let records = state.db.get_active_ban_records(state.bot_id).await?;
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
