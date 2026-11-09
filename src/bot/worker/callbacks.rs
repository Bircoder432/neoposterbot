use super::admins;
use super::menu;
use super::proposals;
use super::reports;
use super::utils;
use crate::bot::media;
use crate::bot::worker::{UserStateEntry, WorkerState};
use crate::l10n;
use crate::locales::Locale;
use anyhow::{Result, anyhow};
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, MaybeInaccessibleMessage};

type R = Result<()>;

pub(super) async fn handle_callback_query(bot: &Bot, q: &CallbackQuery, state: &WorkerState) -> R {
    let user_id = q.from.id.0 as i64;
    let data = q.data.as_deref().unwrap_or("");
    let bot_id = state.bot_id;

    if data.starts_with("setup_lang_") {
        if user_id == state.client_tg_id {
            let lang = if data == "setup_lang_ru" {
                Locale::Ru
            } else {
                Locale::En
            };
            state.db.set_language(bot_id, lang).await?;
            let mut cfg = state.config.write().await;
            cfg.lang = lang.as_str().to_string();
            drop(cfg);
            let code = uuid::Uuid::new_v4().simple().to_string()[..4].to_uppercase();
            state.db.set_setup_code(bot_id, &code).await?;
            bot.answer_callback_query(q.id.clone()).text("✅").await?;
            bot.send_message(
                q.from.id,
                format!(
                    "{}\n{}\n<code>/connect {}</code>",
                    l10n!(lang, "send_command_in_your_channel"),
                    l10n!(lang, "setup_enter_channel"),
                    code
                ),
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
        }
        return Ok(());
    }

    let lang = state.db.get_language(bot_id).await?;

    if let Some(group_id) = data.strip_prefix("cancel_") {
        let canceled = state.proposals.cancel_by_group_id(group_id).await;

        if canceled {
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "proposal_canceled_msg"))
                .await?;
            if let Some(MaybeInaccessibleMessage::Regular(msg)) = &q.message {
                bot.edit_message_text(
                    msg.chat.id,
                    msg.id,
                    l10n!(lang, "proposal_canceled_success"),
                )
                .await
                .ok();
            }
        } else {
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "proposal_already_processed"))
                .await?;
            if let Some(MaybeInaccessibleMessage::Regular(msg)) = &q.message {
                bot.edit_message_text(
                    msg.chat.id,
                    msg.id,
                    l10n!(lang, "proposal_already_processed"),
                )
                .await
                .ok();
            }
        }
        return Ok(());
    }

    // ---- Cancel any active input mode (works for regular users too) ----
    if data == "flowcancel" {
        match state.user_states.remove(&user_id) {
            Some((_, entry)) => {
                if entry.state == "reason" || entry.state == "ban_reason" {
                    // Drop the rejected proposal, same as the "Next" button.
                    state.proposals.discard_rejected(entry.proposal_id).await;
                }
                bot.answer_callback_query(q.id.clone())
                    .text(l10n!(lang, "flow_cancelled_msg"))
                    .await?;
                if let Some(MaybeInaccessibleMessage::Regular(msg)) = &q.message {
                    bot.edit_message_text(
                        msg.chat.id,
                        msg.id,
                        l10n!(lang, "flow_cancelled_msg"),
                    )
                    .await
                    .ok();
                }
            }
            None => {
                bot.answer_callback_query(q.id.clone())
                    .text(l10n!(lang, "no_active_mode"))
                    .await?;
            }
        }
        return Ok(());
    }

    // ---- Personal settings buttons available to any user ----
    if data == "menu_nopostme" || data == "menu_allowpostme" {
        match q.from.username.as_deref() {
            None => {
                bot.answer_callback_query(q.id.clone())
                    .text(l10n!(lang, "nopostme_no_username"))
                    .await?;
            }
            Some(username) => {
                if data == "menu_nopostme" {
                    state.db.add_banned_username(bot_id, username).await?;
                    bot.answer_callback_query(q.id.clone())
                        .text(l10n!(lang, "nopostme_success", username = username))
                        .await?;
                } else {
                    let removed =
                        state.db.remove_banned_username(bot_id, username).await?;
                    let key = if removed {
                        "allowpostme_success"
                    } else {
                        "allowpostme_not_found"
                    };
                    bot.answer_callback_query(q.id.clone())
                        .text(l10n!(lang, key, username = username))
                        .await?;
                }
            }
        }
        return Ok(());
    }

    // ---- Language switching (owner and admins, including frozen ones) ----
    if data == "menu_lang" {
        if !utils::is_authorized(state, user_id).await? {
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "no_access"))
                .await?;
            return Ok(());
        }
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
        if let Some(MaybeInaccessibleMessage::Regular(msg)) = &q.message {
            bot.edit_message_text(msg.chat.id, msg.id, l10n!(lang, "lang_choose_title"))
                .reply_markup(kb)
                .await
                .ok();
        }
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "setlang_ru" || data == "setlang_en" {
        if !utils::is_authorized(state, user_id).await? {
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "no_access"))
                .await?;
            return Ok(());
        }
        let new_lang = if data == "setlang_ru" {
            Locale::Ru
        } else {
            Locale::En
        };
        state.db.set_language(bot_id, new_lang).await?;
        {
            let mut cfg = state.config.write().await;
            cfg.lang = new_lang.as_str().to_string();
        }
        let (text, kb) = menu::build_main_menu(state, user_id, new_lang).await;
        if let Some(MaybeInaccessibleMessage::Regular(msg)) = &q.message {
            bot.edit_message_text(msg.chat.id, msg.id, text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(kb)
                .await
                .ok();
        }
        bot.answer_callback_query(q.id.clone())
            .text(l10n!(new_lang, "lang_updated"))
            .await?;
        return Ok(());
    }

    // ---- Back to the role-aware main menu (frozen moderators allowed) ----
    if data == "menu_main" {
        if !utils::is_authorized(state, user_id).await? {
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "no_access"))
                .await?;
            return Ok(());
        }
        let (text, kb) = menu::build_main_menu(state, user_id, lang).await;
        if let Some(MaybeInaccessibleMessage::Regular(msg)) = &q.message {
            bot.edit_message_text(msg.chat.id, msg.id, text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(kb)
                .await
                .ok();
        }
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if let Some(true) = state.db.get_admin_status(bot_id, user_id).await? {
        bot.answer_callback_query(q.id.clone())
            .text(l10n!(lang, "mod_frozen_action"))
            .await?;
        return Ok(());
    }

    if !utils::is_authorized(state, user_id).await? {
        bot.answer_callback_query(q.id.clone())
            .text(l10n!(lang, "no_access"))
            .await?;
        return Ok(());
    }

    let (chat_id, message_id) = match &q.message {
        Some(MaybeInaccessibleMessage::Regular(msg)) => (msg.chat.id, msg.id),
        _ => return Ok(()),
    };

    if data.starts_with("rmadmin_")
        || data.starts_with("confirm_rmadmin_")
        || data == "cancel_rmadmin"
        || data == "back_to_admins"
        || data.starts_with("admin_manage_")
        || data.starts_with("freeze_admin_")
        || data.starts_with("unfreeze_admin_")
        || data.starts_with("revoke_invite_")
    {
        if user_id != state.client_tg_id {
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "only_owner_remove_admins"))
                .await?;
            return Ok(());
        }
        if let Some(code) = data.strip_prefix("revoke_invite_") {
            state.db.revoke_admin_invite(state.bot_id, code).await?;
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "invite_revoked_msg"))
                .await?;
            bot.edit_message_text(chat_id, message_id, l10n!(lang, "invite_revoked_msg"))
                .await
                .ok();
            return Ok(());
        }
        if let Some(id_str) = data.strip_prefix("admin_manage_") {
            let target_id: i64 = id_str.parse().unwrap_or(0);
            if target_id > 0 {
                let admins_list = state.db.get_admins(state.bot_id).await?;
                if let Some(admin) = admins_list.iter().find(|a| a.user_id == target_id) {
                    let (text, kb) = admins::build_admin_manage_inline(admin, lang);
                    bot.edit_message_text(chat_id, message_id, text)
                        .reply_markup(kb)
                        .await
                        .ok();
                }
            }
            bot.answer_callback_query(q.id.clone()).await?;
            return Ok(());
        }
        if let Some(id_str) = data.strip_prefix("freeze_admin_") {
            let target_id: i64 = id_str.parse().unwrap_or(0);
            if target_id > 0 && target_id != state.client_tg_id {
                state
                    .db
                    .set_admin_frozen(state.bot_id, target_id, true)
                    .await?;
                let admin_name = utils::get_admin_name(state, user_id).await;
                let _ = state
                    .db
                    .log_action(
                        state.bot_id,
                        user_id,
                        &admin_name,
                        "freeze",
                        &target_id.to_string(),
                    )
                    .await;
                let admins_list = state.db.get_admins(state.bot_id).await?;
                if let Some(admin) = admins_list.iter().find(|a| a.user_id == target_id) {
                    let (text, kb) = admins::build_admin_manage_inline(admin, lang);
                    bot.edit_message_text(chat_id, message_id, text)
                        .reply_markup(kb)
                        .await
                        .ok();
                }
                bot.answer_callback_query(q.id.clone())
                    .text(l10n!(lang, "admin_frozen_msg"))
                    .await?;
            }
            return Ok(());
        }
        if let Some(id_str) = data.strip_prefix("unfreeze_admin_") {
            let target_id: i64 = id_str.parse().unwrap_or(0);
            if target_id > 0 {
                let plan = state.db.get_client_plan_by_bot_id(state.bot_id).await?;
                if plan == "free" {
                    let count = state
                        .db
                        .count_active_admins(state.bot_id, state.client_tg_id)
                        .await?;
                    if count >= 1 {
                        bot.answer_callback_query(q.id.clone())
                            .text(l10n!(lang, "limit_reached_unfreeze"))
                            .await?;
                        return Ok(());
                    }
                }
                state
                    .db
                    .set_admin_frozen(state.bot_id, target_id, false)
                    .await?;
                let admin_name = utils::get_admin_name(state, user_id).await;
                let _ = state
                    .db
                    .log_action(
                        state.bot_id,
                        user_id,
                        &admin_name,
                        "unfreeze",
                        &target_id.to_string(),
                    )
                    .await;
                let admins_list = state.db.get_admins(state.bot_id).await?;
                if let Some(admin) = admins_list.iter().find(|a| a.user_id == target_id) {
                    let (text, kb) = admins::build_admin_manage_inline(admin, lang);
                    bot.edit_message_text(chat_id, message_id, text)
                        .reply_markup(kb)
                        .await
                        .ok();
                }
                bot.answer_callback_query(q.id.clone())
                    .text(l10n!(lang, "admin_unfrozen_msg"))
                    .await?;
            }
            return Ok(());
        }
        if let Some(id_str) = data.strip_prefix("rmadmin_") {
            let target_id: i64 = id_str.parse().unwrap_or(0);
            if target_id > 0 {
                let admins_list = state.db.get_admins(state.bot_id).await?;
                if let Some(admin) = admins_list.iter().find(|a| a.user_id == target_id) {
                    let kb = InlineKeyboardMarkup::new(vec![
                        vec![InlineKeyboardButton::callback(
                            l10n!(lang, "confirm_yes"),
                            format!("confirm_rmadmin_{}", target_id),
                        )],
                        vec![InlineKeyboardButton::callback(
                            l10n!(lang, "confirm_no"),
                            "cancel_rmadmin",
                        )],
                    ]);
                    let text = l10n!(
                        lang,
                        "confirm_remove_admin",
                        name = admin.user_name,
                        id = target_id
                    );
                    bot.edit_message_text(chat_id, message_id, text)
                        .reply_markup(kb)
                        .await
                        .ok();
                }
            }
            bot.answer_callback_query(q.id.clone()).await?;
            return Ok(());
        }
        if let Some(id_str) = data.strip_prefix("confirm_rmadmin_") {
            let target_id: i64 = id_str.parse().unwrap_or(0);
            if target_id > 0 && target_id != state.client_tg_id {
                state.db.remove_admin(state.bot_id, target_id).await?;
                let admin_name = utils::get_admin_name(state, user_id).await;
                let _ = state
                    .db
                    .log_action(
                        state.bot_id,
                        user_id,
                        &admin_name,
                        "remove_admin",
                        &target_id.to_string(),
                    )
                    .await;
                let _ = bot
                    .send_message(ChatId(target_id), l10n!(lang, "admin_removed_notification"))
                    .await;
                let (text, markup) =
                    admins::build_admins_inline(&state.db, state.bot_id, state.client_tg_id, lang)
                        .await;
                bot.edit_message_text(chat_id, message_id, text)
                    .reply_markup(markup)
                    .await
                    .ok();
            }
            bot.answer_callback_query(q.id.clone()).await?;
            return Ok(());
        }
        if data == "cancel_rmadmin" || data == "back_to_admins" {
            let (text, markup) =
                admins::build_admins_inline(&state.db, state.bot_id, state.client_tg_id, lang)
                    .await;
            bot.edit_message_text(chat_id, message_id, text)
                .reply_markup(markup)
                .await
                .ok();
            bot.answer_callback_query(q.id.clone()).await?;
            return Ok(());
        }
    }

    // ---- Owner-only menu actions ----
    if data == "menu_addadmin"
        || data == "menu_admins"
        || data == "menu_blacklist"
        || data == "menu_audit"
        || data.starts_with("unblacklist_")
    {
        if user_id != state.client_tg_id {
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "no_access"))
                .await?;
            return Ok(());
        }
        if data == "menu_addadmin" {
            bot.answer_callback_query(q.id.clone()).await?;
            admins::create_admin_invite(bot, chat_id, state, lang).await?;
            return Ok(());
        }
        if data == "menu_admins" {
            let (text, markup) =
                admins::build_admins_inline(&state.db, state.bot_id, state.client_tg_id, lang)
                    .await;
            bot.edit_message_text(chat_id, message_id, text)
                .reply_markup(markup)
                .await
                .ok();
            bot.answer_callback_query(q.id.clone()).await?;
            return Ok(());
        }
        if data == "menu_blacklist" {
            let (text, markup) = menu::build_blacklist(state, lang).await;
            bot.edit_message_text(chat_id, message_id, text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(markup)
                .await
                .ok();
            bot.answer_callback_query(q.id.clone()).await?;
            return Ok(());
        }
        if let Some(username) = data.strip_prefix("unblacklist_") {
            let removed = state.db.remove_banned_username(bot_id, username).await?;
            let (text, markup) = menu::build_blacklist(state, lang).await;
            bot.edit_message_text(chat_id, message_id, text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(markup)
                .await
                .ok();
            let key = if removed {
                "allowpostme_success"
            } else {
                "allowpostme_not_found"
            };
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, key, username = username))
                .await?;
            return Ok(());
        }
        if data == "menu_audit" {
            let (text, markup) = menu::build_audit(state, lang).await;
            bot.edit_message_text(chat_id, message_id, text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(markup)
                .await
                .ok();
            bot.answer_callback_query(q.id.clone()).await?;
            return Ok(());
        }
    }

    // ---- Common menu actions (owner + moderators) ----
    if data == "menu_notif" {
        let (text, kb) = menu::build_notif_settings(state, user_id, lang).await;
        bot.edit_message_text(chat_id, message_id, text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(kb)
            .await
            .ok();
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "notif_toggle_proposals" || data == "notif_toggle_reports" {
        let toggle_proposals = data == "notif_toggle_proposals";
        let (p_on, r_on) = state
            .db
            .get_notification_settings(bot_id, user_id)
            .await
            .unwrap_or((true, true));
        let new_value = if toggle_proposals { !p_on } else { !r_on };
        if toggle_proposals {
            state
                .db
                .set_proposals_notifications(bot_id, user_id, new_value)
                .await?;
        } else {
            state
                .db
                .set_reports_notifications(bot_id, user_id, new_value)
                .await?;
        }
        let (text, kb) = menu::build_notif_settings(state, user_id, lang).await;
        bot.edit_message_text(chat_id, message_id, text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(kb)
            .await
            .ok();
        let toast = if new_value {
            "notif_toggled_on"
        } else {
            "notif_toggled_off"
        };
        bot.answer_callback_query(q.id.clone())
            .text(l10n!(lang, toast))
            .await?;
        return Ok(());
    }

    if data == "menu_proposals" {
        bot.answer_callback_query(q.id.clone()).await?;
        menu::open_proposals(bot, chat_id, user_id, state).await?;
        return Ok(());
    }

    if data == "menu_reports" {
        bot.answer_callback_query(q.id.clone()).await?;
        menu::open_reports(bot, chat_id, user_id, state).await?;
        return Ok(());
    }

    if data == "menu_banned" {
        let (text, kb) = menu::build_banned_list(state, lang).await;
        bot.edit_message_text(chat_id, message_id, text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(kb)
            .await
            .ok();
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if let Some(ban_id) = data.strip_prefix("pardon_") {
        match state.db.pardon_by_ban_id(bot_id, ban_id).await? {
            Some(record) => {
                let admin_name = utils::get_admin_name(state, user_id).await;
                let _ = state
                    .db
                    .log_action(bot_id, user_id, &admin_name, "pardon", ban_id)
                    .await;
                let (text, kb) = menu::build_banned_list(state, lang).await;
                bot.edit_message_text(chat_id, message_id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .reply_markup(kb)
                    .await
                    .ok();
                bot.answer_callback_query(q.id.clone())
                    .text(l10n!(lang, "ban_deactivated", ban_id = record.ban_id))
                    .await?;
            }
            None => {
                bot.answer_callback_query(q.id.clone())
                    .text(l10n!(lang, "ban_not_found"))
                    .await?;
            }
        }
        return Ok(());
    }

    if data == "menu_pardon" {
        bot.answer_callback_query(q.id.clone()).await?;
        menu::start_pardon_flow(bot, chat_id, user_id, state).await?;
        return Ok(());
    }

    if data == "menu_reply" {
        bot.answer_callback_query(q.id.clone()).await?;
        menu::start_reply_flow(bot, chat_id, user_id, state).await?;
        return Ok(());
    }

    if data == "menu_addreplies" {
        bot.answer_callback_query(q.id.clone()).await?;
        menu::start_addreplies_flow(bot, chat_id, user_id, state).await?;
        return Ok(());
    }

    if let Some(id_str) = data.strip_prefix("skip_") {
        if let Ok(id) = id_str.parse::<u64>() {
            if let Some(proposal) = state.proposals.take_active(id).await {
                state.proposals.move_to_back(proposal).await;
                bot.answer_callback_query(q.id.clone())
                    .text(l10n!(lang, "skipped_msg"))
                    .await?;
            } else {
                bot.answer_callback_query(q.id.clone())
                    .text(l10n!(lang, "already_processing"))
                    .await?;
            }
            proposals::show_next_proposal(bot, chat_id, state, lang).await?;
        } else {
            bot.answer_callback_query(q.id.clone()).await?;
        }
        return Ok(());
    }

    if let Some(id_str) = data.strip_prefix("report_dismiss_") {
        if let Ok(report_id) = id_str.parse::<i32>() {
            reports::handle_report_dismiss(bot, chat_id, report_id, q, state, lang).await?;
        }
        return Ok(());
    }

    if let Some(id_str) = data.strip_prefix("report_delete_") {
        if let Ok(report_id) = id_str.parse::<i32>() {
            reports::handle_report_delete(bot, chat_id, report_id, q, state, lang).await?;
        }
        return Ok(());
    }

    if let Some(id_str) = data.strip_prefix("report_ban_") {
        if let Ok(report_id) = id_str.parse::<i32>() {
            reports::handle_report_ban(bot, chat_id, report_id, q, state, lang).await?;
        }
        return Ok(());
    }

    if let Some(id_str) = data.strip_prefix("discardnext_") {
        if let Ok(id) = id_str.parse::<u64>() {
            state.proposals.discard_rejected(id).await;
        }
        proposals::show_next_proposal(bot, chat_id, state, lang).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if let Some(id_str) = data.strip_prefix("backtopending_") {
        if let Ok(id) = id_str.parse::<u64>() {
            match state.proposals.return_to_pending(id).await {
                Some(_) => {
                    bot.answer_callback_query(q.id.clone())
                        .text(l10n!(lang, "returned_to_queue"))
                        .await?;
                    delete_callback_message(bot, chat_id, q).await?;
                    proposals::show_next_proposal(bot, chat_id, state, lang).await?;
                }
                None => {
                    bot.answer_callback_query(q.id.clone())
                        .text(l10n!(lang, "proposal_gone"))
                        .await?;
                }
            }
        }
        return Ok(());
    }

    if let Some(id_str) = data.strip_prefix("approve_") {
        let id: u64 = id_str.parse().map_err(|_| anyhow!("Invalid callback id"))?;
        handle_approve(bot, chat_id, id, q, state, lang).await?;
    } else if let Some(id_str) = data.strip_prefix("reject_") {
        let id: u64 = id_str.parse().map_err(|_| anyhow!("Invalid callback id"))?;
        handle_reject(bot, chat_id, id, q, state, lang).await?;
    } else if let Some(id_str) = data.strip_prefix("reason_") {
        let id: u64 = id_str.parse().map_err(|_| anyhow!("Invalid callback id"))?;
        handle_reason(bot, chat_id, id, q, state, lang).await?;
    } else if let Some(id_str) = data.strip_prefix("ban_reason_") {
        let id: u64 = id_str.parse().map_err(|_| anyhow!("Invalid callback id"))?;
        handle_ban_reason(bot, chat_id, id, q, state, lang).await?;
    }
    Ok(())
}

async fn handle_approve(
    bot: &Bot,
    chat_id: ChatId,
    id: u64,
    q: &CallbackQuery,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let proposal = match state.proposals.take_active(id).await {
        Some(p) => p,
        None => {
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "already_processing"))
                .await?;
            delete_callback_message(bot, chat_id, q).await?;
            return Ok(());
        }
    };
    let reply_to = proposal.first().parent_message_id.map(|v| v as i32);
    let cfg = state.config.read().await;
    let channel_id = cfg.channel_id;
    let bot_username = cfg.bot_username.clone();
    drop(cfg);
    let plan = state.db.get_client_plan_by_bot_id(state.bot_id).await?;
    let is_pro = plan == "pro";
    match media::publish(
        bot,
        channel_id,
        &proposal,
        &bot_username,
        &state.master_config.watermark_username,
        is_pro,
        reply_to,
        lang,
    )
    .await
    {
        Ok(Some(_channel_msg_id)) => {
            let admin_name = utils::get_admin_name(state, q.from.id.0 as i64).await;
            let _ = state
                .db
                .log_action(
                    state.bot_id,
                    q.from.id.0 as i64,
                    &admin_name,
                    "approve",
                    &id.to_string(),
                )
                .await;
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "published"))
                .await?;
            delete_callback_message(bot, chat_id, q).await?;
            proposals::show_next_proposal(bot, chat_id, state, lang).await?;
        }
        Ok(None) => {
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "published_no_id"))
                .await?;
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to publish proposal");
            state.proposals.requeue(proposal).await;
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "failed_publish"))
                .await?;
        }
    }
    Ok(())
}

async fn handle_reject(
    bot: &Bot,
    chat_id: ChatId,
    id: u64,
    q: &CallbackQuery,
    state: &WorkerState,
    lang: Locale,
) -> R {
    match state.proposals.reject(id).await {
        Some(_) => {
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "rejected"))
                .await?;
            delete_callback_message(bot, chat_id, q).await?;
            let kb = InlineKeyboardMarkup::new(vec![
                vec![
                    InlineKeyboardButton::callback(
                        l10n!(lang, "reason_btn"),
                        format!("reason_{id}"),
                    ),
                    InlineKeyboardButton::callback(
                        l10n!(lang, "next_btn"),
                        format!("discardnext_{id}"),
                    ),
                    InlineKeyboardButton::callback(
                        l10n!(lang, "ban_btn"),
                        format!("ban_reason_{id}"),
                    ),
                ],
                vec![InlineKeyboardButton::callback(
                    l10n!(lang, "back_btn"),
                    format!("backtopending_{id}"),
                )],
            ]);
            bot.send_message(chat_id, l10n!(lang, "choose_action"))
                .reply_markup(kb)
                .await?;
        }
        None => {
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "already_processing"))
                .await?;
            delete_callback_message(bot, chat_id, q).await?;
        }
    }
    Ok(())
}

async fn handle_reason(
    bot: &Bot,
    chat_id: ChatId,
    id: u64,
    q: &CallbackQuery,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let admin_id = q.from.id.0 as i64;
    let sender_id = match state.proposals.rejected_sender(id).await {
        Some(s) => s,
        None => {
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "proposal_gone"))
                .await?;
            return Ok(());
        }
    };
    let kb = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        l10n!(lang, "cancel_flow_btn"),
        "flowcancel",
    )]]);
    bot.send_message(chat_id, l10n!(lang, "enter_rejection_reason"))
        .reply_markup(kb)
        .await?;
    state.user_states.insert(
        admin_id,
        UserStateEntry {
            state: "reason".to_string(),
            temp_target_id: sender_id,
            proposal_id: id,
        },
    );
    bot.answer_callback_query(q.id.clone())
        .text(l10n!(lang, "enter_reason_callback"))
        .await?;
    Ok(())
}

async fn handle_ban_reason(
    bot: &Bot,
    chat_id: ChatId,
    id: u64,
    q: &CallbackQuery,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let admin_id = q.from.id.0 as i64;
    let sender_id = match state.proposals.rejected_sender(id).await {
        Some(s) => s,
        None => {
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "proposal_gone"))
                .await?;
            return Ok(());
        }
    };
    let kb = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        l10n!(lang, "cancel_flow_btn"),
        "flowcancel",
    )]]);
    bot.send_message(chat_id, l10n!(lang, "enter_ban_reason"))
        .reply_markup(kb)
        .await?;
    state.user_states.insert(
        admin_id,
        UserStateEntry {
            state: "ban_reason".to_string(),
            temp_target_id: sender_id,
            proposal_id: id,
        },
    );
    bot.answer_callback_query(q.id.clone())
        .text(l10n!(lang, "enter_reason_callback"))
        .await?;
    Ok(())
}

async fn delete_callback_message(bot: &Bot, chat_id: ChatId, q: &CallbackQuery) -> R {
    if let Some(MaybeInaccessibleMessage::Regular(msg)) = &q.message {
        bot.delete_message(chat_id, msg.id).await?;
    }
    Ok(())
}
