use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

use crate::bot::media;
use crate::bot::worker::utils;
use crate::bot::worker::{UserStateEntry, WorkerState};
use crate::db::Database;
use crate::db::models::IncomingProposal;
use crate::hashing::hash_user_id;
use crate::locales::{L10n, Locale};

type R = anyhow::Result<()>;

pub(super) async fn handle_proposal(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let chat_id = msg.chat.id;
    let lang = state.db.get_language(state.bot_id).await?;

    if state
        .db
        .is_banned(state.bot_id, &hash_user_id(user_id))
        .await?
    {
        bot.send_message(chat_id, L10n::user_banned(lang)).await?;
        return Ok(());
    }

    if msg.chat.is_group() || msg.chat.is_supergroup() {
        return Ok(());
    }

    if !utils::has_content(msg) {
        return Ok(());
    }

    let (media_type, media_file_id) = media::extract_media_info(msg, lang);
    let mut message_text = media::extract_message_text(msg, lang);
    let media_group_id = msg.media_group_id().map(|s| s.to_string());

    let mut parent_message_id = None;
    let cfg = state.config.read().await;
    let channel_id = cfg.channel_id;
    let channel_username = cfg.channel_username.clone();
    drop(cfg);

    if !message_text.is_empty() {
        if let Some(channel_msg_id) =
            parse_and_strip_tme_link(&mut message_text, channel_id, channel_username.as_deref())
        {
            parent_message_id = Some(channel_msg_id as i64);
        }
    }

    if media_type == "text" && message_text.trim().is_empty() {
        message_text = " ".to_string();
    }

    let proposal_group_id = media_group_id
        .clone()
        .unwrap_or_else(|| format!("single_{}", uuid::Uuid::new_v4()));

    let notif_text = message_text.clone();
    let notif_type = media_type.clone();

    let inserted = state
        .proposals
        .push(IncomingProposal {
            chat_id: chat_id.0,
            telegram_message_id: msg.id.0 as i32,
            sender_id: user_id,
            message_text,
            media_type,
            media_file_id,
            media_group_id,
            proposal_group_id,
            parent_message_id,
        })
        .await;

    if inserted {
        if parent_message_id.is_some() {
            bot.send_message(chat_id, L10n::reply_accepted(lang))
                .await?;
        } else {
            bot.send_message(chat_id, L10n::proposal_accepted(lang))
                .await?;
        }
        notify_admins(bot, state, &notif_text, &notif_type, lang).await?;
    }
    Ok(())
}

fn parse_and_strip_tme_link(
    text: &mut String,
    channel_id: i64,
    channel_username: Option<&str>,
) -> Option<i32> {
    let lower_text = text.to_ascii_lowercase();
    if let Some(pos) = lower_text.find("t.me/") {
        let prefix_start = if pos >= 8 && lower_text.get(pos - 8..pos) == Some("https://") {
            pos - 8
        } else if pos >= 7 && lower_text.get(pos - 7..pos) == Some("http://") {
            pos - 7
        } else {
            pos
        };
        let url_end = lower_text[pos..]
            .find(|c: char| c.is_whitespace())
            .map(|e| pos + e)
            .unwrap_or(lower_text.len());
        let path = &lower_text[pos + 5..url_end];
        let path = path.split('?').next().unwrap_or(path);
        let parts: Vec<&str> = path.split('/').collect();

        let mut is_match = false;
        let mut msg_id = 0;

        if parts.len() >= 3 && parts[0] == "c" {
            let internal_id_str = parts[1];
            let channel_id_str = channel_id.to_string();
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

    let (media_type, media_file_id) = media::extract_media_info(msg, lang);
    let message_text = media::extract_message_text(msg, lang);
    let media_group_id = msg.media_group_id().map(|s| s.to_string());
    let proposal_group_id = media_group_id
        .clone()
        .unwrap_or_else(|| format!("single_{}", uuid::Uuid::new_v4()));

    let notif_text = message_text.clone();
    let notif_type = media_type.clone();

    let inserted = state
        .proposals
        .push(IncomingProposal {
            chat_id: chat_id.0,
            telegram_message_id: msg.id.0 as i32,
            sender_id: user_id,
            message_text,
            media_type,
            media_file_id,
            media_group_id,
            proposal_group_id,
            parent_message_id: Some(parent_id),
        })
        .await;

    if inserted {
        // Стейт сброшен в дефолт - запись удаляется из мапы.
        state.user_states.remove(&user_id);
        bot.send_message(chat_id, L10n::reply_accepted(lang))
            .await?;
        notify_admins(bot, state, &notif_text, &notif_type, lang).await?;
    }
    Ok(())
}

pub(super) async fn handle_send_reason(
    bot: &Bot,
    msg: &Message,
    state: &WorkerState,
    entry: &UserStateEntry,
) -> R {
    let admin_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    let reason = msg.text().unwrap_or("No reason provided");

    if state
        .proposals
        .take_rejected(entry.proposal_id)
        .await
        .is_none()
    {
        state.user_states.remove(&admin_id);
        bot.send_message(msg.chat.id, L10n::proposal_gone(lang))
            .await?;
        return Ok(());
    }

    let _ = bot
        .send_message(
            ChatId(entry.temp_target_id),
            L10n::rejected_reason(lang, reason),
        )
        .await;
    state.user_states.remove(&admin_id);
    bot.send_message(msg.chat.id, L10n::reason_sent(lang))
        .await?;
    Ok(())
}

pub(super) async fn handle_send_ban_reason(
    bot: &Bot,
    msg: &Message,
    state: &WorkerState,
    entry: &UserStateEntry,
) -> R {
    let admin_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    let reason = msg.text().unwrap_or("No reason provided");
    let target_user_id = entry.temp_target_id;

    let proposal = match state.proposals.take_rejected(entry.proposal_id).await {
        Some(p) => p,
        None => {
            state.user_states.remove(&admin_id);
            bot.send_message(msg.chat.id, L10n::proposal_gone(lang))
                .await?;
            return Ok(());
        }
    };

    let ban_id = Database::new_ban_id();
    let _ = bot
        .send_message(
            ChatId(target_user_id),
            L10n::user_banned_appeal(lang, reason, &ban_id),
        )
        .await;

    match state
        .db
        .ban_user(state.bot_id, &hash_user_id(target_user_id), reason, &ban_id)
        .await
    {
        Ok(()) => {
            state.user_states.remove(&admin_id);
            bot.send_message(msg.chat.id, L10n::user_banned_success(lang))
                .await?;
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to ban user");
            state.proposals.restore_rejected(proposal).await;
            bot.send_message(msg.chat.id, L10n::error_banning_user(lang))
                .await?;
        }
    }
    Ok(())
}

async fn notify_admins(
    bot: &Bot,
    state: &WorkerState,
    text: &str,
    media_type: &str,
    lang: Locale,
) -> R {
    let admins = state.db.get_admins(state.bot_id).await?;
    let notification = L10n::new_proposal_notif(lang, text, media_type);
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
    match state.proposals.pop_next().await {
        None => {
            bot.send_message(chat_id, L10n::no_new_proposals(lang))
                .await?;
        }
        Some(proposal) => {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_link_public_match() {
        let mut text = "Привет, посмотри https://t.me/mychan/15 вот это".to_string();
        let res = parse_and_strip_tme_link(&mut text, 0, Some("mychan"));

        assert_eq!(res, Some(15));
        // Ссылка должна быть вырезана, а текст очищен от лишних пробелов по краям
        assert_eq!(text, "Привет, посмотри  вот это");
    }

    #[test]
    fn test_strip_link_private_match() {
        // channel_id в Telegram для приватных каналов обычно начинается с -100
        let mut text = "https://t.me/c/1234567890/42".to_string();
        let res = parse_and_strip_tme_link(&mut text, -1001234567890, None);

        assert_eq!(res, Some(42));
        assert_eq!(text, ""); // Вся строка была ссылкой
    }

    #[test]
    fn test_strip_link_no_match() {
        let mut text = "Текст без ссылок https://t.me/otherchan/10".to_string();
        let original_text = text.clone();

        let res = parse_and_strip_tme_link(&mut text, 0, Some("mychan"));

        assert_eq!(res, None);
        assert_eq!(text, original_text);
    }

    #[test]
    fn test_strip_link_case_insensitive() {
        let mut text = "Check https://t.me/MyChAn/77 out".to_string();
        let res = parse_and_strip_tme_link(&mut text, 0, Some("mychan"));

        assert_eq!(res, Some(77));
        assert!(!text.contains("https://"));
    }
}
