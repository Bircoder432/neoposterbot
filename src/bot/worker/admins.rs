use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

use super::utils;
use crate::bot::worker::WorkerState;
use crate::db::models::Admin;
use crate::db::repository::Database;
use crate::locales::{L10n, Locale};

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
        bot.send_message(chat_id, L10n::admin_invite_already_admin(lang))
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
                bot.send_message(chat_id, L10n::admin_added_frozen_notification(lang))
                    .await?;
            } else {
                bot.send_message(chat_id, L10n::admin_added_notification(lang))
                    .await?;
            }

            // Notify the owner
            let owner_msg = if frozen {
                L10n::admin_added_frozen(lang, &user_name)
            } else {
                L10n::admin_added(lang, &user_name)
            };
            let _ = bot
                .send_message(ChatId(state.client_tg_id), owner_msg)
                .await;
        }
        _ => {
            bot.send_message(chat_id, L10n::admin_invite_invalid(lang))
                .await?;
        }
    }
    Ok(())
}

pub(super) async fn handle_add_admin(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;

    if !utils::is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, L10n::only_owner_add_admins(lang))
            .await?;
        return Ok(());
    }

    let code = state.db.create_admin_invite(state.bot_id).await?;

    let cfg = state.config.read().await;
    let bot_username = cfg.bot_username.clone();
    drop(cfg);

    let link = format!("https://t.me/{bot_username}?start=admin_{code}");

    let kb = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        L10n::revoke_invite_btn(lang),
        format!("revoke_invite_{}", code),
    )]]);

    bot.send_message(msg.chat.id, L10n::admin_invite_link(lang, &link))
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(kb)
        .await?;

    Ok(())
}

pub(super) async fn handle_admins(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let lang = state.db.get_language(state.bot_id).await?;

    if !utils::is_authorized(state, user_id).await? {
        bot.send_message(msg.chat.id, L10n::only_owner_list_admins(lang))
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
    let mut text = L10n::admins_list_header(lang, owner_id);
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
        text.push_str(L10n::no_admins_to_remove(lang));
    } else {
        text.push_str(L10n::admins_manage_hint(lang));
    }

    let markup = InlineKeyboardMarkup::new(keyboard);
    (text, markup)
}

pub(super) fn build_admin_manage_inline(
    admin: &Admin,
    lang: Locale,
) -> (String, InlineKeyboardMarkup) {
    let text = L10n::admin_manage_title(lang, &admin.user_name, admin.user_id);
    let mut keyboard = vec![];

    if admin.frozen {
        keyboard.push(vec![InlineKeyboardButton::callback(
            L10n::unfreeze_btn(lang),
            format!("unfreeze_admin_{}", admin.user_id),
        )]);
    } else {
        keyboard.push(vec![InlineKeyboardButton::callback(
            L10n::freeze_btn(lang),
            format!("freeze_admin_{}", admin.user_id),
        )]);
    }

    keyboard.push(vec![InlineKeyboardButton::callback(
        L10n::remove_btn(lang),
        format!("rmadmin_{}", admin.user_id),
    )]);
    keyboard.push(vec![InlineKeyboardButton::callback(
        L10n::back_btn(lang),
        "back_to_admins",
    )]);

    let markup = InlineKeyboardMarkup::new(keyboard);
    (text, markup)
}
