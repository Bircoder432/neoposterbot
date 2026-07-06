use anyhow::Result;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, LabeledPrice, MaybeInaccessibleMessage};
use dashmap::DashMap;

use crate::bot::manager::BotManager;
use crate::config::Config;
use crate::db::repository::Database;

pub struct MasterState {
    pub db: Database,
    pub manager: Arc<BotManager>,
    pub config: Arc<Config>,
    pub flow_states: DashMap<i64, FlowState>,
}

#[derive(Clone)]
pub enum FlowState {
    AwaitingToken,
}

pub async fn run_master_bot(state: Arc<MasterState>) -> Result<()> {
    let bot = Bot::new(&state.config.master_bot_token);

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handle_master_message))
        .branch(Update::filter_callback_query().endpoint(handle_master_callback))
        .branch(Update::filter_pre_checkout_query().endpoint(handle_pre_checkout));

    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build();

    dispatcher.dispatch().await;
    Ok(())
}

type ResponseResult = std::result::Result<(), teloxide::RequestError>;

async fn handle_pre_checkout(bot: Bot, q: PreCheckoutQuery) -> ResponseResult {
    if let Err(e) = process_pre_checkout(&bot, &q).await {
        tracing::error!(error = %e, "Pre-checkout error");
    }
    Ok(())
}

async fn handle_master_message(bot: Bot, msg: Message, state: Arc<MasterState>) -> ResponseResult {
    if let Err(e) = process_master_message(&bot, &msg, &state).await {
        tracing::error!(error = %e, "Master message handler error");
    }
    Ok(())
}

async fn handle_master_callback(bot: Bot, q: CallbackQuery, state: Arc<MasterState>) -> ResponseResult {
    if let Err(e) = process_master_callback(&bot, &q, &state).await {
        tracing::error!(error = %e, "Master callback handler error");
    }
    Ok(())
}

// --- ВНУТРЕННЯЯ ЛОГИКА (Возвращает anyhow::Result, прокидывая ошибки наверх) ---

async fn process_pre_checkout(bot: &Bot, q: &PreCheckoutQuery) -> Result<()> {
    bot.answer_pre_checkout_query(q.id.clone(), true).await?;
    Ok(())
}

async fn process_master_message(bot: &Bot, msg: &Message, state: &Arc<MasterState>) -> Result<()> {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let text = msg.text().unwrap_or("");
    let chat_id = msg.chat.id;

    // Обработка успешного платежа
    if let Some(payment) = msg.successful_payment() {
        if payment.invoice_payload == "pro_sub_1m" {
            let _ = state.db.upgrade_to_pro(user_id).await;
            bot.send_message(chat_id, "✅ Вы успешно приобрели Pro-подписку на 30 дней!\n\nОграничения на модераторов сняты, вотермарки отключены.").await?;
        }
        return Ok(());
    }

    // Команды Owner-а (создателя сервиса)
    if user_id == state.config.owner_id {
        if text.starts_with("/help") {
            let txt = "👑 Панель администратора:\n\n/stats - статистика сервиса\n/clients - список клиентов (их user_id)\n/setplan <user_id> <free|pro> - выдать план вручную\n/banclient <user_id> - забанить клиента";
            bot.send_message(chat_id, txt).await?;
            return Ok(());
        } else if text.starts_with("/clients") {
            let clients = state.db.get_all_clients().await?;
            if clients.is_empty() {
                bot.send_message(chat_id, "Клиентов пока нет.").await?;
            } else {
                let mut text = "👥 <b>Список клиентов:</b>\n\n".to_string();
                for c in clients {
                    let plan_info = if c.pro_expires_at.map_or(false, |d| d > chrono::Utc::now()) {
                        format!("Pro (до {})", c.pro_expires_at.unwrap().format("%d.%m.%Y"))
                    } else {
                        "Free".to_string()
                    };
                    text.push_str(&format!("• ID: {} | {}\n", c.tg_user_id, plan_info));
                }
                bot.send_message(chat_id, text).parse_mode(teloxide::types::ParseMode::Html).await?;
            }
            return Ok(());
        } else if text.starts_with("/stats") {
            let clients = state.db.get_all_clients().await?;
            let bots = state.db.get_all_bots().await?;
            let (pro_count, free_count) = state.db.get_stats().await.unwrap_or((0, 0));
            let txt = format!("📊 Статистика:\n\nВсего клиентов: {}\nАктивных ботов: {}\n\nPro-клиентов: {}\nFree-клиентов: {}", clients.len(), bots.len(), pro_count, free_count);
            bot.send_message(chat_id, txt).await?;
            return Ok(());
        } else if text.starts_with("/setplan") {
            let parts: Vec<&str> = text.splitn(3, ' ').collect();
            if parts.len() == 3 {
                let tg_id: i64 = parts[1].parse().unwrap_or(0);
                let plan = parts[2];
                let _ = state.db.set_client_plan(tg_id, plan).await;
                bot.send_message(chat_id, format!("✅ План для {} изменен на {}.", tg_id, plan)).await?;
            } else {
                bot.send_message(chat_id, "Использование: /setplan <user_id> <free|pro>").await?;
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
                bot.send_message(chat_id, format!("🚫 Клиент {} забанен. Боты остановлены.", tg_id)).await?;
            }
            return Ok(());
        }
    }

    // Общие команды
    if text.starts_with("/start") {
        let client = state.db.ensure_client(user_id).await?;
        let is_pro = client.pro_expires_at.map_or(false, |d| d > chrono::Utc::now());

        let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![
            vec![
                InlineKeyboardButton::callback("➕ Добавить бота", "add_bot"),
                InlineKeyboardButton::callback("🤖 Мои боты", "my_bots"),
            ],
        ];

        if is_pro {
            keyboard.push(vec![InlineKeyboardButton::callback("✅ Pro подписка активна", "pro_active_info")]);
        } else {
            keyboard.push(vec![InlineKeyboardButton::callback("⭐ Купить Pro (100 Stars)", "buy_pro")]);
        }

        keyboard.push(vec![InlineKeyboardButton::url("📚 Инструкция", state.config.instruction_url.clone())]);

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
                                    let _ = state.db.add_admin(bot_cfg.id, user_id, "Owner", false).await;

                                    let mut bot_cfg_with_id = bot_cfg.clone();
                                    bot_cfg_with_id.client_tg_id = Some(user_id);
                                    state.manager.start_worker(bot_cfg_with_id).await;
                                    
                                    bot.send_message(chat_id, format!("✅ Бот @{} успешно привязан!\n\nЯ запустил вашего бота. Перейдите в его личные сообщения (@{}) и отправьте /start для первоначальной настройки (выбор языка и привязка канала).", username, username)).await?;
                                    state.flow_states.remove(&user_id);
                                }
                                Err(e) => {
                                    bot.send_message(chat_id, format!("❌ Ошибка сохранения: {}", e)).await?;
                                }
                            }
                        }
                        Err(_) => {
                            bot.send_message(chat_id, "❌ Неверный токен. Попробуйте снова.").await?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

async fn process_master_callback(bot: &Bot, q: &CallbackQuery, state: &Arc<MasterState>) -> Result<()> {
    let user_id = q.from.id.0 as i64;
    let data = q.data.as_deref().unwrap_or("");
    let chat_id = match &q.message {
        Some(MaybeInaccessibleMessage::Regular(msg)) => msg.chat.id,
        _ => return Ok(()),
    };

    if data == "add_bot" {
        state.flow_states.insert(user_id, FlowState::AwaitingToken);
        bot.send_message(chat_id, "Отправьте токен вашего бота, полученный от @BotFather.").await?;
        bot.answer_callback_query(q.id.clone()).await?;
    } else if data == "my_bots" {
        let bots = state.db.get_bots_by_client_tg_id(user_id).await?;
        if bots.is_empty() {
            bot.send_message(chat_id, "У вас пока нет активных ботов. Нажмите «Добавить бота», чтобы создать первый.").await?;
        } else {
            let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];
            for b in &bots {
                keyboard.push(vec![
                    InlineKeyboardButton::callback(
                        format!("🗑 Отвязать @{} (ID: {})", b.bot_username, b.id),
                        format!("unbind_{}", b.id),
                    )
                ]);
            }
            let markup = InlineKeyboardMarkup::new(keyboard);
            bot.send_message(chat_id, "🤖 <b>Ваши активные боты:</b>\n\nНажмите на кнопку, чтобы отвязать бота.")
               .parse_mode(teloxide::types::ParseMode::Html)
               .reply_markup(markup).await?;
        }
        bot.answer_callback_query(q.id.clone()).await?;
    } else if let Some(bot_id_str) = data.strip_prefix("unbind_") {
        if let Ok(bot_id) = bot_id_str.parse::<i32>() {
            let bots = state.db.get_bots_by_client_tg_id(user_id).await?;
            if bots.iter().any(|b| b.id == bot_id) {
                let _ = state.db.deactivate_bot(bot_id).await;
                state.manager.stop_worker(bot_id).await;
                
                bot.answer_callback_query(q.id.clone()).text("✅ Бот отвязан и остановлен.").await?;
                
                if let Some(MaybeInaccessibleMessage::Regular(msg)) = &q.message {
                    bot.delete_message(msg.chat.id, msg.id).await.ok();
                }
                bot.send_message(chat_id, "Бот успешно отвязан. Вы можете привязать его снова в любой момент.").await?;
                return Ok(());
            } else {
                bot.answer_callback_query(q.id.clone()).text("❌ Ошибка: это не ваш бот.").await?;
                return Ok(());
            }
        }
    } else if data == "buy_pro" {
        // ПРОВЕРЯЕМ НАЛИЧИЕ ПОДПИСКИ ПЕРЕД ОТПРАВКОЙ ИНВОЙСА
        let client = state.db.ensure_client(user_id).await?;
        let is_pro = client.pro_expires_at.map_or(false, |d| d > chrono::Utc::now());
        
        if is_pro {
            bot.answer_callback_query(q.id.clone()).text("✅ У вас уже активна Pro-подписка!").await?;
            return Ok(());
        }

        // ОТПРАВКА ИНВОЙСА
        bot.send_invoice(
            chat_id,
            "Pro подписка (30 дней)",
            "Снимает ограничение на 1 модератора и убирает вотермарку из постов на 30 дней.",
            "pro_sub_1m",
            "XTR",
            vec![LabeledPrice::new("Pro Plan", 100)],
        ).await?;
        
        bot.answer_callback_query(q.id.clone()).await?;
    } else if data == "pro_active_info" {
        bot.answer_callback_query(q.id.clone()).text("✅ Ваша Pro-подписка активна! Вотермарки отключены, лимиты сняты.").await?;
    }

    Ok(())
}
