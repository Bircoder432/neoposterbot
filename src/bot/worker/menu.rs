use super::proposals;
use super::reports;
use super::utils;
use crate::bot::media::{self, escape_html};
use crate::bot::worker::{UserStateEntry, WorkerState};
use crate::l10n;
use crate::locales::Locale;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, MessageId, ParseMode};

type R = anyhow::Result<()>;

fn btn(lang: Locale, key: &str, data: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(l10n!(lang, key), data)
}

fn back_to_menu_kb(lang: Locale) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![btn(lang, "back_to_menu_btn", "menu_main")]])
}

/// Role-aware main menu: owner panel, frozen moderator panel or moderator panel.
pub(super) async fn build_main_menu(
    state: &WorkerState,
    user_id: i64,
    lang: Locale,
) -> (String, InlineKeyboardMarkup) {
    if user_id == state.client_tg_id {
        let text = l10n!(lang, "menu_owner_title");
        let kb = InlineKeyboardMarkup::new(vec![
            vec![
                btn(lang, "menu_btn_proposals", "menu_proposals"),
                btn(lang, "menu_btn_reports", "menu_reports"),
            ],
            vec![
                btn(lang, "menu_btn_admins", "menu_admins"),
                btn(lang, "menu_btn_addadmin", "menu_addadmin"),
            ],
            // Merged menu: the ban list itself carries an unban button per ban,
            // so a separate "pardon by code" entry is no longer needed.
            vec![btn(lang, "menu_btn_banned", "menu_banned")],
            vec![
                btn(lang, "menu_btn_blacklist", "menu_blacklist"),
                btn(lang, "menu_btn_audit", "menu_audit"),
            ],
            vec![
                btn(lang, "menu_btn_reply", "menu_reply"),
                btn(lang, "menu_btn_addreplies", "menu_addreplies"),
            ],
            vec![
                btn(lang, "menu_btn_lang", "menu_lang"),
                btn(lang, "menu_btn_notif", "menu_notif"),
            ],
        ]);
        (text, kb)
    } else if matches!(
        state.db.get_admin_status(state.bot_id, user_id).await,
        Ok(Some(true))
    ) {
        // Frozen moderators may only switch the language.
        let text = l10n!(lang, "mod_frozen_panel");
        let kb = InlineKeyboardMarkup::new(vec![vec![btn(lang, "menu_btn_lang", "menu_lang")]]);
        (text, kb)
    } else {
        let text = l10n!(lang, "menu_mod_title");
        let kb = InlineKeyboardMarkup::new(vec![
            vec![
                btn(lang, "menu_btn_proposals", "menu_proposals"),
                btn(lang, "menu_btn_reports", "menu_reports"),
            ],
            // Merged menu: unban buttons live right on the ban list.
            vec![btn(lang, "menu_btn_banned", "menu_banned")],
            vec![
                btn(lang, "menu_btn_reply", "menu_reply"),
                btn(lang, "menu_btn_addreplies", "menu_addreplies"),
            ],
            vec![
                btn(lang, "menu_btn_lang", "menu_lang"),
                btn(lang, "menu_btn_notif", "menu_notif"),
            ],
        ]);
        (text, kb)
    }
}

pub(super) async fn show_main_menu(
    bot: &Bot,
    chat_id: ChatId,
    user_id: i64,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let (text, kb) = build_main_menu(state, user_id, lang).await;
    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await?;
    Ok(())
}

/// Common guard for moderation actions: checks the frozen state first, then
/// general access. Sends the error message to `chat_id` itself and returns
/// `false` when the action must not proceed.
pub(super) async fn ensure_moderator(
    bot: &Bot,
    chat_id: ChatId,
    user_id: i64,
    state: &WorkerState,
    lang: Locale,
) -> anyhow::Result<bool> {
    if let Some(true) = state.db.get_admin_status(state.bot_id, user_id).await? {
        bot.send_message(chat_id, l10n!(lang, "mod_frozen_action"))
            .await?;
        return Ok(false);
    }
    if !utils::is_authorized(state, user_id).await? {
        bot.send_message(chat_id, l10n!(lang, "no_access")).await?;
        return Ok(false);
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// Button equivalents of commands
// ---------------------------------------------------------------------------

pub(super) async fn open_proposals(
    bot: &Bot,
    chat_id: ChatId,
    user_id: i64,
    state: &WorkerState,
) -> R {
    let lang = state.db.get_language(state.bot_id).await?;
    if !ensure_moderator(bot, chat_id, user_id, state, lang).await? {
        return Ok(());
    }
    proposals::show_next_proposal(bot, chat_id, state, lang).await
}

pub(super) async fn open_reports(bot: &Bot, chat_id: ChatId, user_id: i64, state: &WorkerState) -> R {
    let lang = state.db.get_language(state.bot_id).await?;
    if !ensure_moderator(bot, chat_id, user_id, state, lang).await? {
        return Ok(());
    }
    reports::show_next_report(bot, chat_id, state, lang).await
}

/// Active bans list with a per-ban "pardon" inline button.
/// This is the merged "Banned users" menu: every entry carries its own
/// one-tap unban button, so no separate pardon flow is needed.
pub(super) async fn build_banned_list(state: &WorkerState, lang: Locale) -> (String, InlineKeyboardMarkup) {
    let records = state
        .db
        .get_active_ban_records(state.bot_id)
        .await
        .unwrap_or_default();
    if records.is_empty() {
        return (l10n!(lang, "no_active_bans"), back_to_menu_kb(lang));
    }
    let mut text = l10n!(lang, "active_bans_header");
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for (i, record) in records.iter().enumerate() {
        text.push_str(&l10n!(
            lang,
            "ban_record_entry",
            i = i + 1,
            ban_id = record.ban_id.clone(),
            reason = escape_html(&record.reason),
            date = record.created_at.format("%d.%m.%Y %H:%M").to_string()
        ));
        rows.push(vec![InlineKeyboardButton::callback(
            l10n!(lang, "pardon_ban_btn", ban_id = record.ban_id.clone()),
            format!("pardon_{}", record.ban_id),
        )]);
    }
    rows.push(vec![btn(lang, "back_to_menu_btn", "menu_main")]);
    (text, InlineKeyboardMarkup::new(rows))
}

/// Username blacklist with a per-entry remove button.
pub(super) async fn build_blacklist(state: &WorkerState, lang: Locale) -> (String, InlineKeyboardMarkup) {
    let usernames = state
        .db
        .get_banned_usernames(state.bot_id)
        .await
        .unwrap_or_default();
    if usernames.is_empty() {
        return (l10n!(lang, "blacklist_empty"), back_to_menu_kb(lang));
    }
    let mut text = l10n!(lang, "blacklist_title");
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for uname in &usernames {
        text.push_str(&l10n!(lang, "blacklist_entry", username = uname.clone()));
        rows.push(vec![InlineKeyboardButton::callback(
            format!("🗑 @{uname}"),
            format!("unblacklist_{uname}"),
        )]);
    }
    rows.push(vec![btn(lang, "back_to_menu_btn", "menu_main")]);
    (text, InlineKeyboardMarkup::new(rows))
}

/// Audit log (last 20 actions) with a back-to-menu button.
pub(super) async fn build_audit(state: &WorkerState, lang: Locale) -> (String, InlineKeyboardMarkup) {
    let logs = state
        .db
        .get_audit_logs(state.bot_id, 20)
        .await
        .unwrap_or_default();
    if logs.is_empty() {
        return (l10n!(lang, "audit_empty"), back_to_menu_kb(lang));
    }
    let mut text = l10n!(lang, "audit_title");
    for (i, log) in logs.iter().enumerate() {
        let date_str = log.created_at.format("%d.%m.%Y %H:%M").to_string();
        let action_key = match log.action.as_str() {
            "approve" => "audit_action_approve",
            "reject" => "audit_action_reject",
            "ban" => "audit_action_ban",
            "pardon" => "audit_action_pardon",
            "freeze" => "audit_action_freeze",
            "unfreeze" => "audit_action_unfreeze",
            "add_admin" => "audit_action_add_admin",
            "remove_admin" => "audit_action_remove_admin",
            "dismiss_report" => "audit_action_dismiss_report",
            "ban_reporter" => "audit_action_ban_reporter",
            "delete_post" => "audit_action_delete_post",
            _ => "",
        };
        let action_text = if action_key.is_empty() {
            escape_html(&log.action)
        } else {
            l10n!(lang, action_key)
        };
        text.push_str(&l10n!(
            lang,
            "audit_entry",
            i = i + 1,
            date = date_str,
            admin = escape_html(&log.admin_name),
            action = action_text,
            target = escape_html(&log.target)
        ));
    }
    (text, back_to_menu_kb(lang))
}

/// Personal notification settings with on/off toggles.
pub(super) async fn build_notif_settings(
    state: &WorkerState,
    user_id: i64,
    lang: Locale,
) -> (String, InlineKeyboardMarkup) {
    let (p_on, r_on) = state
        .db
        .get_notification_settings(state.bot_id, user_id)
        .await
        .unwrap_or((true, true));
    let p_key = if p_on {
        "notif_proposals_on"
    } else {
        "notif_proposals_off"
    };
    let r_key = if r_on {
        "notif_reports_on"
    } else {
        "notif_reports_off"
    };
    let kb = InlineKeyboardMarkup::new(vec![
        vec![btn(lang, p_key, "notif_toggle_proposals")],
        vec![btn(lang, r_key, "notif_toggle_reports")],
        vec![btn(lang, "back_to_menu_btn", "menu_main")],
    ]);
    (l10n!(lang, "notif_settings_title"), kb)
}

// ---------------------------------------------------------------------------
// Input flows (button equivalents of commands with arguments)
// ---------------------------------------------------------------------------

async fn send_input_prompt(bot: &Bot, chat_id: ChatId, lang: Locale, key: &str) -> R {
    let kb = InlineKeyboardMarkup::new(vec![vec![btn(lang, "cancel_flow_btn", "flowcancel")]]);
    bot.send_message(chat_id, l10n!(lang, key))
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await?;
    Ok(())
}

pub(super) async fn start_reply_flow(
    bot: &Bot,
    chat_id: ChatId,
    user_id: i64,
    state: &WorkerState,
) -> R {
    let lang = state.db.get_language(state.bot_id).await?;
    if !ensure_moderator(bot, chat_id, user_id, state, lang).await? {
        return Ok(());
    }
    state.user_states.insert(
        user_id,
        UserStateEntry {
            state: "await_reply_id".to_string(),
            temp_target_id: 0,
            proposal_id: 0,
        },
    );
    send_input_prompt(bot, chat_id, lang, "reply_id_prompt").await
}

pub(super) async fn start_addreplies_flow(
    bot: &Bot,
    chat_id: ChatId,
    user_id: i64,
    state: &WorkerState,
) -> R {
    let lang = state.db.get_language(state.bot_id).await?;
    if !ensure_moderator(bot, chat_id, user_id, state, lang).await? {
        return Ok(());
    }
    state.user_states.insert(
        user_id,
        UserStateEntry {
            state: "await_addreplies".to_string(),
            temp_target_id: 0,
            proposal_id: 0,
        },
    );
    send_input_prompt(bot, chat_id, lang, "addreplies_prompt").await
}

/// Pardon a ban by its BAN-ID (shared by the /pardon command and the flow).
pub(super) async fn pardon_core(
    bot: &Bot,
    chat_id: ChatId,
    user_id: i64,
    ban_id: &str,
    state: &WorkerState,
    lang: Locale,
) -> R {
    match state.db.pardon_by_ban_id(state.bot_id, ban_id).await? {
        None => {
            bot.send_message(chat_id, l10n!(lang, "ban_not_found"))
                .await?;
        }
        Some(record) => {
            let admin_name = utils::get_admin_name(state, user_id).await;
            let _ = state
                .db
                .log_action(state.bot_id, user_id, &admin_name, "pardon", ban_id)
                .await;
            bot.send_message(
                chat_id,
                l10n!(lang, "ban_deactivated", ban_id = record.ban_id),
            )
            .await?;
        }
    }
    Ok(())
}

pub(super) async fn handle_reply_id_input(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    match msg.text().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        Some(t) => match t.parse::<i32>() {
            Ok(id) if id > 0 => {
                state.user_states.remove(&user_id);
                proposals::enter_reply_mode(bot, msg.chat.id, user_id, state, id as i64, lang)
                    .await
            }
            _ => {
                bot.send_message(msg.chat.id, l10n!(lang, "reply_id_invalid"))
                    .await?;
                Ok(())
            }
        },
        None => {
            send_input_prompt(bot, msg.chat.id, lang, "reply_id_prompt").await?;
            Ok(())
        }
    }
}

pub(super) async fn handle_addreplies_input(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    let link = match msg.text().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        Some(t) => t.to_string(),
        None => {
            send_input_prompt(bot, msg.chat.id, lang, "addreplies_prompt").await?;
            return Ok(());
        }
    };
    state.user_states.remove(&user_id);
    add_replies_core(bot, msg.chat.id, user_id, &link, state, lang).await
}

/// Shared core of /addreplies and the "reply buttons" flow: attaches the
/// reply/report deep links to a channel post.
pub(super) async fn add_replies_core(
    bot: &Bot,
    chat_id: ChatId,
    user_id: i64,
    link: &str,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let channel_msg_id = match utils::parse_post_link(link) {
        Some(id) if id > 0 => id,
        _ => {
            bot.send_message(chat_id, l10n!(lang, "addreplies_invalid_link"))
                .await?;
            return Ok(());
        }
    };
    let cfg = state.config.read().await;
    let channel_id = cfg.channel_id;
    let bot_username = cfg.bot_username.clone();
    drop(cfg);
    let forwarded = bot
        .forward_message(ChatId(user_id), ChatId(channel_id), MessageId(channel_msg_id))
        .await;
    let forwarded = match forwarded {
        Ok(m) => m,
        Err(e) => {
            bot.send_message(
                chat_id,
                l10n!(lang, "addreplies_failed_get", e = e.to_string()),
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
    let has_reply = current_text.contains("start=reply_");
    let has_report = current_text.contains("start=report_");
    if has_reply && has_report {
        bot.send_message(chat_id, l10n!(lang, "addreplies_already_exists"))
            .await?;
        return Ok(());
    }
    let mut new_text = current_text;
    if !has_reply {
        let reply_text = l10n!(lang, "reply_link_text");
        let reply_link = format!(
            "\n<a href=\"https://t.me/{bot_username}?start=reply_{channel_msg_id}\">{reply_text}</a>"
        );
        new_text.push_str(&reply_link);
    }
    if !has_report {
        let report_text = l10n!(lang, "report_link_text");
        let report_link = format!(
            " | <a href=\"https://t.me/{bot_username}?start=report_{channel_msg_id}\">{report_text}</a>"
        );
        new_text.push_str(&report_link);
    }
    match media::add_reply_to_channel_post(bot, channel_id, channel_msg_id, &new_text, &forwarded)
        .await
    {
        Ok(()) => {
            bot.send_message(chat_id, l10n!(lang, "addreplies_success"))
                .await?;
        }
        Err(e) => {
            bot.send_message(
                chat_id,
                l10n!(lang, "addreplies_failed_edit", e = e.to_string()),
            )
            .await?;
        }
    }
    Ok(())
}
