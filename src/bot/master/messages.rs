use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

use super::ui;
use super::{FlowState, MasterState};
use crate::locales::L10n;

type R = Result<()>;

pub(super) async fn process_master_message(
    bot: &Bot,
    msg: &Message,
    state: &Arc<MasterState>,
) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let text = msg.text().unwrap_or("");
    let chat_id = msg.chat.id;

    if let Some(payment) = msg.successful_payment() {
        if payment.invoice_payload == "pro_sub_1m" {
            let _ = state.db.upgrade_to_pro(user_id).await;
            bot.send_message(chat_id, "✅ Вы успешно приобрели Pro-подписку на 30 дней!\n\nОграничения на модераторов сняты, вотермарки отключены.").await?;
        }
        return Ok(());
    }

    if user_id == state.config.owner_id {
        let lang = super::get_locale(state, user_id).await;

        if text.starts_with("/help") {
            bot.send_message(chat_id, L10n::master_help(lang)).await?;
            return Ok(());
        } else if text.starts_with("/clients") {
            ui::send_clients_page(bot, chat_id, state, 0, lang).await?;
            return Ok(());
        } else if text.starts_with("/stats") {
            let clients = state.db.get_all_clients().await?;
            let bots = state.db.get_all_bots().await?;
            let (pro_count, free_count) = state.db.get_stats().await.unwrap_or((0, 0));
            let txt = format!(
                "📊 Статистика:\n\nВсего клиентов: {}\nАктивных ботов: {}\n\nPro-клиентов: {}\nFree-клиентов: {}",
                clients.len(),
                bots.len(),
                pro_count,
                free_count
            );
            bot.send_message(chat_id, txt).await?;
            return Ok(());
        } else if text.starts_with("/setplan") {
            let parts: Vec<&str> = text.splitn(3, ' ').collect();
            if parts.len() == 3 {
                let tg_id: i64 = parts[1].parse().unwrap_or(0);
                let plan = parts[2];
                let _ = state.db.set_client_plan(tg_id, plan).await;
                bot.send_message(
                    chat_id,
                    format!("✅ План для {} изменен на {}.", tg_id, plan),
                )
                .await?;
            } else {
                bot.send_message(chat_id, "Использование: /setplan <user_id> <free|pro>")
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
                    format!("🚫 Клиент {} забанен. Боты остановлены.", tg_id),
                )
                .await?;
            }
            return Ok(());
        }
    }

    if text.starts_with("/start") {
        let client = state.db.ensure_client(user_id).await?;
        let is_pro = client
            .pro_expires_at
            .map_or(false, |d| d > chrono::Utc::now());

        let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![vec![
            InlineKeyboardButton::callback("➕ Добавить бота", "add_bot"),
            InlineKeyboardButton::callback("🤖 Мои боты", "my_bots"),
        ]];

        if is_pro {
            keyboard.push(vec![InlineKeyboardButton::callback(
                "✅ Pro подписка активна",
                "pro_active_info",
            )]);
        } else {
            keyboard.push(vec![InlineKeyboardButton::callback(
                "⭐ Купить Pro (100 Stars)",
                "buy_pro",
            )]);
        }

        keyboard.push(vec![InlineKeyboardButton::url(
            "📚 Инструкция",
            state.config.instruction_url.clone(),
        )]);

        let kb = InlineKeyboardMarkup::new(keyboard);
        bot.send_message(chat_id, "Добро пожаловать в конструктор предложек!\n\nЗдесь вы можете создать своего бота для анонимных предложений и привязать его к вашему Telegram-каналу.\n\nПриобретите Pro-подписку для снятия ограничений на модераторов и отключения вотермарок.")
           .reply_markup(kb).await?;
    } else {
        let flow = state.flow_states.get(&user_id).map(|f| f.clone());

        if let Some(flow) = flow {
            match flow {
                FlowState::AwaitingToken => {
                    let token = text.trim().to_string();
                    match teloxide::Bot::new(&token).get_me().await {
                        Ok(me) => {
                            let username = me.username().to_string();

                            match state.db.add_bot(user_id, &token, &username).await {
                                Ok(bot_cfg) => {
                                    let _ = state
                                        .db
                                        .add_admin(bot_cfg.id, user_id, "Owner", false)
                                        .await;

                                    let mut bot_cfg_with_id = bot_cfg.clone();
                                    bot_cfg_with_id.client_tg_id = Some(user_id);
                                    state.manager.start_worker(bot_cfg_with_id).await;

                                    bot.send_message(chat_id, format!("✅ Бот @{} успешно привязан!\n\nЯ запустил вашего бота. Перейдите в его личные сообщения (@{}) и отправьте /start для первоначальной настройки (выбор языка и привязка канала).", username, username)).await?;
                                    state.flow_states.remove(&user_id);
                                }
                                Err(e) => {
                                    bot.send_message(
                                        chat_id,
                                        format!("❌ Ошибка сохранения: {}", e),
                                    )
                                    .await?;
                                }
                            }
                        }
                        Err(_) => {
                            bot.send_message(chat_id, "❌ Неверный токен. Попробуйте снова.")
                                .await?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
