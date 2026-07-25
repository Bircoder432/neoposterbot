use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use tracing;

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
    let mut message_text = media::extract_message_text(msg, lang);
    let media_group_id = msg.media_group_id().map(|s| s.to_string());

    // ── Фича: обработка прямых ссылок на посты канала ──
    let mut parent_message_id = None;

    let cfg = state.config.read().await;
    let channel_id = cfg.channel_id;
    let channel_username = cfg.channel_username.clone();

    drop(cfg);

    if !message_text.is_empty() {
        if let Some(channel_msg_id) =
            parse_and_strip_tme_link(&mut message_text, channel_id, channel_username.as_deref())
        {
            // Сохраняем как ОТРИЦАТЕЛЬНОЕ число, чтобы отметить, что это прямой ID поста, а не ID из БД
            parent_message_id = Some(-(channel_msg_id as i64));
        }
    }

    // Если после удаления ссылки остался только пробел/пустота, задаём минимальный текст
    if media_type == "text" && message_text.trim().is_empty() {
        message_text = " ".to_string();
    }
    // ── Конец фичи ──

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
        parent_message_id,
    };

    let inserted = state.db.save_message(&new_msg).await?;
    if inserted {
        // Отправляем пользователю уведомление о принятии ответа, если ссылка была найдена
        if parent_message_id.is_some() {
            bot.send_message(chat_id, L10n::reply_accepted(lang))
                .await?;
        } else {
            bot.send_message(chat_id, L10n::proposal_accepted(lang))
                .await?;
        }
        notify_admins(bot, state, &new_msg, lang).await?;
    }
    Ok(())
}

// ── Вспомогательная функция для парсинга и удаления ссылок ──
fn parse_and_strip_tme_link(
    text: &mut String,
    channel_id: i64,
    channel_username: Option<&str>,
) -> Option<i32> {
    let lower_text = text.to_lowercase();
    if let Some(pos) = lower_text.find("t.me/") {
        // Определяем начало URL (включая http:// или https://)
        let prefix_start = if pos >= 8 && &lower_text[pos - 8..pos] == "https://" {
            pos - 8
        } else if pos >= 7 && &lower_text[pos - 7..pos] == "http://" {
            pos - 7
        } else {
            pos
        };

        // Находим конец URL (пробел или конец строки)
        let url_end = lower_text[pos..]
            .find(|c: char| c.is_whitespace())
            .map(|e| pos + e)
            .unwrap_or(lower_text.len());

        // Парсим путь
        let path = &lower_text[pos + 5..url_end]; // пропускаем "t.me/"
        let path = path.split('?').next().unwrap_or(path); // убираем query параметры
        let parts: Vec<&str> = path.split('/').collect();

        let mut is_match = false;
        let mut msg_id = 0;

        if parts.len() >= 3 && parts[0] == "c" {
            // Приватный канал: t.me/c/1234567890/5
            let internal_id_str = parts[1];
            let channel_id_str = channel_id.to_string();
            // channel_id в Bot API имеет формат -1001234567890, в ссылке только 1234567890
            let expected_internal_id = channel_id_str
                .strip_prefix("-100")
                .unwrap_or(&channel_id_str);

            if expected_internal_id == internal_id_str {
                if let Ok(id) = parts[2].parse::<i32>() {
                    msg_id = id;
                    is_match = true;
                }
            }
        } else if parts.len() >= 2 {
            // Публичный канал: t.me/username/5
            let username_from_url = parts[0];
            if let Some(uname) = channel_username {
                if uname.eq_ignore_ascii_case(username_from_url) {
                    if let Ok(id) = parts[1].parse::<i32>() {
                        msg_id = id;
                        is_match = true;
                    }
                }
            }
        }

        // Если ссылка принадлежит нашему каналу, удаляем её из текста
        if is_match {
            text.replace_range(prefix_start..url_end, "");
            *text = text.trim().to_string();
            return Some(msg_id);
        }
    }
    None
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
