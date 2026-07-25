use anyhow::{Result, anyhow};
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, MaybeInaccessibleMessage};

use super::admins;
use super::proposals;
use super::utils;
use crate::bot::media;
use crate::bot::worker::WorkerState;
use crate::db::models::Proposal;
use crate::locales::{L10n, Locale};

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
                    "{}\n\nОтправьте в ваш канал команду:\n<code>/connect {}</code>",
                    L10n::setup_enter_channel(lang),
                    code
                ),
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
        }
        return Ok(());
    }

    let lang = state.db.get_language(bot_id).await?;

    // Проверка на заморозку модератора
    if let Some(true) = state.db.get_admin_status(bot_id, user_id).await? {
        bot.answer_callback_query(q.id.clone())
            .text(L10n::mod_frozen_action(lang))
            .await?;
        return Ok(());
    }

    if !utils::is_authorized(state, user_id).await? {
        bot.answer_callback_query(q.id.clone())
            .text(L10n::no_access(lang))
            .await?;
        return Ok(());
    }

    let (chat_id, message_id) = match &q.message {
        Some(MaybeInaccessibleMessage::Regular(msg)) => (msg.chat.id, msg.id),
        _ => return Ok(()),
    };

    // ── Admin management callbacks (owner only) ──
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
                .text(L10n::only_owner_remove_admins(lang))
                .await?;
            return Ok(());
        }

        if let Some(code) = data.strip_prefix("revoke_invite_") {
            state.db.revoke_admin_invite(state.bot_id, code).await?;
            bot.answer_callback_query(q.id.clone())
                .text(L10n::invite_revoked_msg(lang))
                .await?;
            bot.edit_message_text(chat_id, message_id, L10n::invite_revoked_msg(lang))
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
                let admins_list = state.db.get_admins(state.bot_id).await?;
                if let Some(admin) = admins_list.iter().find(|a| a.user_id == target_id) {
                    let (text, kb) = admins::build_admin_manage_inline(admin, lang);
                    bot.edit_message_text(chat_id, message_id, text)
                        .reply_markup(kb)
                        .await
                        .ok();
                }
                bot.answer_callback_query(q.id.clone())
                    .text(L10n::admin_frozen_msg(lang))
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
                            .text(L10n::limit_reached_unfreeze(lang))
                            .await?;
                        return Ok(());
                    }
                }
                state
                    .db
                    .set_admin_frozen(state.bot_id, target_id, false)
                    .await?;
                let admins_list = state.db.get_admins(state.bot_id).await?;
                if let Some(admin) = admins_list.iter().find(|a| a.user_id == target_id) {
                    let (text, kb) = admins::build_admin_manage_inline(admin, lang);
                    bot.edit_message_text(chat_id, message_id, text)
                        .reply_markup(kb)
                        .await
                        .ok();
                }
                bot.answer_callback_query(q.id.clone())
                    .text(L10n::admin_unfrozen_msg(lang))
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
                            L10n::confirm_yes(lang),
                            format!("confirm_rmadmin_{}", target_id),
                        )],
                        vec![InlineKeyboardButton::callback(
                            L10n::confirm_no(lang),
                            "cancel_rmadmin",
                        )],
                    ]);
                    let text = L10n::confirm_remove_admin(lang, &admin.user_name, target_id);
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
                let _ = bot
                    .send_message(ChatId(target_id), L10n::admin_removed_notification(lang))
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

    // ── Proposal callbacks ──
    if data == "next" {
        proposals::show_next_proposal(bot, chat_id, state, lang).await?;
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
        // ── FIX: используем msg_id (ID сообщения в БД) вместо sender_id ──
        let msg_id: i64 = id_str.parse().map_err(|_| anyhow!("Invalid callback id"))?;
        handle_reason(bot, chat_id, msg_id, q, state, lang).await?;
    } else if let Some(id_str) = data.strip_prefix("ban_reason_") {
        // ── FIX: используем msg_id (ID сообщения в БД) вместо sender_id ──
        let msg_id: i64 = id_str.parse().map_err(|_| anyhow!("Invalid callback id"))?;
        handle_ban_reason(bot, chat_id, msg_id, q, state, lang).await?;
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
        .get_message_by_id(state.bot_id, msg_id)
        .await?
        .ok_or_else(|| anyhow!("Message not found"))?;
    let group_id = msg.proposal_group_id.clone();
    let messages = state
        .db
        .get_proposal_by_group_id(state.bot_id, &group_id)
        .await?;
    let proposal = Proposal {
        group_id: group_id.clone(),
        messages,
    };

    let reply_to = if let Some(parent_id) = proposal.first().parent_message_id {
        if parent_id < 0 {
            Some((-parent_id) as i32)
        } else {
            match state.db.get_message_by_id(state.bot_id, parent_id).await? {
                Some(parent) if parent.channel_message_id.is_some() => parent.channel_message_id,
                _ => None,
            }
        }
    } else {
        None
    };

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
        Ok(Some(channel_msg_id)) => {
            state
                .db
                .update_channel_message_id(state.bot_id, &group_id, channel_msg_id)
                .await?;
            state
                .db
                .update_proposal_status(state.bot_id, &group_id, "approved")
                .await?;
            bot.answer_callback_query(q.id.clone())
                .text(L10n::published(lang))
                .await?;
            delete_callback_message(bot, chat_id, q).await?;
            proposals::show_next_proposal(bot, chat_id, state, lang).await?;
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
        .get_message_by_id(state.bot_id, msg_id)
        .await?
        .ok_or_else(|| anyhow!("Message not found"))?;
    let group_id = msg.proposal_group_id.clone();

    state
        .db
        .update_proposal_status(state.bot_id, &group_id, "rejected")
        .await?;
    // ── FIX: НЕ удаляем предложение из БД, чтобы позже можно было
    //    найти sender_id по msg_id при обработке кнопок reason/ban_reason ──

    bot.answer_callback_query(q.id.clone())
        .text(L10n::rejected(lang))
        .await?;
    delete_callback_message(bot, chat_id, q).await?;

    // ── FIX: используем msg_id (ID сообщения в БД) вместо sender_id ──
    let kb = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(L10n::reason_btn(lang), format!("reason_{msg_id}")),
        InlineKeyboardButton::callback(L10n::next_btn(lang), "next"),
        InlineKeyboardButton::callback(L10n::ban_btn(lang), format!("ban_reason_{msg_id}")),
    ]]);

    bot.send_message(chat_id, L10n::choose_action(lang))
        .reply_markup(kb)
        .await?;
    Ok(())
}

async fn handle_reason(
    bot: &Bot,
    chat_id: ChatId,
    msg_id: i64,
    q: &CallbackQuery,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let admin_id = q.from.id.0 as i64;

    // ── FIX: получаем sender_id из БД по msg_id, а не из callback_data ──
    let msg = state.db.get_message_by_id(state.bot_id, msg_id).await?;
    let sender_id = match msg {
        Some(m) => m.sender_id,
        None => {
            bot.answer_callback_query(q.id.clone()).text("❌").await?;
            return Ok(());
        }
    };

    bot.send_message(chat_id, L10n::enter_rejection_reason(lang))
        .await?;
    state
        .db
        .set_user_state(state.bot_id, admin_id, "reason", sender_id)
        .await?;
    bot.answer_callback_query(q.id.clone())
        .text(L10n::enter_reason_callback(lang))
        .await?;
    Ok(())
}

async fn handle_ban_reason(
    bot: &Bot,
    chat_id: ChatId,
    msg_id: i64,
    q: &CallbackQuery,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let admin_id = q.from.id.0 as i64;

    // ── FIX: получаем sender_id из БД по msg_id, а не из callback_data ──
    let msg = state.db.get_message_by_id(state.bot_id, msg_id).await?;
    let sender_id = match msg {
        Some(m) => m.sender_id,
        None => {
            bot.answer_callback_query(q.id.clone()).text("❌").await?;
            return Ok(());
        }
    };

    bot.send_message(chat_id, L10n::enter_ban_reason(lang))
        .await?;
    state
        .db
        .set_user_state(state.bot_id, admin_id, "ban_reason", sender_id)
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
