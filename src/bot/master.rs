use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, LabeledPrice};

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
    AwaitingChannelId(String),
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
    // Подтверждаем все pre-checkout запросы для Stars
    bot.answer_pre_checkout_query(q.id, true).await?;
    Ok(())
}

async fn handle_master_message(bot: Bot, msg: Message, state: Arc<MasterState>) -> ResponseResult {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let text = msg.text().unwrap_or("");
    let chat_id = msg.chat.id;

    // Обработка успешного платежа
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
            let txt = "👑 Панель администратора:\n\n/stats - статистика сервиса\n/setplan <user_id> <free|pro> - выдать план вручную\n/banclient <user_id> - забанить клиента";
            bot.send_message(chat_id, txt).await?;
            return Ok(());
        } else if text.starts_with("/stats") {
            let clients = state.db.get_all_clients().await.unwrap_or_default();
            let bots = state.db.get_all_bots().await.unwrap_or_default();
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
                let bot_ids = state.db.ban_client(tg_id).await.unwrap_or_default();
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

    // Общие команды
    if text.starts_with("/start") {
        let _ = state.db.ensure_client(user_id).await;

        let kb = InlineKeyboardMarkup::new(vec![
            vec![
                InlineKeyboardButton::callback("➕ Добавить бота", "add_bot"),
                InlineKeyboardButton::callback("🤖 Мои боты", "my_bots"),
            ],
            vec![InlineKeyboardButton::callback(
                "⭐ Купить Pro (100 Stars)",
                "buy_pro",
            )],
        ]);
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
                            let _ = state.db.ensure_client(user_id).await;
                            state
                                .flow_states
                                .insert(user_id, FlowState::AwaitingChannelId(token));
                            bot.send_message(chat_id, format!("✅ Токен принят! Бот: @{}\n\nТеперь отправьте ID вашего Telegram-канала (начинается с -100...). Бот должен быть администратором канала!", username)).await?;
                        }
                        Err(_) => {
                            bot.send_message(chat_id, "❌ Неверный токен. Попробуйте снова.")
                                .await?;
                        }
                    }
                }
                FlowState::AwaitingChannelId(token) => {
                    let channel_id: i64 = match text.trim().parse() {
                        Ok(id) if id < 0 => id,
                        _ => {
                            bot.send_message(chat_id, "❌ Неверный ID канала. Он должен начинаться с -100. Попробуйте снова.").await?;
                            return Ok(());
                        }
                    };

                    let me = teloxide::Bot::new(&token).get_me().await.unwrap();
                    let username = me.username().to_string();

                    match state
                        .db
                        .add_bot(user_id, &token, &username, channel_id)
                        .await
                    {
                        Ok(bot_cfg) => {
                            let _ = state.db.add_admin(bot_cfg.id, user_id, "Owner").await;

                            let mut bot_cfg_with_id = bot_cfg.clone();
                            bot_cfg_with_id.client_tg_id = Some(user_id);
                            state.manager.start_worker(bot_cfg_with_id).await;

                            bot.send_message(chat_id, format!("✅ Бот @{} успешно привязан и запущен!\n\nВы автоматически получили права владельца в этом боте. Перейдите в личные сообщения вашего бота (@{}) и отправьте /start для настройки.", username, username)).await?;
                            state.flow_states.remove(&user_id);
                        }
                        Err(e) => {
                            bot.send_message(chat_id, format!("❌ Ошибка сохранения: {}", e))
                                .await?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_master_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<MasterState>,
) -> ResponseResult {
    let user_id = q.from.id.0 as i64;
    let data = q.data.as_deref().unwrap_or("");
    let chat_id = match &q.message {
        Some(teloxide::types::MaybeInaccessibleMessage::Regular(msg)) => msg.chat.id,
        _ => return Ok(()),
    };

    if data == "add_bot" {
        state.flow_states.insert(user_id, FlowState::AwaitingToken);
        bot.send_message(
            chat_id,
            "Отправьте токен вашего бота, полученный от @BotFather.",
        )
        .await?;
    } else if data == "my_bots" {
        let bots = state
            .db
            .get_bots_by_client_tg_id(user_id)
            .await
            .unwrap_or_default();
        if bots.is_empty() {
            bot.send_message(
                chat_id,
                "У вас пока нет активных ботов. Нажмите «Добавить бота», чтобы создать первый.",
            )
            .await?;
        } else {
            let mut text = "🤖 <b>Ваши активные боты:</b>\n\n".to_string();
            for b in bots {
                text.push_str(&format!(
                    "• @{} (Канал: {})\n",
                    b.bot_username, b.channel_id
                ));
            }
            bot.send_message(chat_id, text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }
    } else if data == "buy_pro" {
        // Отправляем инвойс для оплаты Telegram Stars
        bot.send_invoice(
            chat_id,
            "Pro подписка (30 дней)",
            "Снимает ограничение на 1 модератора и убирает вотермарку из постов на 30 дней.",
            "pro_sub_1m",
            "XTR",
            vec![LabeledPrice::new("Pro Plan", 100)],
        )
        .await?;
    }

    bot.answer_callback_query(q.id).await?;
    Ok(())
}
