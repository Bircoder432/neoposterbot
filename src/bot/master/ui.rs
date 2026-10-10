use super::MasterState;
use crate::l10n;
use crate::locales::Locale;
use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, MessageId, ParseMode};

const CLIENTS_PER_PAGE: i64 = 5;
const BOTS_PER_PAGE: i64 = 5;

type R = Result<()>;

pub(super) async fn send_clients_page(
    bot: &Bot,
    chat_id: ChatId,
    state: &Arc<MasterState>,
    page: i64,
    lang: Locale,
) -> R {
    let total_clients = state.db.count_clients().await?;
    if total_clients == 0 {
        bot.send_message(chat_id, l10n!(lang, "clients_empty"))
            .await?;
        return Ok(());
    }
    let total_pages = (total_clients + CLIENTS_PER_PAGE - 1) / CLIENTS_PER_PAGE;
    let current_page = page.max(0).min(total_pages - 1);
    let offset = current_page * CLIENTS_PER_PAGE;
    let clients = state
        .db
        .get_clients_paginated(offset, CLIENTS_PER_PAGE)
        .await?;
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];
    for c in &clients {
        let plan_str = if c.pro_expires_at.map_or(false, |d| d > chrono::Utc::now()) {
            "Pro"
        } else {
            "Free"
        };
        let status_str = if c.banned { "🚫" } else { "✅" };
        keyboard.push(vec![InlineKeyboardButton::callback(
            format!(
                "{} {} (ID: {}) - {}",
                status_str, plan_str, c.tg_user_id, c.tg_user_id
            ),
            format!("client_{}", c.tg_user_id),
        )]);
    }
    let mut nav_row = vec![];
    if current_page > 0 {
        nav_row.push(InlineKeyboardButton::callback(
            "⬅️",
            format!("clients_page_{}", current_page - 1),
        ));
    }
    if current_page < total_pages - 1 {
        nav_row.push(InlineKeyboardButton::callback(
            "➡️",
            format!("clients_page_{}", current_page + 1),
        ));
    }
    if !nav_row.is_empty() {
        keyboard.push(nav_row);
    }
    let markup = InlineKeyboardMarkup::new(keyboard);
    bot.send_message(chat_id, l10n!(lang, "clients_list_title"))
        .reply_markup(markup)
        .await?;
    Ok(())
}

pub(super) async fn send_client_details(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    state: &Arc<MasterState>,
    tg_id: i64,
    lang: Locale,
) -> R {
    let client = state.db.get_client_by_tg_id(tg_id).await?;
    let client = match client {
        Some(c) => c,
        None => {
            return Ok(());
        }
    };
    let bots = state.db.get_all_bots_by_client_tg_id(tg_id).await?;
    let is_pro = client
        .pro_expires_at
        .map_or(false, |d| d > chrono::Utc::now());
    let expires = client
        .pro_expires_at
        .map(|d| d.format("%d.%m.%Y").to_string());

    let plan_str = if is_pro {
        if let Some(date) = expires {
            l10n!(lang, "client_plan_pro_until", date = date)
        } else {
            l10n!(lang, "client_plan_pro")
        }
    } else {
        l10n!(lang, "client_plan_free")
    };
    let status = if client.banned {
        l10n!(lang, "client_status_banned")
    } else {
        l10n!(lang, "client_status_active")
    };

    let text = l10n!(
        lang,
        "client_details",
        id_label = l10n!(lang, "field_tg_id"),
        tg_id = tg_id,
        plan_label = l10n!(lang, "field_plan"),
        plan = plan_str,
        status_label = l10n!(lang, "field_status"),
        status = status,
        bots_label = l10n!(lang, "field_bots"),
        bots = bots.len()
    );

    let ban_btn_text = if client.banned {
        l10n!(lang, "unban_client_btn")
    } else {
        l10n!(lang, "ban_client_btn")
    };
    let change_plan_text = if is_pro {
        l10n!(lang, "make_free")
    } else {
        l10n!(lang, "make_pro")
    };

    let keyboard = vec![
        vec![InlineKeyboardButton::callback(
            ban_btn_text,
            format!("banclient_{}", tg_id),
        )],
        vec![InlineKeyboardButton::callback(
            change_plan_text,
            format!("changeplan_{}", tg_id),
        )],
        vec![InlineKeyboardButton::callback(
            l10n!(lang, "client_bots_btn"),
            format!("clientbots_{}", tg_id),
        )],
        vec![InlineKeyboardButton::callback(
            l10n!(lang, "back_btn"),
            format!("clients_page_0"),
        )],
    ];
    let markup = InlineKeyboardMarkup::new(keyboard);
    bot.edit_message_text(chat_id, message_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(markup)
        .await
        .ok();
    Ok(())
}

pub(super) async fn send_client_bots(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    state: &Arc<MasterState>,
    tg_id: i64,
    page: i64,
    lang: Locale,
) -> R {
    let all_bots = state.db.get_all_bots_by_client_tg_id(tg_id).await?;
    if all_bots.is_empty() {
        let kb = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
            l10n!(lang, "back_btn"),
            format!("client_{}", tg_id),
        )]]);
        bot.edit_message_text(chat_id, message_id, l10n!(lang, "no_bots_msg"))
            .reply_markup(kb)
            .await
            .ok();
        return Ok(());
    }
    let total_bots = all_bots.len() as i64;
    let total_pages = (total_bots + BOTS_PER_PAGE - 1) / BOTS_PER_PAGE;
    let current_page = page.max(0).min(total_pages - 1);
    let start = (current_page * BOTS_PER_PAGE) as usize;
    let end = (start + BOTS_PER_PAGE as usize).min(total_bots as usize);
    let bots_to_show = &all_bots[start..end];
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];
    for b in bots_to_show {
        let status = if b.active { "✅" } else { "❌" };
        keyboard.push(vec![InlineKeyboardButton::callback(
            format!("{} @{} (ID: {})", status, b.bot_username, b.id),
            format!("botinfo_{}", b.id),
        )]);
    }
    let mut nav_row = vec![];
    if current_page > 0 {
        nav_row.push(InlineKeyboardButton::callback(
            "⬅️",
            format!("clientbots_page_{}_{}", tg_id, current_page - 1),
        ));
    }
    if current_page < total_pages - 1 {
        nav_row.push(InlineKeyboardButton::callback(
            "➡️",
            format!("clientbots_page_{}_{}", tg_id, current_page + 1),
        ));
    }
    if !nav_row.is_empty() {
        keyboard.push(nav_row);
    }
    keyboard.push(vec![InlineKeyboardButton::callback(
        l10n!(lang, "back_btn"),
        format!("client_{}", tg_id),
    )]);
    let markup = InlineKeyboardMarkup::new(keyboard);
    bot.edit_message_text(chat_id, message_id, l10n!(lang, "client_bots_title"))
        .reply_markup(markup)
        .await
        .ok();
    Ok(())
}

pub(super) async fn send_bot_info(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    state: &Arc<MasterState>,
    bot_id: i32,
    lang: Locale,
) -> R {
    let pool = &state.db.pool;
    let bot_cfg: Option<crate::db::models::BotConfig> = sqlx::query_as(
        "SELECT b.*, c.tg_user_id as client_tg_id FROM bots b JOIN clients c ON b.client_id = c.id WHERE b.id = $1"
    )
    .bind(bot_id)
    .fetch_optional(pool)
    .await?;
    if let Some(b) = bot_cfg {
        let channel_display = b.channel_username.clone().unwrap_or_else(|| {
            if b.channel_id != 0 {
                format!("ID: {}", b.channel_id)
            } else {
                l10n!(lang, "channel_not_configured")
            }
        });

        let status = if b.active {
            l10n!(lang, "bot_status_active")
        } else {
            l10n!(lang, "bot_status_inactive")
        };
        let text = l10n!(
            lang,
            "bot_info",
            username_label = l10n!(lang, "field_username"),
            username = b.bot_username,
            channel_label = l10n!(lang, "field_channel"),
            channel = channel_display,
            status_label = l10n!(lang, "field_status"),
            status = status
        );

        let keyboard = vec![vec![InlineKeyboardButton::callback(
            l10n!(lang, "back_btn"),
            format!("clientbots_{}_0", b.client_tg_id.unwrap_or(0)),
        )]];
        let markup = InlineKeyboardMarkup::new(keyboard);
        bot.edit_message_text(chat_id, message_id, text)
            .parse_mode(ParseMode::Html)
            .reply_markup(markup)
            .await
            .ok();
    }
    Ok(())
}
