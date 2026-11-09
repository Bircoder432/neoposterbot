use super::MasterState;
use crate::db::models::PromoCode;
use crate::l10n;
use crate::locales::Locale;
use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, MessageId, ParseMode};

const CLIENTS_PER_PAGE: i64 = 5;
const BOTS_PER_PAGE: i64 = 10;
const PROMOS_PER_PAGE: i64 = 5;

type R = Result<()>;

/// Final price after applying a pending-discount percent (capped at 99%).
pub(super) fn discounted_price(price: i64, discount: i32) -> i64 {
    let d = discount.clamp(0, 99) as f64;
    (((price as f64) * (100.0 - d) / 100.0).round() as i64).max(1)
}

/// Short human label for a promo code: "🎁 PRO 30d" / "💰 −25%"
fn promo_type_short(p: &PromoCode, lang: Locale) -> String {
    if p.is_free_pro() {
        l10n!(lang, "master_promo_short_free", days = p.param)
    } else {
        l10n!(lang, "master_promo_short_disc", percent = p.param)
    }
}

fn promo_uses_str(p: &PromoCode) -> String {
    if p.max_uses == 0 {
        format!("{}/∞", p.used_count)
    } else {
        format!("{}/{}", p.used_count, p.max_uses)
    }
}

async fn edit_or_send(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    text: String,
    markup: InlineKeyboardMarkup,
) -> R {
    match message_id {
        Some(mid) => {
            bot.edit_message_text(chat_id, mid, text)
                .parse_mode(ParseMode::Html)
                .reply_markup(markup)
                .await
                .ok();
        }
        None => {
            bot.send_message(chat_id, text)
                .parse_mode(ParseMode::Html)
                .reply_markup(markup)
                .await?;
        }
    }
    Ok(())
}

// ==================== Owner panel ====================

pub(super) async fn show_owner_menu(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &Arc<MasterState>,
) -> R {
    let lang = super::get_locale(state, state.config.owner_id).await;
    let clients = state.db.count_clients().await.unwrap_or(0);
    let promos = state.db.count_promo_codes().await.unwrap_or(0);
    let banned = state.db.count_banned_clients().await.unwrap_or(0);
    let text = l10n!(lang, "master_owner_menu", clients = clients, promos = promos);
    let keyboard = vec![
        vec![
            InlineKeyboardButton::callback(l10n!(lang, "mbtn_clients"), "owner_clients"),
            InlineKeyboardButton::callback(l10n!(lang, "mbtn_bots"), "owner_bots"),
        ],
        vec![
            InlineKeyboardButton::callback(
                format!("{} · {}", l10n!(lang, "mbtn_banned"), banned),
                "owner_banned",
            ),
            InlineKeyboardButton::callback(l10n!(lang, "mbtn_stats"), "owner_stats"),
        ],
        vec![
            InlineKeyboardButton::callback(l10n!(lang, "mbtn_broadcast"), "owner_broadcast"),
            InlineKeyboardButton::callback(l10n!(lang, "mbtn_promo"), "owner_promo"),
        ],
        vec![
            InlineKeyboardButton::callback(l10n!(lang, "mbtn_proset"), "owner_proset"),
            InlineKeyboardButton::callback(l10n!(lang, "mbtn_cabinet"), "owner_cabinet"),
        ],
        vec![
            InlineKeyboardButton::callback(l10n!(lang, "mbtn_lang"), "owner_lang"),
            InlineKeyboardButton::callback(l10n!(lang, "mbtn_help"), "owner_help"),
        ],
    ];
    edit_or_send(bot, chat_id, message_id, text, InlineKeyboardMarkup::new(keyboard)).await
}

pub(super) async fn show_stats(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &Arc<MasterState>,
) -> R {
    let lang = super::get_locale(state, state.config.owner_id).await;
    let clients = state.db.count_clients().await.unwrap_or(0);
    let bots = state.db.get_all_bots().await?.len() as i64;
    let (pro_count, free_count) = state.db.get_stats().await.unwrap_or((0, 0));
    let promos_total = state.db.count_promo_codes().await.unwrap_or(0);
    let promos_active = state.db.count_active_promos().await.unwrap_or(0);
    let text = l10n!(
        lang,
        "master_stats_text",
        clients = clients,
        bots = bots,
        pro = pro_count,
        free = free_count,
        promos = promos_total,
        active = promos_active
    );
    let keyboard = vec![
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_refresh"), "owner_stats")],
        vec![InlineKeyboardButton::callback(l10n!(lang, "back_to_menu_btn"), "owner_menu")],
    ];
    edit_or_send(bot, chat_id, message_id, text, InlineKeyboardMarkup::new(keyboard)).await
}

pub(super) async fn show_help(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    lang: Locale,
) -> R {
    let text = l10n!(lang, "master_help");
    let keyboard = vec![vec![InlineKeyboardButton::callback(
        l10n!(lang, "back_to_menu_btn"),
        "owner_menu",
    )]];
    edit_or_send(bot, chat_id, message_id, text, InlineKeyboardMarkup::new(keyboard)).await
}

// ==================== PRO settings ====================

pub(super) async fn show_pro_settings(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &Arc<MasterState>,
) -> R {
    let lang = super::get_locale(state, state.config.owner_id).await;
    let days = state.db.get_pro_days().await?;
    let price = state.db.get_pro_price().await?;
    let text = l10n!(lang, "master_proset_title", days = days, price = price);
    let keyboard = vec![
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_pro_days"), "set_prodays")],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_pro_price"), "set_proprice")],
        vec![InlineKeyboardButton::callback(l10n!(lang, "back_to_menu_btn"), "owner_menu")],
    ];
    edit_or_send(bot, chat_id, message_id, text, InlineKeyboardMarkup::new(keyboard)).await
}

pub(super) async fn show_pro_days_chooser(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    lang: Locale,
) -> R {
    let text = l10n!(lang, "master_prodays_title");
    let keyboard = vec![
        vec![
            InlineKeyboardButton::callback("7", "prodays_7"),
            InlineKeyboardButton::callback("14", "prodays_14"),
            InlineKeyboardButton::callback("30", "prodays_30"),
            InlineKeyboardButton::callback("90", "prodays_90"),
            InlineKeyboardButton::callback("180", "prodays_180"),
        ],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_custom"), "prodays_custom")],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_cancel"), "owner_proset")],
    ];
    edit_or_send(bot, chat_id, message_id, text, InlineKeyboardMarkup::new(keyboard)).await
}

pub(super) async fn show_pro_price_chooser(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    lang: Locale,
) -> R {
    let text = l10n!(lang, "master_proprice_title");
    let keyboard = vec![
        vec![
            InlineKeyboardButton::callback("50", "proprice_50"),
            InlineKeyboardButton::callback("100", "proprice_100"),
            InlineKeyboardButton::callback("200", "proprice_200"),
            InlineKeyboardButton::callback("500", "proprice_500"),
        ],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_custom"), "proprice_custom")],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_cancel"), "owner_proset")],
    ];
    edit_or_send(bot, chat_id, message_id, text, InlineKeyboardMarkup::new(keyboard)).await
}

// ==================== Promo codes ====================

pub(super) async fn show_promo_list(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &Arc<MasterState>,
    page: i64,
) -> R {
    let lang = super::get_locale(state, state.config.owner_id).await;
    let total = state.db.count_promo_codes().await?;
    let text;
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

    if total == 0 {
        text = l10n!(lang, "master_promo_empty");
    } else {
        let total_pages = (total + PROMOS_PER_PAGE - 1) / PROMOS_PER_PAGE;
        let current_page = page.max(0).min(total_pages - 1);
        let offset = current_page * PROMOS_PER_PAGE;
        let promos = state
            .db
            .get_promo_codes_paginated(offset, PROMOS_PER_PAGE)
            .await?;

        text = l10n!(
            lang,
            "master_promo_title",
            page = current_page + 1,
            total = total_pages
        );
        for p in &promos {
            let status = if p.active { "✅" } else { "⏸" };
            keyboard.push(vec![InlineKeyboardButton::callback(
                format!("{} {} · {} · {}", status, p.code, promo_type_short(p, lang), promo_uses_str(p)),
                format!("promo_view_{}", p.id),
            )]);
        }
        let mut nav_row = vec![];
        if current_page > 0 {
            nav_row.push(InlineKeyboardButton::callback(
                "⬅️",
                format!("promo_page_{}", current_page - 1),
            ));
        }
        if current_page < total_pages - 1 {
            nav_row.push(InlineKeyboardButton::callback(
                "➡️",
                format!("promo_page_{}", current_page + 1),
            ));
        }
        if !nav_row.is_empty() {
            keyboard.push(nav_row);
        }
    }

    keyboard.push(vec![InlineKeyboardButton::callback(
        l10n!(lang, "mbtn_promo_create"),
        "promo_create",
    )]);
    keyboard.push(vec![InlineKeyboardButton::callback(
        l10n!(lang, "back_to_menu_btn"),
        "owner_menu",
    )]);
    edit_or_send(bot, chat_id, message_id, text, InlineKeyboardMarkup::new(keyboard)).await
}

pub(super) async fn show_promo_details(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    state: &Arc<MasterState>,
    promo_id: i32,
) -> R {
    let lang = super::get_locale(state, state.config.owner_id).await;
    let promo = match state.db.get_promo_by_id(promo_id).await? {
        Some(p) => p,
        None => {
            let kb = InlineKeyboardMarkup::new(vec![vec![
                InlineKeyboardButton::callback(l10n!(lang, "mbtn_to_list"), "promo_page_0"),
                InlineKeyboardButton::callback(l10n!(lang, "back_to_menu_btn"), "owner_menu"),
            ]]);
            bot.edit_message_text(chat_id, message_id, l10n!(lang, "client_not_found"))
                .reply_markup(kb)
                .await
                .ok();
            return Ok(());
        }
    };
    let type_str = if promo.is_free_pro() {
        l10n!(lang, "master_promo_type_free", days = promo.param)
    } else {
        l10n!(lang, "master_promo_type_disc", percent = promo.param)
    };
    let uses_str = if promo.max_uses == 0 {
        l10n!(lang, "master_promo_uses_of_unlimited", used = promo.used_count)
    } else {
        l10n!(lang, "master_promo_uses_of", used = promo.used_count, max = promo.max_uses)
    };
    let status_str = if promo.active {
        l10n!(lang, "master_promo_active")
    } else {
        l10n!(lang, "master_promo_inactive")
    };
    let created = promo.created_at.format("%d.%m.%Y");
    let text = l10n!(
        lang,
        "master_promo_details",
        code = promo.code,
        type = type_str,
        uses = uses_str,
        status = status_str,
        created = created
    );
    let toggle_label = if promo.active {
        l10n!(lang, "master_promo_deactivate")
    } else {
        l10n!(lang, "master_promo_activate")
    };
    let keyboard = vec![
        vec![InlineKeyboardButton::callback(toggle_label, format!("promo_toggle_{}", promo.id))],
        vec![InlineKeyboardButton::callback(l10n!(lang, "remove_btn"), format!("promo_del_{}", promo.id))],
        vec![
            InlineKeyboardButton::callback(l10n!(lang, "mbtn_to_list"), "promo_page_0"),
            InlineKeyboardButton::callback(l10n!(lang, "back_to_menu_btn"), "owner_menu"),
        ],
    ];
    edit_or_send(bot, chat_id, Some(message_id), text, InlineKeyboardMarkup::new(keyboard)).await
}

pub(super) async fn show_promo_delete_confirm(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    promo: &PromoCode,
    lang: Locale,
) -> R {
    let text = l10n!(lang, "master_promo_del_confirm", code = promo.code);
    let keyboard = vec![
        vec![InlineKeyboardButton::callback(
            l10n!(lang, "mbtn_delete_yes"),
            format!("promo_delok_{}", promo.id),
        )],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_cancel"), format!("promo_view_{}", promo.id))],
    ];
    edit_or_send(bot, chat_id, Some(message_id), text, InlineKeyboardMarkup::new(keyboard)).await
}

pub(super) async fn show_promo_type_choice(bot: &Bot, chat_id: ChatId, lang: Locale) -> R {
    let text = l10n!(lang, "master_promo_create_title");
    let keyboard = vec![
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_promo_kfree"), "promo_kfree")],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_promo_kdisc"), "promo_kdisc")],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_cancel"), "owner_promo")],
    ];
    let markup = InlineKeyboardMarkup::new(keyboard);
    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(markup)
        .await?;
    Ok(())
}

pub(super) async fn show_promo_days_chooser(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    lang: Locale,
) -> R {
    let text = l10n!(lang, "master_promo_days_title");
    let keyboard = vec![
        vec![
            InlineKeyboardButton::callback("7", "promo_d_7"),
            InlineKeyboardButton::callback("14", "promo_d_14"),
            InlineKeyboardButton::callback("30", "promo_d_30"),
            InlineKeyboardButton::callback("90", "promo_d_90"),
        ],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_custom"), "promo_d_custom")],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_cancel"), "promo_page_0")],
    ];
    edit_or_send(bot, chat_id, message_id, text, InlineKeyboardMarkup::new(keyboard)).await
}

pub(super) async fn show_promo_percent_chooser(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    lang: Locale,
) -> R {
    let text = l10n!(lang, "master_promo_percent_title");
    let keyboard = vec![
        vec![
            InlineKeyboardButton::callback("10%", "promo_p_10"),
            InlineKeyboardButton::callback("25%", "promo_p_25"),
            InlineKeyboardButton::callback("50%", "promo_p_50"),
            InlineKeyboardButton::callback("75%", "promo_p_75"),
        ],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_custom"), "promo_p_custom")],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_cancel"), "promo_page_0")],
    ];
    edit_or_send(bot, chat_id, message_id, text, InlineKeyboardMarkup::new(keyboard)).await
}

/// `kind` is the short tag: "free" (param = days) | "disc" (param = percent).
pub(super) async fn show_promo_uses_chooser(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    kind: &str,
    param: i32,
    lang: Locale,
) -> R {
    let kind_tag = if kind == "free" {
        l10n!(lang, "master_promo_type_free", days = param)
    } else {
        l10n!(lang, "master_promo_type_disc", percent = param)
    };
    let text = l10n!(lang, "master_promo_uses_title", kind = kind_tag);
    let keyboard = vec![
        vec![
            InlineKeyboardButton::callback("1", format!("promo_u_1_{}_{}", kind, param)),
            InlineKeyboardButton::callback("5", format!("promo_u_5_{}_{}", kind, param)),
            InlineKeyboardButton::callback("10", format!("promo_u_10_{}_{}", kind, param)),
        ],
        vec![InlineKeyboardButton::callback(
            l10n!(lang, "mbtn_unlimited"),
            format!("promo_u_0_{}_{}", kind, param),
        )],
        vec![InlineKeyboardButton::callback(
            l10n!(lang, "mbtn_custom"),
            format!("promo_u_custom_{}_{}", kind, param),
        )],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_cancel"), "promo_page_0")],
    ];
    edit_or_send(bot, chat_id, message_id, text, InlineKeyboardMarkup::new(keyboard)).await
}

// ==================== Client cabinet ====================

/// Shared client-facing keyboard (used by /start and the owner's cabinet).
pub(super) async fn build_client_keyboard(
    state: &Arc<MasterState>,
    user_id: i64,
) -> Result<InlineKeyboardMarkup> {
    let lang = super::get_locale(state, user_id).await;
    let client = state.db.ensure_client(user_id).await?;
    let is_pro = client
        .pro_expires_at
        .map_or(false, |d| d > chrono::Utc::now());
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![
        vec![
            InlineKeyboardButton::callback(l10n!(lang, "mk_add_bot"), "add_bot"),
            InlineKeyboardButton::callback(l10n!(lang, "mk_my_bots"), "my_bots"),
        ],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mk_check_bot"), "check_bot")],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mk_redeem_promo"), "redeem_promo")],
    ];
    if is_pro {
        keyboard.push(vec![InlineKeyboardButton::callback(
            l10n!(lang, "mk_pro_active"),
            "pro_active_info",
        )]);
    } else {
        let price = state.db.get_pro_price().await?;
        let discount = state
            .db
            .get_pending_discount(user_id)
            .await?
            .map(|d| d.discount_percent)
            .unwrap_or(0)
            .min(99);
        let label = if discount > 0 {
            l10n!(lang, "mk_buy_pro_disc", discount = discount, price = discounted_price(price, discount))
        } else {
            l10n!(lang, "mk_buy_pro", price = price)
        };
        keyboard.push(vec![InlineKeyboardButton::callback(label, "buy_pro")]);
    }
    keyboard.push(vec![InlineKeyboardButton::url(
        l10n!(lang, "mk_instruction"),
        state.config.instruction_url.clone(),
    )]);
    keyboard.push(vec![InlineKeyboardButton::callback(
        l10n!(lang, "mbtn_lang"),
        "mlang",
    )]);
    Ok(InlineKeyboardMarkup::new(keyboard))
}

/// Renders the client cabinet onto an existing message (or sends a new one
/// when `message_id` is None).
pub(super) async fn render_cabinet(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &Arc<MasterState>,
    user_id: i64,
) -> R {
    let lang = super::get_locale(state, user_id).await;
    let kb = build_client_keyboard(state, user_id).await?;
    edit_or_send(
        bot,
        chat_id,
        message_id,
        l10n!(lang, "master_cabinet_title"),
        kb,
    )
    .await
}

pub(super) async fn send_client_cabinet(
    bot: &Bot,
    chat_id: ChatId,
    state: &Arc<MasterState>,
    user_id: i64,
) -> R {
    render_cabinet(bot, chat_id, None, state, user_id).await
}

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
            format!("{} {} · ID: {}", status_str, plan_str, c.tg_user_id),
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

pub(super) async fn send_bots_page(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &Arc<MasterState>,
    page: i64,
    lang: Locale,
) -> R {
    let all_bots = state.db.get_all_bots_with_clients().await?;
    if all_bots.is_empty() {
        let text = l10n!(lang, "bots_list_empty");
        if let Some(mid) = message_id {
            bot.edit_message_text(chat_id, mid, text).await.ok();
        } else {
            bot.send_message(chat_id, text).await?;
        }
        return Ok(());
    }

    let total = all_bots.len() as i64;
    let total_pages = (total + BOTS_PER_PAGE - 1) / BOTS_PER_PAGE;
    let current_page = page.max(0).min(total_pages - 1);
    let start = (current_page * BOTS_PER_PAGE) as usize;
    let end = (start + BOTS_PER_PAGE as usize).min(total as usize);

    let mut text = l10n!(lang, "bots_list_title");
    for (i, b) in all_bots[start..end].iter().enumerate() {
        let status = if b.active {
            l10n!(lang, "bot_status_active_text")
        } else {
            l10n!(lang, "bot_status_inactive_text")
        };
        text.push_str(&l10n!(
            lang,
            "bots_list_entry",
            i = start + i + 1,
            username = b.bot_username,
            bot_id = b.id,
            client_id = b.client_tg_id.unwrap_or(0),
            status = status
        ));
    }

    let mut keyboard = vec![];
    let mut nav_row = vec![];
    if current_page > 0 {
        nav_row.push(InlineKeyboardButton::callback(
            "⬅️",
            format!("bots_page_{}", current_page - 1),
        ));
    }
    if current_page < total_pages - 1 {
        nav_row.push(InlineKeyboardButton::callback(
            "➡️",
            format!("bots_page_{}", current_page + 1),
        ));
    }
    if !nav_row.is_empty() {
        keyboard.push(nav_row);
    }

    let markup = InlineKeyboardMarkup::new(keyboard);
    if let Some(mid) = message_id {
        bot.edit_message_text(chat_id, mid, text)
            .parse_mode(ParseMode::Html)
            .reply_markup(markup)
            .await
            .ok();
    } else {
        bot.send_message(chat_id, text)
            .parse_mode(ParseMode::Html)
            .reply_markup(markup)
            .await?;
    }
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
    let client = match state.db.get_client_by_tg_id(tg_id).await? {
        Some(c) => c,
        None => return Ok(()),
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
            "clients_page_0",
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
    ).bind(bot_id).fetch_optional(pool).await?;

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

// ==================== Banned clients (quick unban) ====================

/// Merged ban/unban menu for the service owner: every banned client is
/// listed with a one-tap unban button, so unbanning no longer requires
/// digging through the full client list.
pub(super) async fn show_banned_clients(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &Arc<MasterState>,
    page: i64,
) -> R {
    let lang = super::get_locale(state, state.config.owner_id).await;
    let total = state.db.count_banned_clients().await?;
    if total == 0 {
        let kb = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
            l10n!(lang, "back_to_menu_btn"),
            "owner_menu",
        )]]);
        return edit_or_send(bot, chat_id, message_id, l10n!(lang, "master_banned_empty"), kb).await;
    }
    let total_pages = (total + CLIENTS_PER_PAGE - 1) / CLIENTS_PER_PAGE;
    let current_page = page.max(0).min(total_pages - 1);
    let offset = current_page * CLIENTS_PER_PAGE;
    let clients = state
        .db
        .get_banned_clients_paginated(offset, CLIENTS_PER_PAGE)
        .await?;

    let text = l10n!(
        lang,
        "master_banned_title",
        page = current_page + 1,
        total = total_pages
    );
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];
    for c in &clients {
        let plan_str = if c.pro_expires_at.map_or(false, |d| d > chrono::Utc::now()) {
            "Pro"
        } else {
            "Free"
        };
        keyboard.push(vec![InlineKeyboardButton::callback(
            format!("🔓 {} · {}", c.tg_user_id, plan_str),
            format!("unbanclient_{}", c.tg_user_id),
        )]);
    }
    let mut nav_row = vec![];
    if current_page > 0 {
        nav_row.push(InlineKeyboardButton::callback(
            "⬅️",
            format!("bannedpage_{}", current_page - 1),
        ));
    }
    if current_page < total_pages - 1 {
        nav_row.push(InlineKeyboardButton::callback(
            "➡️",
            format!("bannedpage_{}", current_page + 1),
        ));
    }
    if !nav_row.is_empty() {
        keyboard.push(nav_row);
    }
    keyboard.push(vec![InlineKeyboardButton::callback(
        l10n!(lang, "back_to_menu_btn"),
        "owner_menu",
    )]);
    edit_or_send(bot, chat_id, message_id, text, InlineKeyboardMarkup::new(keyboard)).await
}

// ==================== Language chooser ====================

/// Language picker shared by the owner menu and the client cabinet.
/// `prefix` selects the callback data ("owner_lang" or "mlang"),
/// `back_data` is the callback the back button returns to.
pub(super) async fn show_lang_chooser(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    lang: Locale,
    prefix: &str,
    back_data: &str,
) -> R {
    let keyboard = vec![
        vec![
            InlineKeyboardButton::callback("🇷🇺 Русский", format!("{prefix}_ru")),
            InlineKeyboardButton::callback("🇬🇧 English", format!("{prefix}_en")),
        ],
        vec![InlineKeyboardButton::callback(l10n!(lang, "back_btn"), back_data)],
    ];
    edit_or_send(
        bot,
        chat_id,
        message_id,
        l10n!(lang, "master_lang_title"),
        InlineKeyboardMarkup::new(keyboard),
    )
    .await
}
