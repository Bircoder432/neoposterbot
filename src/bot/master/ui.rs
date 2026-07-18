use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, MessageId, ParseMode};

use super::MasterState;
use crate::locales::{L10n, Locale};

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
        bot.send_message(chat_id, L10n::clients_empty(lang)).await?;
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

    // Pagination buttons
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
    bot.send_message(chat_id, L10n::clients_list_title(lang))
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
    let plan = if is_pro { "pro" } else { "free" };
    let expires = client
        .pro_expires_at
        .map(|d| d.format("%d.%m.%Y").to_string());

    let text = L10n::client_details(
        lang,
        tg_id,
        plan,
        expires.as_deref(),
        client.banned,
        bots.len(),
    );

    let ban_btn_text = if client.banned {
        L10n::unban_client_btn(lang)
    } else {
        L10n::ban_client_btn(lang)
    };

    let change_plan_text = if is_pro {
        L10n::make_free(lang)
    } else {
        L10n::make_pro(lang)
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
            L10n::client_bots_btn(lang),
            format!("clientbots_{}", tg_id),
        )],
        vec![InlineKeyboardButton::callback(
            L10n::back_btn(lang),
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
            L10n::back_btn(lang),
            format!("client_{}", tg_id),
        )]]);
        bot.edit_message_text(chat_id, message_id, L10n::no_bots_msg(lang))
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

    // Pagination
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

    // Back button
    keyboard.push(vec![InlineKeyboardButton::callback(
        L10n::back_btn(lang),
        format!("client_{}", tg_id),
    )]);

    let markup = InlineKeyboardMarkup::new(keyboard);
    bot.edit_message_text(chat_id, message_id, L10n::client_bots_title(lang))
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
    // Since we don't have a direct get_bot_by_id, we can query all bots and find it.
    // In a real app, add get_bot_by_id to repository.
    // For simplicity, let's use a raw query or iterate.
    // Let's add a quick raw query here.
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
                L10n::channel_not_configured(lang).to_string()
            }
        });

        let text = L10n::bot_info_text(lang, &b.bot_username, &channel_display, b.active);

        let keyboard = vec![vec![InlineKeyboardButton::callback(
            L10n::back_btn(lang),
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
