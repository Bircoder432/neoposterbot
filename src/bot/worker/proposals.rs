use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

use crate::bot::media;
use crate::bot::worker::WorkerState;
use crate::bot::worker::utils;
use crate::db::models::{NewMessage, Proposal};
use crate::locales::{L10n, Locale};

type R = anyhow::Result<()>;

pub(super) async fn handle_proposal(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let chat_id = msg.chat.id;
    let lang = state.db.get_language(state.bot_id).await?;

    if state.db.is_banned(state.bot_id, user_id).await? {
        bot.send_message(chat_id, L10n::user_banned(lang)).await?;
        return Ok(());
    }
    if msg.chat.is_group() || msg.chat.is_supergroup() {
        return Ok(());
    }
    if !utils::has_content(msg) {
        return Ok(());
    }
    if state
        .db
        .message_exists(state.bot_id, chat_id.0, msg.id.0 as i32)
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
        bot_id: state.bot_id,
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

pub(super) async fn handle_reply_content(
    bot: &Bot,
    msg: &Message,
    state: &WorkerState,
    parent_id: i64,
) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let chat_id = msg.chat.id;
    let lang = state.db.get_language(state.bot_id).await?;

    if state
        .db
        .message_exists(state.bot_id, chat_id.0, msg.id.0 as i32)
        .await?
    {
        state.db.clear_user_state(state.bot_id, user_id).await?;
        return Ok(());
    }

    let (media_type, media_file_id) = media::extract_media_info(msg, lang);
    let message_text = media::extract_message_text(msg, lang);
    let media_group_id = msg.media_group_id().map(|s| s.to_string());

    let proposal_group_id = media_group_id
        .clone()
        .unwrap_or_else(|| format!("single_{}", uuid::Uuid::new_v4()));

    let new_msg = NewMessage {
        bot_id: state.bot_id,
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
            state.db.clear_user_state(state.bot_id, user_id).await?;
            bot.send_message(chat_id, L10n::reply_accepted(lang))
                .await?;
            notify_admins(bot, state, &new_msg, lang).await?;
        }
        Ok(false) => {}
        Err(e) => {
            tracing::error!(error = %e, "Failed to save reply");
            bot.send_message(chat_id, L10n::error_sending_reply(lang))
                .await?;
            state.db.clear_user_state(state.bot_id, user_id).await?;
        }
    }
    Ok(())
}

pub(super) async fn handle_send_reason(
    bot: &Bot,
    msg: &Message,
    state: &WorkerState,
    target_user_id: i64,
) -> R {
    let admin_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    let reason = msg.text().unwrap_or("No reason provided");

    let _ = bot
        .send_message(ChatId(target_user_id), L10n::rejected_reason(lang, reason))
        .await;
    state.db.clear_user_state(state.bot_id, admin_id).await?;
    bot.send_message(msg.chat.id, L10n::reason_sent(lang))
        .await?;
    Ok(())
}

pub(super) async fn handle_send_ban_reason(
    bot: &Bot,
    msg: &Message,
    state: &WorkerState,
    target_user_id: i64,
) -> R {
    let admin_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    let reason = msg.text().unwrap_or("No reason provided");

    match state
        .db
        .create_ban_record(state.bot_id, target_user_id, reason)
        .await
    {
        Ok(ban_id) => {
            state.db.ban_user(state.bot_id, target_user_id).await?;
            state.db.clear_user_state(state.bot_id, admin_id).await?;

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
            state.db.clear_user_state(state.bot_id, admin_id).await?;
        }
    }
    Ok(())
}

async fn notify_admins(bot: &Bot, state: &WorkerState, msg: &NewMessage, lang: Locale) -> R {
    let admins = state.db.get_admins(state.bot_id).await?;
    let notification = L10n::new_proposal_notif(lang, &msg.message_text, &msg.media_type);
    for admin in admins {
        if !admin.frozen {
            let _ = bot.send_message(ChatId(admin.user_id), &notification).await;
        }
    }
    Ok(())
}

pub(super) async fn show_next_proposal(
    bot: &Bot,
    chat_id: ChatId,
    state: &WorkerState,
    lang: Locale,
) -> R {
    match state.db.get_next_pending_proposal(state.bot_id).await? {
        None => {
            bot.send_message(chat_id, L10n::no_new_proposals(lang))
                .await?;
        }
        Some((gid,)) => {
            let messages = state
                .db
                .get_proposal_by_group_id(state.bot_id, &gid)
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
