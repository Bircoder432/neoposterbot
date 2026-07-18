use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

use crate::bot::worker::WorkerState;
use crate::locales::L10n;

type R = anyhow::Result<()>;

pub(super) async fn handle_setup(bot: &Bot, msg: &Message, state: &WorkerState) -> R {
    let text = msg.text().unwrap_or("");
    let bot_id = state.bot_id;

    if text.starts_with("/start") {
        let kb = InlineKeyboardMarkup::new(vec![vec![
            InlineKeyboardButton::callback("🇷🇺 Русский", "setup_lang_ru"),
            InlineKeyboardButton::callback("🇬🇧 English", "setup_lang_en"),
        ]]);
        bot.send_message(
            msg.chat.id,
            "👋 Давайте настроим вашего бота!\n\nPlease select your language:\n\nПожалуйста, выберите язык:",
        )
        .reply_markup(kb)
        .await?;
        return Ok(());
    }

    let lang = state.db.get_language(bot_id).await?;

    if let Some(code) = state.db.get_setup_code(bot_id).await? {
        bot.send_message(
            msg.chat.id,
            format!(
                "{}\n\nОтправьте в ваш канал команду:\n<code>/connect {}</code>",
                L10n::setup_enter_channel(lang),
                code
            ),
        )
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;
    } else {
        let kb = InlineKeyboardMarkup::new(vec![vec![
            InlineKeyboardButton::callback("🇷🇺 Русский", "setup_lang_ru"),
            InlineKeyboardButton::callback("🇬🇧 English", "setup_lang_en"),
        ]]);
        bot.send_message(
            msg.chat.id,
            "👋 Please select your language:\n\nПожалуйста, выберите язык:",
        )
        .reply_markup(kb)
        .await?;
    }
    Ok(())
}

pub(super) async fn process_channel_post(bot: &Bot, post: &Message, state: &WorkerState) -> R {
    let setup_complete = state.db.is_setup_complete(state.bot_id).await?;
    if setup_complete {
        return Ok(());
    }

    let text = post.text().unwrap_or("").to_lowercase();
    if let Some(code_provided) = text.strip_prefix("/connect ") {
        let code_provided = code_provided.trim();
        if let Some(expected_code) = state.db.get_setup_code(state.bot_id).await? {
            if expected_code.to_lowercase() == code_provided {
                let channel_id = post.chat.id.0;
                let channel_name = post.chat.title().unwrap_or("Канал").to_string();
                let channel_username = post.chat.username().map(|s| s.to_string());
                state
                    .db
                    .complete_setup(state.bot_id, channel_id, channel_username.as_deref())
                    .await?;

                let mut cfg = state.config.write().await;
                cfg.channel_id = channel_id;
                cfg.channel_username = channel_username;
                cfg.setup_complete = true;
                cfg.setup_code = None;
                drop(cfg);

                bot.delete_message(post.chat.id, post.id).await.ok();

                if state.client_tg_id != 0 {
                    let lang = state.db.get_language(state.bot_id).await?;
                    bot.send_message(
                        ChatId(state.client_tg_id),
                        L10n::setup_success(lang, &channel_name),
                    )
                    .await?;
                }
            }
        }
    }
    Ok(())
}
