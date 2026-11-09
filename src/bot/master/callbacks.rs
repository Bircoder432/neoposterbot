use super::ui;
use super::{FlowState, MasterState};
use crate::l10n;
use crate::locales::Locale;
use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, LabeledPrice, MaybeInaccessibleMessage, ParseMode,
};

type R = Result<()>;

pub(super) async fn process_master_callback(
    bot: &Bot,
    q: &CallbackQuery,
    state: &Arc<MasterState>,
) -> R {
    let user_id = q.from.id.0 as i64;
    let data = q.data.as_deref().unwrap_or("");
    let (chat_id, message_id) = match &q.message {
        Some(MaybeInaccessibleMessage::Regular(msg)) => (msg.chat.id, msg.id),
        _ => return Ok(()),
    };

    if data == "add_bot" {
        let lang = super::get_locale(state, user_id).await;
        state.flow_states.insert(user_id, FlowState::AwaitingToken);
        bot.send_message(chat_id, l10n!(lang, "master_add_bot_prompt"))
            .await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "my_bots" {
        let lang = super::get_locale(state, user_id).await;
        let bots = state.db.get_bots_by_client_tg_id(user_id).await?;
        if bots.is_empty() {
            bot.send_message(chat_id, l10n!(lang, "master_my_bots_empty"))
                .await?;
        } else {
            let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];
            for b in &bots {
                keyboard.push(vec![InlineKeyboardButton::callback(
                    l10n!(lang, "master_unbind_btn", username = b.bot_username, id = b.id),
                    format!("unbind_{}", b.id),
                )]);
            }
            let markup = InlineKeyboardMarkup::new(keyboard);
            bot.send_message(chat_id, l10n!(lang, "master_my_bots_title"))
                .parse_mode(ParseMode::Html)
                .reply_markup(markup)
                .await?;
        }
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "check_bot" {
        let lang = super::get_locale(state, user_id).await;
        state
            .flow_states
            .insert(user_id, FlowState::AwaitingBotCheck);
        bot.send_message(chat_id, l10n!(lang, "master_check_prompt"))
            .parse_mode(ParseMode::Html)
            .await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if let Some(bot_id_str) = data.strip_prefix("unbind_") {
        if let Ok(bot_id) = bot_id_str.parse::<i32>() {
            let lang = super::get_locale(state, user_id).await;
            let bots = state.db.get_bots_by_client_tg_id(user_id).await?;
            if bots.iter().any(|b| b.id == bot_id) {
                let _ = state.db.deactivate_bot(bot_id).await;
                state.manager.stop_worker(bot_id).await;
                bot.answer_callback_query(q.id.clone())
                    .text(l10n!(lang, "master_unbind_ok"))
                    .await?;
                if let Some(MaybeInaccessibleMessage::Regular(msg)) = &q.message {
                    bot.delete_message(msg.chat.id, msg.id).await.ok();
                }
                bot.send_message(chat_id, l10n!(lang, "master_unbind_ok2"))
                    .await?;
                return Ok(());
            } else {
                bot.answer_callback_query(q.id.clone())
                    .text(l10n!(lang, "master_unbind_not_yours"))
                    .await?;
                return Ok(());
            }
        }
    }

    if data == "buy_pro" {
        let lang = super::get_locale(state, user_id).await;
        let client = state.db.ensure_client(user_id).await?;
        let is_pro = client
            .pro_expires_at
            .map_or(false, |d| d > chrono::Utc::now());
        if is_pro {
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "master_pro_active_toast"))
                .await?;
            return Ok(());
        }
        let price = state.db.get_pro_price().await?;
        let days = state.db.get_pro_days().await?;
        let discount = state
            .db
            .get_pending_discount(user_id)
            .await?
            .map(|d| d.discount_percent)
            .unwrap_or(0)
            .min(99);
        let final_price = ui::discounted_price(price, discount);
        let title = if discount > 0 {
            l10n!(lang, "master_invoice_title_disc", days = days)
        } else {
            l10n!(lang, "master_invoice_title", days = days)
        };
        let description = if discount > 0 {
            l10n!(lang, "master_invoice_desc_disc", days = days, discount = discount)
        } else {
            l10n!(lang, "master_invoice_desc", days = days)
        };
        bot.send_invoice(
            chat_id,
            title,
            description,
            "pro_sub_1m",
            "XTR",
            vec![LabeledPrice::new("Pro Plan", final_price as u32)],
        )
        .await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "pro_active_info" {
        let lang = super::get_locale(state, user_id).await;
        bot.answer_callback_query(q.id.clone())
            .text(l10n!(lang, "master_pro_active_toast"))
            .await?;
        return Ok(());
    }

    if data == "redeem_promo" {
        let lang = super::get_locale(state, user_id).await;
        let _ = state.db.ensure_client(user_id).await?;
        state.flow_states.insert(user_id, FlowState::AwaitingPromoRedeem);
        bot.send_message(chat_id, l10n!(lang, "master_redeem_prompt"))
            .parse_mode(ParseMode::Html)
            .await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    // ---- Language selection (works for any user, including the owner) ----
    if data == "mlang" {
        let lang = super::get_locale(state, user_id).await;
        ui::show_lang_chooser(bot, chat_id, Some(message_id), lang, "mlang", "cabinet_back")
            .await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "mlang_ru" || data == "mlang_en" {
        let new_lang = if data == "mlang_ru" {
            Locale::Ru
        } else {
            Locale::En
        };
        state.db.set_client_lang_safe(user_id, new_lang).await?;
        bot.answer_callback_query(q.id.clone())
            .text(l10n!(new_lang, "lang_updated"))
            .await?;
        // Re-render the whole cabinet in the new language.
        ui::render_cabinet(bot, chat_id, Some(message_id), state, user_id).await?;
        return Ok(());
    }

    if data == "cabinet_back" {
        ui::render_cabinet(bot, chat_id, Some(message_id), state, user_id).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if user_id != state.config.owner_id {
        return Ok(());
    }

    let lang = super::get_locale(state, user_id).await;

    // ==================== Owner panel ====================

    if data == "owner_menu" {
        ui::show_owner_menu(bot, chat_id, Some(message_id), state).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "owner_clients" {
        ui::send_clients_page(bot, chat_id, state, 0, lang).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "owner_bots" {
        ui::send_bots_page(bot, chat_id, None, state, 0, lang).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "owner_stats" {
        ui::show_stats(bot, chat_id, Some(message_id), state).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "owner_broadcast" {
        state
            .flow_states
            .insert(user_id, FlowState::AwaitingBroadcast);
        bot.send_message(
            chat_id,
            format!("{}\n/cancel — отмена.", l10n!(lang, "broadcast_prompt")),
        )
        .await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "owner_promo" {
        ui::show_promo_list(bot, chat_id, None, state, 0).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "owner_proset" {
        ui::show_pro_settings(bot, chat_id, Some(message_id), state).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "owner_cabinet" {
        ui::send_client_cabinet(bot, chat_id, state, user_id).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "owner_help" {
        ui::show_help(bot, chat_id, Some(message_id), lang).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "owner_banned" {
        ui::show_banned_clients(bot, chat_id, Some(message_id), state, 0).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if let Some(p) = data.strip_prefix("bannedpage_") {
        let page: i64 = p.parse().unwrap_or(0);
        ui::show_banned_clients(bot, chat_id, Some(message_id), state, page).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    // One-tap unban straight from the "Banned" list.
    if let Some(id_str) = data.strip_prefix("unbanclient_") {
        if let Ok(tg_id) = id_str.parse::<i64>() {
            let client = state.db.get_client_by_tg_id(tg_id).await?;
            if let Some(c) = client {
                if c.banned {
                    let bots = state.db.unban_client(tg_id).await?;
                    // Free plan: keep only one active bot after unban.
                    let stopped = state.db.enforce_free_bot_limit(tg_id).await?;
                    for b in &bots {
                        if !stopped.contains(&b.id) {
                            state.manager.start_worker(b.clone()).await;
                        }
                    }
                    for id in &stopped {
                        state.manager.stop_worker(*id).await;
                    }
                    bot.answer_callback_query(q.id.clone())
                        .text(l10n!(lang, "client_unbanned_msg"))
                        .await?;
                }
            }
        }
        ui::show_banned_clients(bot, chat_id, Some(message_id), state, 0).await?;
        return Ok(());
    }

    if data == "owner_lang" {
        ui::show_lang_chooser(bot, chat_id, Some(message_id), lang, "owner_lang", "owner_menu")
            .await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "owner_lang_ru" || data == "owner_lang_en" {
        let new_lang = if data == "owner_lang_ru" {
            Locale::Ru
        } else {
            Locale::En
        };
        state
            .db
            .set_client_lang_safe(state.config.owner_id, new_lang)
            .await?;
        bot.answer_callback_query(q.id.clone())
            .text(l10n!(new_lang, "lang_updated"))
            .await?;
        ui::show_owner_menu(bot, chat_id, Some(message_id), state).await?;
        return Ok(());
    }

    // ==================== PRO settings ====================

    if data == "set_prodays" {
        ui::show_pro_days_chooser(bot, chat_id, Some(message_id), lang).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "set_proprice" {
        ui::show_pro_price_chooser(bot, chat_id, Some(message_id), lang).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "prodays_custom" {
        state.flow_states.insert(user_id, FlowState::AwaitingProDays);
        bot.send_message(chat_id, l10n!(lang, "master_prodays_prompt"))
            .await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "proprice_custom" {
        state.flow_states.insert(user_id, FlowState::AwaitingProPrice);
        bot.send_message(chat_id, l10n!(lang, "master_proprice_prompt"))
            .await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if let Some(n) = data.strip_prefix("prodays_") {
        if let Ok(days) = n.parse::<i32>() {
            if (1..=365).contains(&days) {
                state.db.set_setting("pro_days", &days.to_string()).await?;
                bot.answer_callback_query(q.id.clone())
                    .text("✅ Срок PRO обновлён")
                    .await?;
                ui::show_pro_settings(bot, chat_id, Some(message_id), state).await?;
                return Ok(());
            }
        }
    }

    if let Some(n) = data.strip_prefix("proprice_") {
        if let Ok(price) = n.parse::<i64>() {
            if (1..=250_000).contains(&price) {
                state
                    .db
                    .set_setting("pro_price_stars", &price.to_string())
                    .await?;
                bot.answer_callback_query(q.id.clone())
                    .text("✅ Цена PRO обновлена")
                    .await?;
                ui::show_pro_settings(bot, chat_id, Some(message_id), state).await?;
                return Ok(());
            }
        }
    }

    // ==================== Promo codes ====================

    if data == "promo_create" {
        ui::show_promo_type_choice(bot, chat_id, lang).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "promo_kfree" {
        ui::show_promo_days_chooser(bot, chat_id, Some(message_id), lang).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "promo_kdisc" {
        ui::show_promo_percent_chooser(bot, chat_id, Some(message_id), lang).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "promo_d_custom" {
        state.flow_states.insert(user_id, FlowState::AwaitingPromoDays);
        bot.send_message(chat_id, l10n!(lang, "master_promo_days_prompt"))
            .await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "promo_p_custom" {
        state.flow_states.insert(user_id, FlowState::AwaitingPromoDiscount);
        bot.send_message(chat_id, l10n!(lang, "master_promo_percent_prompt"))
            .await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if let Some(n) = data.strip_prefix("promo_d_") {
        if let Ok(days) = n.parse::<i32>() {
            if (1..=365).contains(&days) {
                ui::show_promo_uses_chooser(bot, chat_id, Some(message_id), "free", days, lang)
                    .await?;
                bot.answer_callback_query(q.id.clone()).await?;
                return Ok(());
            }
        }
    }

    if let Some(n) = data.strip_prefix("promo_p_") {
        if let Ok(percent) = n.parse::<i32>() {
            if (1..=99).contains(&percent) {
                ui::show_promo_uses_chooser(bot, chat_id, Some(message_id), "disc", percent, lang)
                    .await?;
                bot.answer_callback_query(q.id.clone()).await?;
                return Ok(());
            }
        }
    }

    if let Some(rest) = data.strip_prefix("promo_u_custom_") {
        if let Some((kind, param_str)) = rest.split_once('_') {
            if let Ok(param) = param_str.parse::<i32>() {
                if kind == "free" || kind == "disc" {
                    state.flow_states.insert(
                        user_id,
                        FlowState::AwaitingPromoUses {
                            kind: kind.to_string(),
                            param,
                        },
                    );
                    bot.send_message(chat_id, l10n!(lang, "master_promo_uses_prompt"))
                        .await?;
                    bot.answer_callback_query(q.id.clone()).await?;
                    return Ok(());
                }
            }
        }
    }

    if let Some(rest) = data.strip_prefix("promo_u_") {
        if let Some((uses_str, tail)) = rest.split_once('_') {
            if let (Ok(uses), Some((kind, param_str))) =
                (uses_str.parse::<i32>(), tail.split_once('_'))
            {
                if let Ok(param) = param_str.parse::<i32>() {
                    if (kind == "free" || kind == "disc") && (0..=10_000).contains(&uses) {
                        create_promo_and_reply(bot, chat_id, state, user_id, kind, param, uses)
                            .await?;
                        bot.answer_callback_query(q.id.clone()).await?;
                        return Ok(());
                    }
                }
            }
        }
    }

    if let Some(p) = data.strip_prefix("promo_page_") {
        let page: i64 = p.parse().unwrap_or(0);
        ui::show_promo_list(bot, chat_id, Some(message_id), state, page).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if let Some(id_str) = data.strip_prefix("promo_view_") {
        if let Ok(promo_id) = id_str.parse::<i32>() {
            ui::show_promo_details(bot, chat_id, message_id, state, promo_id).await?;
            bot.answer_callback_query(q.id.clone()).await?;
            return Ok(());
        }
    }

    if let Some(id_str) = data.strip_prefix("promo_toggle_") {
        if let Ok(promo_id) = id_str.parse::<i32>() {
            if let Some(promo) = state.db.get_promo_by_id(promo_id).await? {
                state.db.set_promo_active(promo_id, !promo.active).await?;
            }
            ui::show_promo_details(bot, chat_id, message_id, state, promo_id).await?;
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "master_status_updated"))
                .await?;
            return Ok(());
        }
    }

    if let Some(id_str) = data.strip_prefix("promo_delok_") {
        if let Ok(promo_id) = id_str.parse::<i32>() {
            state.db.delete_promo_code(promo_id).await?;
            ui::show_promo_list(bot, chat_id, Some(message_id), state, 0).await?;
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "master_promo_deleted"))
                .await?;
            return Ok(());
        }
    }

    if let Some(id_str) = data.strip_prefix("promo_del_") {
        if let Ok(promo_id) = id_str.parse::<i32>() {
            if let Some(promo) = state.db.get_promo_by_id(promo_id).await? {
                ui::show_promo_delete_confirm(bot, chat_id, message_id, &promo, lang).await?;
                bot.answer_callback_query(q.id.clone()).await?;
            }
            return Ok(());
        }
    }

    if let Some(page_str) = data.strip_prefix("clients_page_") {
        let page: i64 = page_str.parse().unwrap_or(0);
        ui::send_clients_page(bot, chat_id, state, page, lang).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if let Some(page_str) = data.strip_prefix("bots_page_") {
        let page: i64 = page_str.parse().unwrap_or(0);
        ui::send_bots_page(bot, chat_id, Some(message_id), state, page, lang).await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if let Some(client_id_str) = data.strip_prefix("client_") {
        if let Ok(tg_id) = client_id_str.parse::<i64>() {
            ui::send_client_details(bot, chat_id, message_id, state, tg_id, lang).await?;
            bot.answer_callback_query(q.id.clone()).await?;
            return Ok(());
        }
    }

    if let Some(client_id_str) = data.strip_prefix("banclient_") {
        if let Ok(tg_id) = client_id_str.parse::<i64>() {
            let client = state.db.get_client_by_tg_id(tg_id).await?;
            if let Some(c) = client {
                if c.banned {
                    let bots = state.db.unban_client(tg_id).await?;
                    // Free plan: keep only one active bot after unban.
                    let stopped = state.db.enforce_free_bot_limit(tg_id).await?;
                    for b in &bots {
                        if !stopped.contains(&b.id) {
                            state.manager.start_worker(b.clone()).await;
                        }
                    }
                    for id in &stopped {
                        state.manager.stop_worker(*id).await;
                    }
                    bot.answer_callback_query(q.id.clone())
                        .text(l10n!(lang, "client_unbanned_msg"))
                        .await?;
                } else {
                    let bot_ids = state.db.ban_client(tg_id).await?;
                    for bid in bot_ids {
                        state.manager.stop_worker(bid).await;
                    }
                    bot.answer_callback_query(q.id.clone())
                        .text(l10n!(lang, "client_banned_msg"))
                        .await?;
                }
                ui::send_client_details(bot, chat_id, message_id, state, tg_id, lang).await?;
                return Ok(());
            }
        }
    }

    if let Some(client_id_str) = data.strip_prefix("changeplan_") {
        if let Ok(tg_id) = client_id_str.parse::<i64>() {
            let (result, stopped) = state.db.toggle_client_plan(tg_id).await?;
            for id in &stopped {
                state.manager.stop_worker(*id).await;
            }
            let toast = match result.as_str() {
                "pro" => l10n!(lang, "plan_changed_pro_msg"),
                _ => {
                    let mut t = l10n!(lang, "plan_changed_free_msg");
                    if !stopped.is_empty() {
                        t.push_str(&l10n!(lang, "master_bots_stopped_note", count = stopped.len()));
                    }
                    t
                }
            };
            bot.answer_callback_query(q.id.clone())
                .text(toast)
                .await?;
            ui::send_client_details(bot, chat_id, message_id, state, tg_id, lang).await?;
            return Ok(());
        }
    }

    if let Some(client_id_str) = data.strip_prefix("clientbots_") {
        if let Ok(tg_id) = client_id_str.parse::<i64>() {
            ui::send_client_bots(bot, chat_id, message_id, state, tg_id, 0, lang).await?;
            bot.answer_callback_query(q.id.clone()).await?;
            return Ok(());
        }
    }

    if let Some(parts) = data.strip_prefix("clientbots_page_") {
        if let Some((tg_id_str, page_str)) = parts.rsplit_once('_') {
            if let (Ok(tg_id), Ok(page)) = (tg_id_str.parse::<i64>(), page_str.parse::<i64>()) {
                ui::send_client_bots(bot, chat_id, message_id, state, tg_id, page, lang).await?;
                bot.answer_callback_query(q.id.clone()).await?;
                return Ok(());
            }
        }
    }

    if let Some(bot_id_str) = data.strip_prefix("botinfo_") {
        if let Ok(bot_id) = bot_id_str.parse::<i32>() {
            ui::send_bot_info(bot, chat_id, message_id, state, bot_id, lang).await?;
            bot.answer_callback_query(q.id.clone()).await?;
            return Ok(());
        }
    }

    Ok(())
}

/// Creates a promo code (owner) and replies with the generated code.
/// `kind` is the short tag: "free" | "disc".
pub(super) async fn create_promo_and_reply(
    bot: &Bot,
    chat_id: ChatId,
    state: &Arc<MasterState>,
    user_id: i64,
    kind: &str,
    param: i32,
    max_uses: i32,
) -> R {
    let db_kind = if kind == "free" { "free_pro" } else { "discount" };
    let promo = state
        .db
        .create_promo_code(db_kind, param, max_uses, user_id)
        .await?;
    let lang = super::get_locale(state, state.config.owner_id).await;
    let type_str = if db_kind == "free_pro" {
        l10n!(lang, "master_promo_type_free", days = param)
    } else {
        l10n!(lang, "master_promo_type_disc", percent = param)
    };
    let uses_str = if max_uses == 0 {
        l10n!(lang, "master_promo_uses_unlimited")
    } else {
        l10n!(lang, "master_promo_uses_n", n = max_uses)
    };
    let text = l10n!(
        lang,
        "master_promo_created",
        code = promo.code,
        type = type_str,
        uses = uses_str
    );
    let kb = InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_promo_more"), "promo_create")],
        vec![InlineKeyboardButton::callback(l10n!(lang, "mbtn_to_list"), "owner_promo")],
    ]);
    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await?;
    Ok(())
}
