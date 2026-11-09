use super::ui;
use super::{FlowState, MasterState};
use crate::l10n;
use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};

type R = Result<()>;

pub(super) async fn process_master_message(
    bot: &Bot,
    msg: &Message,
    state: &Arc<MasterState>,
) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let text = msg.text().unwrap_or("");
    let chat_id = msg.chat.id;
    let lang = super::get_locale(state, user_id).await;

    if let Some(payment) = msg.successful_payment() {
        if payment.invoice_payload == "pro_sub_1m" {
            let days = state.db.get_pro_days().await?;
            let until = state.db.upgrade_to_pro(user_id).await.ok().flatten();
            let _ = state.db.clear_pending_discount(user_id).await;
            let until_str = until
                .map(|d| l10n!(lang, "master_pro_until_suffix", date = d.format("%d.%m.%Y")))
                .unwrap_or_default();
            bot.send_message(
                chat_id,
                l10n!(lang, "master_payment_success", days = days, until = until_str),
            )
            .await?;
        }
        return Ok(());
    }

    if text.starts_with("/check") {
        return handle_check_command(bot, chat_id, text, state, lang).await;
    }

    if text.starts_with("/cancel") {
        if state.flow_states.remove(&user_id).is_some() {
            bot.send_message(chat_id, l10n!(lang, "flow_cancelled_msg"))
                .await?;
        } else {
            bot.send_message(chat_id, l10n!(lang, "no_active_mode"))
                .await?;
        }
        return Ok(());
    }

    // Language selection for every user (the owner and clients alike).
    if text.starts_with("/lang") {
        let arg = text.splitn(2, char::is_whitespace).nth(1).unwrap_or("").trim();
        match crate::locales::Locale::parse(arg) {
            Some(new_lang) => {
                state.db.set_client_lang_safe(user_id, new_lang).await?;
                bot.send_message(chat_id, l10n!(new_lang, "lang_updated"))
                    .await?;
            }
            None => {
                let kb = InlineKeyboardMarkup::new(vec![vec![
                    InlineKeyboardButton::callback("🇷🇺 Русский", "mlang_ru"),
                    InlineKeyboardButton::callback("🇬🇧 English", "mlang_en"),
                ]]);
                bot.send_message(chat_id, l10n!(lang, "master_lang_title"))
                    .reply_markup(kb)
                    .await?;
            }
        }
        return Ok(());
    }

    if user_id == state.config.owner_id {
        if text.starts_with("/help") {
            bot.send_message(chat_id, l10n!(lang, "master_help"))
                .await?;
            return Ok(());
        } else if text.starts_with("/menu") {
            ui::show_owner_menu(bot, chat_id, None, state).await?;
            return Ok(());
        } else if text.starts_with("/clients") {
            ui::send_clients_page(bot, chat_id, state, 0, lang).await?;
            return Ok(());
        } else if text.starts_with("/bots") {
            ui::send_bots_page(bot, chat_id, None, state, 0, lang).await?;
            return Ok(());
        } else if text.starts_with("/broadcast") {
            state
                .flow_states
                .insert(user_id, FlowState::AwaitingBroadcast);
            bot.send_message(chat_id, l10n!(lang, "broadcast_prompt"))
                .await?;
            return Ok(());
        } else if text.starts_with("/stats") {
            let clients = state.db.count_clients().await.unwrap_or(0);
            let bots = state.db.get_all_bots().await?.len() as i64;
            let (pro_count, free_count) = state.db.get_stats().await.unwrap_or((0, 0));
            let promos_total = state.db.count_promo_codes().await.unwrap_or(0);
            let promos_active = state.db.count_active_promos().await.unwrap_or(0);
            bot.send_message(
                chat_id,
                l10n!(
                    lang,
                    "master_stats_text",
                    clients = clients,
                    bots = bots,
                    pro = pro_count,
                    free = free_count,
                    promos = promos_total,
                    active = promos_active
                ),
            )
            .await?;
            return Ok(());
        } else if text.starts_with("/setplan") {
            let parts: Vec<&str> = text.splitn(3, ' ').collect();
            if parts.len() == 3 {
                let tg_id: i64 = parts[1].parse().unwrap_or(0);
                let plan = parts[2];
                let stopped = state.db.set_client_plan(tg_id, plan).await?;
                for id in &stopped {
                    state.manager.stop_worker(*id).await;
                }
                let mut reply = l10n!(lang, "master_setplan_ok", id = tg_id, plan = plan);
                if !stopped.is_empty() {
                    reply.push_str(&l10n!(
                        lang,
                        "master_bots_stopped_note",
                        count = stopped.len()
                    ));
                }
                bot.send_message(chat_id, reply).await?;
            } else {
                bot.send_message(chat_id, l10n!(lang, "master_setplan_usage"))
                    .await?;
            }
            return Ok(());
        } else if text.starts_with("/banclient") {
            let parts: Vec<&str> = text.splitn(2, ' ').collect();
            if parts.len() == 2 {
                let tg_id: i64 = parts[1].parse().unwrap_or(0);
                let bot_ids = state.db.ban_client(tg_id).await?;
                for bid in bot_ids {
                    state.manager.stop_worker(bid).await;
                }
                bot.send_message(
                    chat_id,
                    l10n!(lang, "master_banclient_ok", id = tg_id),
                )
                .await?;
            }
            return Ok(());
        }
    }

    if text.starts_with("/start") {
        if user_id == state.config.owner_id {
            ui::show_owner_menu(bot, chat_id, None, state).await?;
            return Ok(());
        }
        let kb = ui::build_client_keyboard(state, user_id).await?;
        bot.send_message(chat_id, l10n!(lang, "master_welcome"))
            .reply_markup(kb)
            .await?;
    } else {
        let flow = state.flow_states.get(&user_id).map(|f| f.clone());
        if let Some(flow) = flow {
            match flow {
                FlowState::AwaitingToken => {
                    let token = text.trim().to_string();
                    match teloxide::Bot::new(&token).get_me().await {
                        Ok(me) => {
                            let username = me.username().to_string();
                            // Free-plan limit: one active bot per client.
                            // Re-linking an already active own bot changes nothing.
                            let client = state.db.get_client_by_tg_id(user_id).await?;
                            let is_pro = client
                                .map(|c| {
                                    c.pro_expires_at
                                        .map_or(false, |d| d > chrono::Utc::now())
                                })
                                .unwrap_or(false);
                            if !is_pro {
                                let existing = state.db.find_bot_by_token(&token).await?;
                                let adds_new_bot = match &existing {
                                    None => true,
                                    Some(b) if b.client_tg_id == Some(user_id) => !b.active,
                                    // Someone else's token: let add_bot report its own error.
                                    Some(_) => false,
                                };
                                if adds_new_bot {
                                    let active =
                                        state.db.count_active_bots_by_client(user_id).await?;
                                    if active >= crate::db::repository::FREE_ACTIVE_BOTS {
                                        state.flow_states.remove(&user_id);
                                        bot.send_message(chat_id, l10n!(lang, "master_bot_limit"))
                                            .parse_mode(ParseMode::Html)
                                            .reply_markup(InlineKeyboardMarkup::new(vec![
                                                vec![InlineKeyboardButton::callback(
                                                    l10n!(
                                                        lang,
                                                        "mk_buy_pro",
                                                        price = state.db.get_pro_price().await?
                                                    ),
                                                    "buy_pro",
                                                )],
                                                vec![InlineKeyboardButton::callback(
                                                    l10n!(lang, "mk_redeem_promo"),
                                                    "redeem_promo",
                                                )],
                                            ]))
                                            .await?;
                                        return Ok(());
                                    }
                                }
                            }
                            match state.db.add_bot(user_id, &token, &username).await {
                                Ok(bot_cfg) => {
                                    let _ = state
                                        .db
                                        .add_admin(bot_cfg.id, user_id, "Owner", false)
                                        .await;
                                    let mut bot_cfg_with_id = bot_cfg.clone();
                                    bot_cfg_with_id.client_tg_id = Some(user_id);
                                    state.manager.start_worker(bot_cfg_with_id).await;
                                    bot.send_message(
                                        chat_id,
                                        l10n!(lang, "master_bot_saved", username = username),
                                    )
                                    .await?;
                                    state.flow_states.remove(&user_id);
                                }
                                Err(e) => {
                                    bot.send_message(
                                        chat_id,
                                        l10n!(lang, "master_bot_save_error", e = e),
                                    )
                                    .await?;
                                }
                            }
                        }
                        Err(_) => {
                            bot.send_message(chat_id, l10n!(lang, "master_token_invalid"))
                                .await?;
                        }
                    }
                }
                FlowState::AwaitingBotCheck => {
                    let query = text.trim();
                    if query.is_empty() || query.starts_with('/') {
                        bot.send_message(chat_id, l10n!(lang, "master_check_invalid_input"))
                            .await?;
                        return Ok(());
                    }
                    perform_bot_check(bot, chat_id, query, state, lang).await?;
                    state.flow_states.remove(&user_id);
                }
                FlowState::AwaitingBroadcast => {
                    if user_id == state.config.owner_id {
                        let clients = state.db.get_clients_with_bots().await?;
                        if clients.is_empty() {
                            bot.send_message(chat_id, l10n!(lang, "broadcast_no_clients"))
                                .await?;
                            state.flow_states.remove(&user_id);
                            return Ok(());
                        }
                        bot.send_message(chat_id, l10n!(lang, "broadcast_started"))
                            .await?;
                        let broadcast_text = text.to_string();
                        let mut sent = 0u64;
                        let mut failed = 0u64;
                        for client in &clients {
                            match bot
                                .send_message(ChatId(client.tg_user_id), &broadcast_text)
                                .await
                            {
                                Ok(_) => sent += 1,
                                Err(_) => failed += 1,
                            }
                        }
                        state.flow_states.remove(&user_id);
                        bot.send_message(
                            chat_id,
                            l10n!(lang, "broadcast_finished", sent = sent, failed = failed),
                        )
                        .await?;
                    }
                }
                FlowState::AwaitingPromoRedeem => {
                    let code = text.trim();
                    if code.is_empty() || code.starts_with('/') {
                        bot.send_message(chat_id, l10n!(lang, "master_promo_invalid_input"))
                            .await?;
                        return Ok(());
                    }
                    let _ = state.db.ensure_client(user_id).await?;
                    match state.db.redeem_promo_code(user_id, code).await {
                        Ok(info) => {
                            state.flow_states.remove(&user_id);
                            if info.kind == "free_pro" {
                                let until = info
                                    .new_pro_until
                                    .map(|d| {
                                        l10n!(
                                            lang,
                                            "master_pro_until_suffix",
                                            date = d.format("%d.%m.%Y")
                                        )
                                    })
                                    .unwrap_or_default();
                                bot.send_message(
                                    chat_id,
                                    l10n!(
                                        lang,
                                        "master_redeem_free_ok",
                                        days = info.param,
                                        until = until
                                    ),
                                )
                                .await?;
                            } else {
                                bot.send_message(
                                    chat_id,
                                    l10n!(
                                        lang,
                                        "master_redeem_disc_ok",
                                        percent = info.param
                                    ),
                                )
                                .await?;
                            }
                        }
                        Err(e) => {
                            bot.send_message(chat_id, format!("❌ {}", e)).await?;
                        }
                    }
                }
                FlowState::AwaitingPromoDays => {
                    if user_id != state.config.owner_id {
                        state.flow_states.remove(&user_id);
                        return Ok(());
                    }
                    match text.trim().parse::<i32>() {
                        Ok(d) if (1..=365).contains(&d) => {
                            state.flow_states.remove(&user_id);
                            ui::show_promo_uses_chooser(bot, chat_id, None, "free", d, lang)
                                .await?;
                        }
                        _ => {
                            bot.send_message(chat_id, l10n!(lang, "master_err_days"))
                                .await?;
                        }
                    }
                }
                FlowState::AwaitingPromoDiscount => {
                    if user_id != state.config.owner_id {
                        state.flow_states.remove(&user_id);
                        return Ok(());
                    }
                    match text.trim().parse::<i32>() {
                        Ok(p) if (1..=99).contains(&p) => {
                            state.flow_states.remove(&user_id);
                            ui::show_promo_uses_chooser(bot, chat_id, None, "disc", p, lang)
                                .await?;
                        }
                        _ => {
                            bot.send_message(chat_id, l10n!(lang, "master_err_percent"))
                                .await?;
                        }
                    }
                }
                FlowState::AwaitingPromoUses { kind, param } => {
                    if user_id != state.config.owner_id {
                        state.flow_states.remove(&user_id);
                        return Ok(());
                    }
                    match text.trim().parse::<i32>() {
                        Ok(u) if (0..=10_000).contains(&u) => {
                            state.flow_states.remove(&user_id);
                            super::callbacks::create_promo_and_reply(
                                bot, chat_id, state, user_id, &kind, param, u,
                            )
                            .await?;
                        }
                        _ => {
                            bot.send_message(chat_id, l10n!(lang, "master_err_uses"))
                                .await?;
                        }
                    }
                }
                FlowState::AwaitingProDays => {
                    if user_id != state.config.owner_id {
                        state.flow_states.remove(&user_id);
                        return Ok(());
                    }
                    match text.trim().parse::<i32>() {
                        Ok(d) if (1..=365).contains(&d) => {
                            state.flow_states.remove(&user_id);
                            state.db.set_setting("pro_days", &d.to_string()).await?;
                            bot.send_message(
                                chat_id,
                                l10n!(lang, "master_prodays_updated", days = d),
                            )
                            .await?;
                            ui::show_pro_settings(bot, chat_id, None, state).await?;
                        }
                        _ => {
                            bot.send_message(chat_id, l10n!(lang, "master_err_days"))
                                .await?;
                        }
                    }
                }
                FlowState::AwaitingProPrice => {
                    if user_id != state.config.owner_id {
                        state.flow_states.remove(&user_id);
                        return Ok(());
                    }
                    match text.trim().parse::<i64>() {
                        Ok(p) if (1..=250_000).contains(&p) => {
                            state.flow_states.remove(&user_id);
                            state
                                .db
                                .set_setting("pro_price_stars", &p.to_string())
                                .await?;
                            bot.send_message(
                                chat_id,
                                l10n!(lang, "master_proprice_updated", price = p),
                            )
                            .await?;
                            ui::show_pro_settings(bot, chat_id, None, state).await?;
                        }
                        _ => {
                            bot.send_message(chat_id, l10n!(lang, "master_err_price"))
                                .await?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

async fn handle_check_command(
    bot: &Bot,
    chat_id: ChatId,
    text: &str,
    state: &Arc<MasterState>,
    lang: crate::locales::Locale,
) -> R {
    let parts: Vec<&str> = text.splitn(2, char::is_whitespace).collect();
    if parts.len() < 2 || parts[1].trim().is_empty() {
        bot.send_message(chat_id, l10n!(lang, "master_check_usage"))
            .parse_mode(ParseMode::Html)
            .await?;
        return Ok(());
    }
    let query = parts[1].trim();
    perform_bot_check(bot, chat_id, query, state, lang).await
}

async fn perform_bot_check(
    bot: &Bot,
    chat_id: ChatId,
    query: &str,
    state: &Arc<MasterState>,
    lang: crate::locales::Locale,
) -> R {
    let username = query.trim_start_matches('@');
    if username.is_empty() {
        bot.send_message(chat_id, l10n!(lang, "master_check_invalid_username"))
            .await?;
        return Ok(());
    }
    let service_name = &state.config.watermark_username;
    let bot_cfg = state.db.find_active_bot_by_username(username).await?;
    match bot_cfg {
        Some(cfg) => {
            let setup_status = if cfg.setup_complete {
                l10n!(lang, "master_check_status_ok")
            } else {
                l10n!(lang, "master_check_status_pending")
            };
            let channel_info = cfg
                .channel_username
                .as_deref()
                .map(|u| format!("@{}", u))
                .unwrap_or_else(|| "—".to_string());
            bot.send_message(
                chat_id,
                l10n!(
                    lang,
                    "master_check_ok",
                    username = cfg.bot_username,
                    service = service_name,
                    status = setup_status,
                    channel = channel_info
                ),
            )
            .parse_mode(ParseMode::Html)
            .await?;
        }
        None => {
            bot.send_message(
                chat_id,
                l10n!(
                    lang,
                    "master_check_not_serviced",
                    username = username,
                    service = service_name
                ),
            )
            .parse_mode(ParseMode::Html)
            .await?;
        }
    }
    Ok(())
}
