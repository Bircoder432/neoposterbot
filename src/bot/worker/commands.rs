use teloxide::prelude::*;
use teloxide::types::MessageId;

use super::admins;
use super::proposals;
use super::utils;
use crate::bot::media;
use crate::bot::worker::{UserStateEntry, WorkerState};
use crate::hashing::hash_user_id;
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
        "addreplies" => handle_add_replies(bot, msg, state, args).await,
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

async fn handle_start(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let chat_id = msg.chat.id;
    let lang = state.db.get_language(state.bot_id).await?;

    if let Some(code) = args.strip_prefix("admin_") {
        return admins::handle_admin_invite(bot, chat_id, user_id, code, state, lang).await;
    }

    if let Some(parent_id_str) = args.strip_prefix("reply_") {
        if let Ok(channel_msg_id) = parent_id_str.parse::<i32>() {
            if channel_msg_id > 0 {
                state.user_states.insert(
                    user_id,
                    UserStateEntry {
                        state: "reply_mode".to_string(),
                        temp_target_id: channel_msg_id as i64,
                        proposal_id: 0,
                    },
                );
                bot.send_message(chat_id, L10n::send_reply_to_post(lang))
                    .await?;
                return Ok(());
            }
        }
        bot.send_message(chat_id, L10n::invalid_reply_link(lang))
            .await?;
        return Ok(());
    }

    if state
        .db
        .is_banned(state.bot_id, &hash_user_id(user_id))
        .await?
    {
        bot.send_message(chat_id, L10n::user_banned(lang)).await?;
        return Ok(());
    }

    if state.client_tg_id == user_id {
        bot.send_message(chat_id, L10n::owner_panel(lang)).await?;
    } else if utils::is_authorized(state, user_id).await? {
        if let Some(true) = state.db.get_admin_status(state.bot_id, user_id).await? {
            bot.send_message(chat_id, L10n::mod_frozen_panel(lang))
                .await?;
        } else {
            bot.send_message(chat_id, L10n::mod_panel(lang)).await?;
        }
    } else {
        bot.send_message(chat_id, L10n::welcome(lang)).await?;
    }
    Ok(())
}

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

    let channel_msg_id: i32 = match args.trim().parse() {
        Ok(id) if id > 0 => id,
        _ => {
            bot.send_message(msg.chat.id, L10n::reply_usage(lang))
                .await?;
            return Ok(());
        }
    };

    state.user_states.insert(
        user_id,
        UserStateEntry {
            state: "reply_mode".to_string(),
            temp_target_id: channel_msg_id as i64,
            proposal_id: 0,
        },
    );
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

    match state.db.pardon_by_ban_id(state.bot_id, ban_id).await? {
        None => {
            bot.send_message(msg.chat.id, L10n::ban_not_found(lang))
                .await?;
        }
        Some(record) => {
            // Сырой айди юзера не хранится, поэтому уведомить его невозможно.
            bot.send_message(msg.chat.id, L10n::ban_deactivated(lang, &record.ban_id))
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

fn parse_post_link(link: &str) -> Option<i32> {
    let lower = link.to_lowercase();
    let pos = lower.find("t.me/")?;
    let path = &lower[pos + 5..];
    let path = path.split('?').next().unwrap_or(path);
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 3 && parts[0] == "c" {
        parts[2].parse::<i32>().ok()
    } else if parts.len() >= 2 {
        parts[1].parse::<i32>().ok()
    } else {
        None
    }
}

async fn handle_add_replies(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
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

    let link = args.trim();
    if link.is_empty() {
        bot.send_message(msg.chat.id, L10n::addreplies_usage(lang))
            .await?;
        return Ok(());
    }

    let channel_msg_id = match parse_post_link(link) {
        Some(id) if id > 0 => id,
        _ => {
            bot.send_message(msg.chat.id, L10n::addreplies_invalid_link(lang))
                .await?;
            return Ok(());
        }
    };

    let cfg = state.config.read().await;
    let channel_id = cfg.channel_id;
    let bot_username = cfg.bot_username.clone();
    drop(cfg);

    let forwarded = bot
        .forward_message(
            ChatId(user_id),
            ChatId(channel_id),
            MessageId(channel_msg_id),
        )
        .await;

    let forwarded = match forwarded {
        Ok(m) => m,
        Err(e) => {
            bot.send_message(
                msg.chat.id,
                L10n::addreplies_failed_get(lang, &e.to_string()),
            )
            .await?;
            return Ok(());
        }
    };

    let current_text = forwarded
        .text()
        .map(|s| s.to_string())
        .or_else(|| forwarded.caption().map(|s| s.to_string()))
        .unwrap_or_default();
    let _ = bot.delete_message(ChatId(user_id), forwarded.id).await;

    if current_text.contains("start=reply_") {
        bot.send_message(msg.chat.id, L10n::addreplies_already_exists(lang))
            .await?;
        return Ok(());
    }

    let reply_text = L10n::reply_link_text(lang);
    let reply_link = format!(
        "\n<a href=\"https://t.me/{bot_username}?start=reply_{channel_msg_id}\">{reply_text}</a>"
    );
    let new_text = format!("{current_text}{reply_link}");

    match media::add_reply_to_channel_post(bot, channel_id, channel_msg_id, &new_text, &forwarded)
        .await
    {
        Ok(()) => {
            bot.send_message(msg.chat.id, L10n::addreplies_success(lang))
                .await?;
        }
        Err(e) => {
            bot.send_message(
                msg.chat.id,
                L10n::addreplies_failed_edit(lang, &e.to_string()),
            )
            .await?;
        }
    }
    Ok(())
}
