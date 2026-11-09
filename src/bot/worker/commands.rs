use super::admins;
use super::menu;
use super::proposals;
use super::reports;
use super::utils;
use crate::bot::worker::WorkerState;
use crate::hashing::hash_user_id;
use crate::l10n;
use crate::locales::Locale;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};

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
        "blacklist" => handle_blacklist(bot, msg, state).await,
        "pardon" => handle_pardon(bot, msg, state, args).await,
        "reply" => handle_reply_command(bot, msg, state, args).await,
        "addreplies" => handle_add_replies(bot, msg, state, args).await,
        "reports" => handle_reports(bot, msg, state).await,
        "audit" => handle_audit(bot, msg, state).await,
        "lang" => handle_set_language(bot, msg, state, args).await,
        "cancel" => handle_cancel(bot, msg, state).await,
        "nopostme" => handle_nopostme(bot, msg, state).await,
        "allowpostme" => handle_allowpostme(bot, msg, state).await,
        _ => Ok(()),
    }
}

async fn handle_cancel(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    match state.user_states.remove(&user_id) {
        Some((_, entry)) => {
            if entry.state == "reason" || entry.state == "ban_reason" {
                // Drop the rejected proposal, same as the "Next" button.
                state.proposals.discard_rejected(entry.proposal_id).await;
            }
            bot.send_message(msg.chat.id, l10n!(lang, "mode_cancelled"))
                .await?;
        }
        None => {
            bot.send_message(msg.chat.id, l10n!(lang, "no_active_mode"))
                .await?;
        }
    }
    Ok(())
}

async fn handle_set_language(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    if !utils::is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, l10n!(lang, "no_access"))
            .await?;
        return Ok(());
    }
    match Locale::parse(args.trim()) {
        Some(new_lang) => {
            state.db.set_language(state.bot_id, new_lang).await?;
            let mut cfg = state.config.write().await;
            cfg.lang = new_lang.as_str().to_string();
            drop(cfg);
            bot.send_message(msg.chat.id, l10n!(new_lang, "lang_updated"))
                .await?;
        }
        None => {
            // Offer the language picker instead of demanding a command argument.
            let kb = InlineKeyboardMarkup::new(vec![
                vec![
                    InlineKeyboardButton::callback("🇷🇺 Русский", "setlang_ru"),
                    InlineKeyboardButton::callback("🇬🇧 English", "setlang_en"),
                ],
                vec![InlineKeyboardButton::callback(
                    l10n!(lang, "back_to_menu_btn"),
                    "menu_main",
                )],
            ]);
            bot.send_message(msg.chat.id, l10n!(lang, "lang_choose_title"))
                .reply_markup(kb)
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

    if state
        .db
        .is_banned(state.bot_id, &hash_user_id(user_id))
        .await?
    {
        bot.send_message(chat_id, l10n!(lang, "user_banned"))
            .await?;
        return Ok(());
    }

    if let Some(parent_id_str) = args.strip_prefix("reply_") {
        if let Ok(channel_msg_id) = parent_id_str.parse::<i32>() {
            if channel_msg_id > 0 {
                return proposals::enter_reply_mode(
                    bot,
                    chat_id,
                    user_id,
                    state,
                    channel_msg_id as i64,
                    lang,
                )
                .await;
            }
        }
        bot.send_message(chat_id, l10n!(lang, "invalid_reply_link"))
            .await?;
        return Ok(());
    }

    if let Some(parent_id_str) = args.strip_prefix("report_") {
        if let Ok(channel_msg_id) = parent_id_str.parse::<i32>() {
            if channel_msg_id > 0 {
                return reports::enter_report_mode(
                    bot,
                    chat_id,
                    user_id,
                    state,
                    channel_msg_id as i64,
                    lang,
                )
                .await;
            }
        }
        bot.send_message(chat_id, l10n!(lang, "invalid_reply_link"))
            .await?;
        return Ok(());
    }

    if state.client_tg_id == user_id || utils::is_authorized(state, user_id).await? {
        // Button panel (owner / moderator / frozen moderator variants).
        menu::show_main_menu(bot, chat_id, user_id, state, lang).await?;
    } else {
        let kb = InlineKeyboardMarkup::new(vec![
            vec![InlineKeyboardButton::callback(
                l10n!(lang, "user_nopostme_btn"),
                "menu_nopostme",
            )],
            vec![InlineKeyboardButton::callback(
                l10n!(lang, "user_allowpostme_btn"),
                "menu_allowpostme",
            )],
        ]);
        bot.send_message(chat_id, l10n!(lang, "welcome"))
            .reply_markup(kb)
            .await?;
    }
    Ok(())
}

async fn handle_reply_command(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    if !menu::ensure_moderator(bot, msg.chat.id, user_id, state, lang).await? {
        return Ok(());
    }
    let channel_msg_id: i32 = match args.trim().parse() {
        Ok(id) if id > 0 => id,
        _ => {
            bot.send_message(msg.chat.id, l10n!(lang, "reply_usage"))
                .await?;
            return Ok(());
        }
    };
    proposals::enter_reply_mode(
        bot,
        msg.chat.id,
        user_id,
        state,
        channel_msg_id as i64,
        lang,
    )
    .await
}

async fn handle_proposals(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    menu::open_proposals(bot, msg.chat.id, user_id, state).await
}

async fn handle_reports(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    menu::open_reports(bot, msg.chat.id, user_id, state).await
}

async fn handle_audit(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    if user_id != state.client_tg_id {
        bot.send_message(msg.chat.id, l10n!(lang, "no_access"))
            .await?;
        return Ok(());
    }
    let (text, kb) = menu::build_audit(state, lang).await;
    bot.send_message(msg.chat.id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await?;
    Ok(())
}

async fn handle_blacklist(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;

    if user_id != state.client_tg_id {
        bot.send_message(msg.chat.id, l10n!(lang, "no_access"))
            .await?;
        return Ok(());
    }

    let (text, kb) = menu::build_blacklist(state, lang).await;
    bot.send_message(msg.chat.id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await?;
    Ok(())
}

async fn handle_pardon(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    if !menu::ensure_moderator(bot, msg.chat.id, user_id, state, lang).await? {
        return Ok(());
    }
    let ban_id = args.trim();
    if ban_id.is_empty() {
        // No BAN-ID given: open the merged "Banned users" menu instead of
        // demanding a code - every ban there has its own one-tap unban button.
        let (text, kb) = menu::build_banned_list(state, lang).await;
        bot.send_message(msg.chat.id, text)
            .parse_mode(ParseMode::Html)
            .reply_markup(kb)
            .await?;
        return Ok(());
    }
    menu::pardon_core(bot, msg.chat.id, user_id, ban_id, state, lang).await
}

async fn handle_banned(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    if !menu::ensure_moderator(bot, msg.chat.id, user_id, state, lang).await? {
        return Ok(());
    }
    let (text, kb) = menu::build_banned_list(state, lang).await;
    bot.send_message(msg.chat.id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await?;
    Ok(())
}

async fn handle_add_replies(bot: &Bot, msg: &Message, state: &WorkerState, args: &str) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    if !menu::ensure_moderator(bot, msg.chat.id, user_id, state, lang).await? {
        return Ok(());
    }
    let link = args.trim();
    if link.is_empty() {
        bot.send_message(msg.chat.id, l10n!(lang, "addreplies_usage"))
            .await?;
        return Ok(());
    }
    menu::add_replies_core(bot, msg.chat.id, user_id, link, state, lang).await
}

async fn handle_nopostme(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let lang = state.db.get_language(state.bot_id).await?;
    let username = match msg.from.as_ref().and_then(|u| u.username.clone()) {
        Some(u) => u,
        None => {
            bot.send_message(msg.chat.id, l10n!(lang, "nopostme_no_username"))
                .await?;
            return Ok(());
        }
    };
    state
        .db
        .add_banned_username(state.bot_id, &username)
        .await?;
    bot.send_message(
        msg.chat.id,
        l10n!(lang, "nopostme_success", username = username),
    )
    .await?;
    Ok(())
}

async fn handle_allowpostme(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let lang = state.db.get_language(state.bot_id).await?;
    let username = match msg.from.as_ref().and_then(|u| u.username.clone()) {
        Some(u) => u,
        None => {
            bot.send_message(msg.chat.id, l10n!(lang, "allowpostme_no_username"))
                .await?;
            return Ok(());
        }
    };
    let removed = state
        .db
        .remove_banned_username(state.bot_id, &username)
        .await?;
    if removed {
        bot.send_message(
            msg.chat.id,
            l10n!(lang, "allowpostme_success", username = username),
        )
        .await?;
    } else {
        bot.send_message(msg.chat.id, l10n!(lang, "allowpostme_not_found"))
            .await?;
    }
    Ok(())
}
