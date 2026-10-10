use crate::bot::worker::WorkerState;
use crate::db::Database;
use crate::hashing::hash_user_id;
use crate::l10n;
use crate::locales::Locale;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};
use url::Url;

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
        bot.send_message(chat_id, l10n!(lang, "user_banned"))
            .await?;
        return Ok(());
    }
    let reason = msg.text().unwrap_or("").trim();
    if reason.is_empty() {
        bot.send_message(chat_id, l10n!(lang, "report_empty_reason"))
            .await?;
        return Ok(());
    }
    let user_hash = hash_user_id(user_id);
    state
        .db
        .create_report(state.bot_id, channel_msg_id as i32, &user_hash, reason)
        .await?;

    notify_admins_about_report(bot, state, reason, lang).await;

    state.user_states.remove(&user_id);
    bot.send_message(chat_id, l10n!(lang, "report_accepted"))
        .await?;
    Ok(())
}

pub(super) async fn notify_admins_about_report(
    bot: &Bot,
    state: &WorkerState,
    reason: &str,
    lang: Locale,
) {
    let admins = state.db.get_admins(state.bot_id).await.unwrap_or_default();
    let notif = l10n!(lang, "new_report_notif", reason = reason);
    for admin in admins {
        if !admin.frozen {
            let _ = bot.send_message(ChatId(admin.user_id), &notif).await;
        }
    }
}

pub(super) async fn show_next_report(
    bot: &Bot,
    chat_id: ChatId,
    state: &WorkerState,
    lang: Locale,
) -> R {
    let reports = state.db.get_pending_reports(state.bot_id).await?;
    if reports.is_empty() {
        bot.send_message(chat_id, l10n!(lang, "no_pending_reports"))
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
    let text = l10n!(
        lang,
        "report_display",
        id = report.id,
        date = date_str,
        reason = report.reason,
        link = post_link
    );
    let kb = InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::url(
            l10n!(lang, "report_view_post_btn"),
            url,
        )],
        vec![
            InlineKeyboardButton::callback(
                l10n!(lang, "report_dismiss_btn"),
                format!("report_dismiss_{}", report.id),
            ),
            InlineKeyboardButton::callback(
                l10n!(lang, "delete_post_btn"),
                format!("report_delete_{}", report.id),
            ),
            InlineKeyboardButton::callback(
                l10n!(lang, "report_ban_btn"),
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
        .text(l10n!(lang, "report_dismissed_msg"))
        .await?;
    if let Some(teloxide::types::MaybeInaccessibleMessage::Regular(msg)) = &q.message {
        bot.delete_message(msg.chat.id, msg.id).await.ok();
    }
    show_next_report(bot, chat_id, state, lang).await?;
    Ok(())
}

pub(super) async fn handle_report_delete(
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
                .text(l10n!(lang, "report_gone"))
                .await?;
            return Ok(());
        }
    };
    let cfg = state.config.read().await;
    let channel_id = cfg.channel_id;
    drop(cfg);
    let _ = bot
        .delete_message(
            ChatId(channel_id),
            teloxide::types::MessageId(report.channel_msg_id),
        )
        .await;
    state.db.resolve_report(state.bot_id, report_id).await?;
    let _ = state
        .db
        .log_action(
            state.bot_id,
            admin_id,
            &admin_name,
            "delete_post",
            &report_id.to_string(),
        )
        .await;
    bot.answer_callback_query(q.id.clone())
        .text(l10n!(lang, "post_deleted_msg"))
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
                .text(l10n!(lang, "report_gone"))
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
            .text(l10n!(lang, "reporter_already_banned"))
            .await?;
        if let Some(teloxide::types::MaybeInaccessibleMessage::Regular(msg)) = &q.message {
            bot.delete_message(msg.chat.id, msg.id).await.ok();
        }
        show_next_report(bot, chat_id, state, lang).await?;
        return Ok(());
    }
    let ban_id = Database::new_ban_id();
    let ban_reason = l10n!(lang, "report_ban_reason", reason = report.reason);
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
                .text(l10n!(lang, "reporter_banned_msg", ban_id = ban_id))
                .await?;
            if let Some(teloxide::types::MaybeInaccessibleMessage::Regular(msg)) = &q.message {
                bot.delete_message(msg.chat.id, msg.id).await.ok();
            }
            show_next_report(bot, chat_id, state, lang).await?;
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to ban reporter");
            bot.answer_callback_query(q.id.clone())
                .text(l10n!(lang, "error_banning_user"))
                .await?;
        }
    }
    Ok(())
}
