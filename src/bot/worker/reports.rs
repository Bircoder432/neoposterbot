use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};
use url::Url;

use crate::bot::worker::WorkerState;
use crate::db::Database;
use crate::hashing::hash_user_id;
use crate::locales::{L10n, Locale};

type R = anyhow::Result<()>;

pub(super) async fn handle_report_content(
    bot: &Bot,
    msg: &Message,
    state: &WorkerState,
    channel_msg_id: i64,
) -> R {
    let user_id = msg.from.as_ref().unwrap().id.0 as i64;
    let chat_id = msg.chat.id;
    let lang = state.db.get_language(state.bot_id).await?;

    if state
        .db
        .is_banned(state.bot_id, &hash_user_id(user_id))
        .await?
    {
        bot.send_message(chat_id, L10n::user_banned(lang)).await?;
        return Ok(());
    }

    let reason = msg.text().unwrap_or("").trim();
    if reason.is_empty() {
        bot.send_message(chat_id, L10n::report_empty_reason(lang))
            .await?;
        return Ok(());
    }

    let user_hash = hash_user_id(user_id);
    state
        .db
        .create_report(state.bot_id, channel_msg_id as i32, &user_hash, reason)
        .await?;

    state.user_states.remove(&user_id);
    bot.send_message(chat_id, L10n::report_accepted(lang))
        .await?;
    Ok(())
}

pub(super) async fn show_next_report(
    bot: &Bot,
    chat_id: ChatId,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let reports = state.db.get_pending_reports(state.bot_id).await?;
    if reports.is_empty() {
        bot.send_message(chat_id, L10n::no_pending_reports(lang))
            .await?;
        return Ok(());
    }

    let report = &reports[0];

    let cfg = state.config.read().await;
    let channel_id = cfg.channel_id;
    let channel_username = cfg.channel_username.clone();
    drop(cfg);

    let post_link = if let Some(ref uname) = channel_username {
        format!("https://t.me/{}/{}", uname, report.channel_msg_id)
    } else {
        let id_str = channel_id.to_string();
        let internal = id_str.strip_prefix("-100").unwrap_or(&id_str);
        format!("https://t.me/c/{}/{}", internal, report.channel_msg_id)
    };

    let url = Url::parse(&post_link).unwrap_or_else(|_| Url::parse("https://t.me").unwrap());

    let date_str = report.created_at.format("%d.%m.%Y %H:%M").to_string();
    let text = L10n::report_display(lang, report.id, &report.reason, &post_link, &date_str);

    let kb = InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::url(
            L10n::report_view_post_btn(lang),
            url,
        )],
        vec![
            InlineKeyboardButton::callback(
                L10n::report_dismiss_btn(lang),
                format!("report_dismiss_{}", report.id),
            ),
            InlineKeyboardButton::callback(
                L10n::report_ban_btn(lang),
                format!("report_ban_{}", report.id),
            ),
        ],
    ]);

    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await?;

    Ok(())
}

pub(super) async fn handle_report_dismiss(
    bot: &Bot,
    chat_id: ChatId,
    report_id: i32,
    q: &CallbackQuery,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let admin_id = q.from.id.0 as i64;
    let admin_name = crate::bot::worker::utils::get_admin_name(state, admin_id).await;

    state.db.dismiss_report(state.bot_id, report_id).await?;
    let _ = state
        .db
        .log_action(
            state.bot_id,
            admin_id,
            &admin_name,
            "dismiss_report",
            &report_id.to_string(),
        )
        .await;

    bot.answer_callback_query(q.id.clone())
        .text(L10n::report_dismissed_msg(lang))
        .await?;
    if let Some(teloxide::types::MaybeInaccessibleMessage::Regular(msg)) = &q.message {
        bot.delete_message(msg.chat.id, msg.id).await.ok();
    }
    show_next_report(bot, chat_id, state, lang).await?;
    Ok(())
}

pub(super) async fn handle_report_ban(
    bot: &Bot,
    chat_id: ChatId,
    report_id: i32,
    q: &CallbackQuery,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let admin_id = q.from.id.0 as i64;
    let admin_name = crate::bot::worker::utils::get_admin_name(state, admin_id).await;

    let report = match state.db.get_report_by_id(state.bot_id, report_id).await? {
        Some(r) => r,
        None => {
            bot.answer_callback_query(q.id.clone())
                .text(L10n::report_gone(lang))
                .await?;
            return Ok(());
        }
    };
    if state
        .db
        .is_banned(state.bot_id, &report.reporter_hash)
        .await?
    {
        state.db.resolve_report(state.bot_id, report_id).await?;
        bot.answer_callback_query(q.id.clone())
            .text(L10n::reporter_already_banned(lang))
            .await?;
        if let Some(teloxide::types::MaybeInaccessibleMessage::Regular(msg)) = &q.message {
            bot.delete_message(msg.chat.id, msg.id).await.ok();
        }
        show_next_report(bot, chat_id, state, lang).await?;
        return Ok(());
    }

    let ban_id = Database::new_ban_id();
    let ban_reason = L10n::report_ban_reason(lang, &report.reason);

    match state
        .db
        .ban_user(state.bot_id, &report.reporter_hash, &ban_reason, &ban_id)
        .await
    {
        Ok(()) => {
            state.db.resolve_report(state.bot_id, report_id).await?;
            let _ = state
                .db
                .log_action(state.bot_id, admin_id, &admin_name, "ban_reporter", &ban_id)
                .await;
            bot.answer_callback_query(q.id.clone())
                .text(L10n::reporter_banned_msg(lang, &ban_id))
                .await?;
            if let Some(teloxide::types::MaybeInaccessibleMessage::Regular(msg)) = &q.message {
                bot.delete_message(msg.chat.id, msg.id).await.ok();
            }
            show_next_report(bot, chat_id, state, lang).await?;
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to ban reporter");
            bot.answer_callback_query(q.id.clone())
                .text(L10n::error_banning_user(lang))
                .await?;
        }
    }
    Ok(())
}
