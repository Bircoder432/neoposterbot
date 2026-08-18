use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, LabeledPrice, MaybeInaccessibleMessage, ParseMode,
};

use super::ui;
use super::{FlowState, MasterState};
use crate::locales::L10n;

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
        state.flow_states.insert(user_id, FlowState::AwaitingToken);
        bot.send_message(
            chat_id,
            "Отправьте токен вашего бота, полученный от @BotFather.",
        )
        .await?;
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "my_bots" {
        let bots = state.db.get_bots_by_client_tg_id(user_id).await?;
        if bots.is_empty() {
            bot.send_message(
                chat_id,
                "У вас пока нет активных ботов. Нажмите «Добавить бота», чтобы создать первый.",
            )
            .await?;
        } else {
            let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];
            for b in &bots {
                keyboard.push(vec![InlineKeyboardButton::callback(
                    format!("🗑 Отвязать @{} (ID: {})", b.bot_username, b.id),
                    format!("unbind_{}", b.id),
                )]);
            }
            let markup = InlineKeyboardMarkup::new(keyboard);
            bot.send_message(
                chat_id,
                "🤖 <b>Ваши активные боты:</b>\n\nНажмите на кнопку, чтобы отвязать бота.",
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(markup)
            .await?;
        }
        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if let Some(bot_id_str) = data.strip_prefix("unbind_") {
        if let Ok(bot_id) = bot_id_str.parse::<i32>() {
            let bots = state.db.get_bots_by_client_tg_id(user_id).await?;
            if bots.iter().any(|b| b.id == bot_id) {
                let _ = state.db.deactivate_bot(bot_id).await;
                state.manager.stop_worker(bot_id).await;

                bot.answer_callback_query(q.id.clone())
                    .text("✅ Бот отвязан и остановлен.")
                    .await?;

                if let Some(MaybeInaccessibleMessage::Regular(msg)) = &q.message {
                    bot.delete_message(msg.chat.id, msg.id).await.ok();
                }
                bot.send_message(
                    chat_id,
                    "Бот успешно отвязан. Вы можете привязать его снова в любой момент.",
                )
                .await?;
                return Ok(());
            } else {
                bot.answer_callback_query(q.id.clone())
                    .text("❌ Ошибка: это не ваш бот.")
                    .await?;
                return Ok(());
            }
        }
    }

    if data == "buy_pro" {
        let client = state.db.ensure_client(user_id).await?;
        let is_pro = client
            .pro_expires_at
            .map_or(false, |d| d > chrono::Utc::now());

        if is_pro {
            bot.answer_callback_query(q.id.clone())
                .text("✅ У вас уже активна Pro-подписка!")
                .await?;
            return Ok(());
        }

        bot.send_invoice(
            chat_id,
            "Pro подписка (30 дней)",
            "Снимает ограничение на 1 модератора и убирает вотермарку из постов на 30 дней.",
            "pro_sub_1m",
            "XTR",
            vec![LabeledPrice::new("Pro Plan", 100)],
        )
        .await?;

        bot.answer_callback_query(q.id.clone()).await?;
        return Ok(());
    }

    if data == "pro_active_info" {
        bot.answer_callback_query(q.id.clone())
            .text("✅ Ваша Pro-подписка активна! Вотермарки отключены, лимиты сняты.")
            .await?;
        return Ok(());
    }

    if user_id != state.config.owner_id {
        return Ok(());
    }

    let lang = super::get_locale(state, user_id).await;

    if let Some(page_str) = data.strip_prefix("clients_page_") {
        let page: i64 = page_str.parse().unwrap_or(0);
        ui::send_clients_page(bot, chat_id, state, page, lang).await?;
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
                    for b in bots {
                        state.manager.start_worker(b).await;
                    }
                    bot.answer_callback_query(q.id.clone())
                        .text(L10n::client_unbanned_msg(lang))
                        .await?;
                } else {
                    let bot_ids = state.db.ban_client(tg_id).await?;
                    for bid in bot_ids {
                        state.manager.stop_worker(bid).await;
                    }
                    bot.answer_callback_query(q.id.clone())
                        .text(L10n::client_banned_msg(lang))
                        .await?;
                }
                ui::send_client_details(bot, chat_id, message_id, state, tg_id, lang).await?;
                return Ok(());
            }
        }
    }

    if let Some(client_id_str) = data.strip_prefix("changeplan_") {
        if let Ok(tg_id) = client_id_str.parse::<i64>() {
            match state.db.toggle_client_plan(tg_id).await?.as_str() {
                "pro" => {
                    bot.answer_callback_query(q.id.clone())
                        .text(L10n::plan_changed_pro_msg(lang))
                        .await?;
                }
                _ => {
                    bot.answer_callback_query(q.id.clone())
                        .text(L10n::plan_changed_free_msg(lang))
                        .await?;
                }
            }
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
