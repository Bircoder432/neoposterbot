use teloxide::prelude::*;

use crate::bot::worker::WorkerState;

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
