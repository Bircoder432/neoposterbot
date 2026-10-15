use crate::bot::media;
use crate::bot::worker::admins::notify_admins;
use crate::bot::worker::utils;
use crate::bot::worker::{UserStateEntry, WorkerState};
use crate::db::Database;
use crate::db::models::IncomingProposal;
use crate::hashing::hash_user_id;
use crate::l10n;
use crate::locales::Locale;
use regex::Regex;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

type R = anyhow::Result<()>;

fn contains_banned_username(text: &str, banned_usernames: &[String]) -> bool {
    let lower_text = text.to_lowercase();
    for uname in banned_usernames {
        let escaped = regex::escape(uname);
        let pattern = format!(
            r"(?i)(^|[^a-zA-Z0-9_])(?:@|t\.me/)?{}([^a-zA-Z0-9_]|$)",
            escaped
        );
        if let Ok(re) = Regex::new(&pattern) {
            if re.is_match(&lower_text) {
                return true;
            }
        }
    }
    false
}

pub(super) async fn handle_proposal(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let chat_id = msg.chat.id;
    let lang = state.db.get_language(state.bot_id).await?;

    if state
        .db
        .is_banned(state.bot_id, &hash_user_id(user_id))
        .await?
    {
        bot.send_message(chat_id, l10n!(lang, "user_banned"))
            .await?;
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

    let banned = match state.db.get_banned_usernames(state.bot_id).await {
        Ok(b) => b,
        Err(_) => Vec::new(),
    };
    if contains_banned_username(&message_text, &banned) {
        bot.send_message(chat_id, l10n!(lang, "auto_reject_forbidden_username"))
            .await?;
        return Ok(());
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
            bot.send_message(chat_id, l10n!(lang, "reply_accepted"))
                .await?;
        } else {
            bot.send_message(chat_id, l10n!(lang, "proposal_accepted"))
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

    if state
        .db
        .is_banned(state.bot_id, &hash_user_id(user_id))
        .await?
    {
        bot.send_message(chat_id, l10n!(lang, "user_banned"))
            .await?;
        return Ok(());
    }

    let (media_type, media_file_id) = media::extract_media_info(msg, lang);
    let message_text = media::extract_message_text(msg, lang);
    let media_group_id = msg.media_group_id().map(|s| s.to_string());

    let banned = match state.db.get_banned_usernames(state.bot_id).await {
        Ok(b) => b,
        Err(_) => Vec::new(),
    };
    if contains_banned_username(&message_text, &banned) {
        bot.send_message(chat_id, l10n!(lang, "auto_reject_forbidden_username"))
            .await?;
        return Ok(());
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
            parent_message_id: Some(parent_id),
        })
        .await;

    if inserted {
        state.user_states.remove(&user_id);
        bot.send_message(chat_id, l10n!(lang, "reply_accepted"))
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
        bot.send_message(msg.chat.id, l10n!(lang, "proposal_gone"))
            .await?;
        return Ok(());
    }

    let _ = bot
        .send_message(
            ChatId(entry.temp_target_id),
            l10n!(lang, "rejected_reason", reason = reason),
        )
        .await;

    let admin_name = utils::get_admin_name(state, admin_id).await;
    let _ = state
        .db
        .log_action(
            state.bot_id,
            admin_id,
            &admin_name,
            "reject",
            &entry.proposal_id.to_string(),
        )
        .await;

    state.user_states.remove(&admin_id);
    bot.send_message(msg.chat.id, l10n!(lang, "reason_sent"))
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
            bot.send_message(msg.chat.id, l10n!(lang, "proposal_gone"))
                .await?;
            return Ok(());
        }
    };

    let ban_id = Database::new_ban_id();
    let _ = bot
        .send_message(
            ChatId(target_user_id),
            l10n!(lang, "user_banned_appeal", reason = reason, ban_id = ban_id),
        )
        .await;

    match state
        .db
        .ban_user(state.bot_id, &hash_user_id(target_user_id), reason, &ban_id)
        .await
    {
        Ok(()) => {
            let admin_name = utils::get_admin_name(state, admin_id).await;
            let _ = state
                .db
                .log_action(state.bot_id, admin_id, &admin_name, "ban", &ban_id)
                .await;
            state.user_states.remove(&admin_id);
            bot.send_message(msg.chat.id, l10n!(lang, "user_banned_success"))
                .await?;
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to ban user");
            state.proposals.restore_rejected(proposal).await;
            bot.send_message(msg.chat.id, l10n!(lang, "error_banning_user"))
                .await?;
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
            bot.send_message(chat_id, l10n!(lang, "no_new_proposals"))
                .await?;
        }
        Some(proposal) => {
            bot.send_message(
                chat_id,
                l10n!(
                    lang,
                    "proposal_items_count",
                    count = proposal.messages.len()
                ),
            )
            .await?;
            if let Err(e) = media::send_for_moderation(bot, chat_id, &proposal, lang).await {
                bot.send_message(
                    chat_id,
                    l10n!(lang, "failed_display_media", e = e.to_string()),
                )
                .await?;
            }
            let first = proposal.first();
            let text = l10n!(
                lang,
                "proposal_action_header",
                id = first.id,
                date = first.created_at.format("%d.%m.%Y %H:%M").to_string()
            );
            let kb = InlineKeyboardMarkup::new(vec![vec![
                InlineKeyboardButton::callback(
                    l10n!(lang, "approve_btn"),
                    format!("approve_{}", first.id),
                ),
                InlineKeyboardButton::callback(
                    l10n!(lang, "reject_btn"),
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
        assert_eq!(text, "Привет, посмотри  вот это");
    }

    #[test]
    fn test_strip_link_private_match() {
        let mut text = "https://t.me/c/1234567890/42".to_string();
        let res = parse_and_strip_tme_link(&mut text, -1001234567890, None);
        assert_eq!(res, Some(42));
        assert_eq!(text, "");
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
