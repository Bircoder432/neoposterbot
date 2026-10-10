use super::utils;
use crate::bot::worker::WorkerState;
use crate::db::models::Admin;
use crate::db::repository::Database;
use crate::l10n;
use crate::locales::Locale;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

type R = anyhow::Result<()>;

pub(super) async fn handle_admin_invite(
    bot: &Bot,
    chat_id: ChatId,
    user_id: i64,
    code: &str,
    state: &WorkerState,
    lang: Locale,
) -> R {
    if state.db.is_admin(state.bot_id, user_id).await? {
        bot.send_message(chat_id, l10n!(lang, "admin_invite_already_admin"))
            .await?;
        return Ok(());
    }
    match state.db.use_admin_invite(state.bot_id, code).await {
        Ok(true) => {
            let plan = state.db.get_client_plan_by_bot_id(state.bot_id).await?;
            let admin_count = state
                .db
                .count_active_admins(state.bot_id, state.client_tg_id)
                .await?;
            let frozen = plan == "free" && admin_count >= 1;
            let user_name = utils::resolve_user_name(bot, user_id).await;
            state
                .db
                .add_admin(state.bot_id, user_id, &user_name, frozen)
                .await?;
            if frozen {
                bot.send_message(chat_id, l10n!(lang, "admin_added_frozen_notification"))
                    .await?;
            } else {
                bot.send_message(chat_id, l10n!(lang, "admin_added_notification"))
                    .await?;
            }
            let owner_msg = if frozen {
                l10n!(lang, "admin_added_frozen", name = user_name)
            } else {
                l10n!(lang, "admin_added", name = user_name)
            };
            let _ = bot
                .send_message(ChatId(state.client_tg_id), owner_msg)
                .await;
        }
        _ => {
            bot.send_message(chat_id, l10n!(lang, "admin_invite_invalid"))
                .await?;
        }
    }
    Ok(())
}

pub(super) async fn notify_admins(
    bot: &Bot,
    state: &WorkerState,
    text: &str,
    media_type: &str,
    lang: Locale,
) -> R {
    let admins = state.db.get_admins(state.bot_id).await?;
    let notification = l10n!(
        lang,
        "new_proposal_notif",
        text = text,
        media_type = media_type
    );
    for admin in admins {
        if !admin.frozen {
            let _ = bot.send_message(ChatId(admin.user_id), &notification).await;
        }
    }
    Ok(())
}

pub(super) async fn handle_add_admin(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    if !utils::is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, l10n!(lang, "only_owner_add_admins"))
            .await?;
        return Ok(());
    }
    let code = state.db.create_admin_invite(state.bot_id).await?;
    let cfg = state.config.read().await;
    let bot_username = cfg.bot_username.clone();
    drop(cfg);
    let link = format!("https://t.me/{bot_username}?start=admin_{code}");
    let kb = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        l10n!(lang, "revoke_invite_btn"),
        format!("revoke_invite_{}", code),
    )]]);
    bot.send_message(msg.chat.id, l10n!(lang, "admin_invite_link", link = link))
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(kb)
        .await?;
    Ok(())
}

pub(super) async fn handle_admins(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;
    if !utils::is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, l10n!(lang, "only_owner_list_admins"))
            .await?;
        return Ok(());
    }
    let (text, markup) =
        build_admins_inline(&state.db, state.bot_id, state.client_tg_id, lang).await;
    bot.send_message(msg.chat.id, text)
        .reply_markup(markup)
        .await?;
    Ok(())
}

pub(super) async fn build_admins_inline(
    db: &Database,
    bot_id: i32,
    owner_id: i64,
    lang: Locale,
) -> (String, InlineKeyboardMarkup) {
    let admins = db.get_admins(bot_id).await.unwrap_or_default();
    let mut text = l10n!(lang, "admins_list_header", owner_id = owner_id);
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];
    for admin in &admins {
        if admin.user_id == owner_id {
            continue;
        }
        let label = if admin.frozen {
            format!("❄️ {} (ID: {})", admin.user_name, admin.user_id)
        } else {
            format!("👤 {} (ID: {})", admin.user_name, admin.user_id)
        };
        keyboard.push(vec![InlineKeyboardButton::callback(
            label,
            format!("admin_manage_{}", admin.user_id),
        )]);
    }
    if keyboard.is_empty() {
        text.push_str(&l10n!(lang, "no_admins_to_remove"));
    } else {
        text.push_str(&l10n!(lang, "admins_manage_hint"));
    }
    let markup = InlineKeyboardMarkup::new(keyboard);
    (text, markup)
}

pub(super) fn build_admin_manage_inline(
    admin: &Admin,
    lang: Locale,
) -> (String, InlineKeyboardMarkup) {
    let text = l10n!(
        lang,
        "admin_manage_title",
        name = admin.user_name,
        id = admin.user_id
    );
    let mut keyboard = vec![];
    if admin.frozen {
        keyboard.push(vec![InlineKeyboardButton::callback(
            l10n!(lang, "unfreeze_btn"),
            format!("unfreeze_admin_{}", admin.user_id),
        )]);
    } else {
        keyboard.push(vec![InlineKeyboardButton::callback(
            l10n!(lang, "freeze_btn"),
            format!("freeze_admin_{}", admin.user_id),
        )]);
    }
    keyboard.push(vec![InlineKeyboardButton::callback(
        l10n!(lang, "remove_btn"),
        format!("rmadmin_{}", admin.user_id),
    )]);
    keyboard.push(vec![InlineKeyboardButton::callback(
        l10n!(lang, "back_btn"),
        "back_to_admins",
    )]);
    let markup = InlineKeyboardMarkup::new(keyboard);
    (text, markup)
}
