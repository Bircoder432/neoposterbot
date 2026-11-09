use teloxide::prelude::*;

use crate::bot::worker::WorkerState;
use crate::l10n;
use crate::locales::Locale;

pub(super) async fn is_authorized(state: &WorkerState, user_id: i64) -> anyhow::Result<bool> {
    Ok(state.client_tg_id == user_id || state.db.is_admin(state.bot_id, user_id).await?)
}

pub(super) fn has_content(msg: &Message) -> bool {
    msg.text().is_some()
        || msg.photo().is_some()
        || msg.document().is_some()
        || msg.video().is_some()
        || msg.video_note().is_some()
        || msg.audio().is_some()
        || msg.voice().is_some()
        || msg.sticker().is_some()
}

pub(super) async fn resolve_user_name(bot: &Bot, user_id: i64) -> String {
    match bot.get_chat(ChatId(user_id)).await {
        Ok(chat) => chat
            .username()
            .or_else(|| chat.first_name())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("user_{user_id}")),
        Err(_) => format!("user_{user_id}"),
    }
}

pub async fn get_admin_name(state: &WorkerState, user_id: i64) -> String {
    if user_id == state.client_tg_id {
        return "Owner".to_string();
    }
    if let Ok(admins) = state.db.get_admins(state.bot_id).await {
        if let Some(admin) = admins.iter().find(|a| a.user_id == user_id) {
            return admin.user_name.clone();
        }
    }
    format!("user_{user_id}")
}

/// Extracts a channel post id from a public or private t.me post link.
pub(super) fn parse_post_link(link: &str) -> Option<i32> {
    let lower = link.to_lowercase();
    let pos = lower.find("t.me/")?;
    let path = &lower[pos + 5..];
    let path = path.split('?').next().unwrap_or(path);
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 3 && parts[0] == "c" {
        parts[2].parse::<i32>().ok()
    } else if parts.len() >= 2 && parts[0] != "c" {
        parts[1].parse::<i32>().ok()
    } else {
        None
    }
}

/// Human-readable localized label for a raw media type (used in notifications).
pub(super) fn media_label(lang: Locale, media_type: &str) -> String {
    let key = match media_type {
        "text" => "media_label_text",
        "photo" => "media_label_photo",
        "document" => "media_label_document",
        "video" => "media_label_video",
        "video_note" => "media_label_video_note",
        "audio" => "media_label_audio",
        "voice" => "media_label_voice",
        "sticker" => "media_label_sticker",
        _ => return media_type.to_string(),
    };
    l10n!(lang, key)
}

/// Truncates a string to at most `max` characters (by chars, not bytes),
/// appending an ellipsis when the text was cut.
pub(super) fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_post_link_public_channel() {
        assert_eq!(
            parse_post_link("https://t.me/my_awesome_channel/42"),
            Some(42)
        );
        assert_eq!(parse_post_link("http://t.me/channel/100"), Some(100));
        assert_eq!(parse_post_link("t.me/channel/999"), Some(999));
    }

    #[test]
    fn test_parse_post_link_private_channel() {
        assert_eq!(parse_post_link("https://t.me/c/1234567890/55"), Some(55));
        assert_eq!(parse_post_link("https://t.me/c/9876543210/1"), Some(1));
    }

    #[test]
    fn test_parse_post_link_with_query_params() {
        assert_eq!(parse_post_link("https://t.me/channel/42?single"), Some(42));
        assert_eq!(
            parse_post_link("https://t.me/c/123/10?comment=123"),
            Some(10)
        );
    }

    #[test]
    fn test_parse_post_link_invalid() {
        assert_eq!(parse_post_link("not a link at all"), None);
        assert_eq!(parse_post_link("https://t.me/username"), None);
        assert_eq!(parse_post_link("https://t.me/c/12345"), None);
        assert_eq!(parse_post_link("https://vk.com/wall123_456"), None);
        assert_eq!(parse_post_link(""), None);
    }

    #[test]
    fn test_truncate_chars() {
        assert_eq!(truncate_chars("hello", 10), "hello");
        assert_eq!(truncate_chars("привет мир", 6), "привет…");
        assert_eq!(truncate_chars("", 5), "");
    }
}
