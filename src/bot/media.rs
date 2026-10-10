use crate::db::models::{Proposal, ProposalMessage};
use crate::l10n;
use crate::locales::Locale;
use anyhow::Result;
use teloxide::prelude::*;
use teloxide::types::{
    FileId, InputFile, InputMedia, InputMediaAudio, InputMediaPhoto, InputMediaVideo,
    LinkPreviewOptions, Message as TgMessage, MessageId, ParseMode, ReplyParameters,
};

pub fn extract_media_info(msg: &TgMessage, _lang: Locale) -> (String, String) {
    if let Some(photo) = msg.photo().and_then(|p| p.last()) {
        return ("photo".into(), photo.file.id.to_string());
    }
    if let Some(doc) = msg.document() {
        return ("document".into(), doc.file.id.to_string());
    }
    if let Some(video) = msg.video() {
        return ("video".into(), video.file.id.to_string());
    }
    if let Some(audio) = msg.audio() {
        return ("audio".into(), audio.file.id.to_string());
    }
    if let Some(voice) = msg.voice() {
        return ("voice".into(), voice.file.id.to_string());
    }
    if let Some(sticker) = msg.sticker() {
        return ("sticker".into(), sticker.file.id.to_string());
    }
    if let Some(vn) = msg.video_note() {
        return ("video_note".into(), vn.file.id.to_string());
    }
    ("text".into(), String::new())
}

pub fn extract_message_text(msg: &TgMessage, lang: Locale) -> String {
    if let Some(text) = msg.text() {
        return text.to_string();
    }
    if let Some(caption) = msg.caption() {
        return caption.to_string();
    }
    if msg.media_group_id().is_some() {
        return String::new();
    }
    if msg.photo().map_or(false, |p| !p.is_empty()) {
        return l10n!(lang, "photo_text");
    }
    if let Some(doc) = msg.document() {
        return l10n!(
            lang,
            "document_text",
            name = doc.file_name.as_deref().unwrap_or("unknown")
        );
    }
    if msg.video().is_some() {
        return l10n!(lang, "video_text");
    }
    if msg.video_note().is_some() {
        return l10n!(lang, "video_note_text");
    }
    if let Some(audio) = msg.audio() {
        return l10n!(
            lang,
            "audio_text",
            title = audio.title.as_deref().unwrap_or("Audio")
        );
    }
    if msg.voice().is_some() {
        return l10n!(lang, "voice_text");
    }
    if msg.sticker().is_some() {
        return l10n!(lang, "sticker_text");
    }
    l10n!(lang, "media_content_text")
}

pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn input_file(file_id: &str) -> InputFile {
    InputFile::file_id(FileId(file_id.to_string()))
}

fn disabled_preview_options() -> LinkPreviewOptions {
    LinkPreviewOptions {
        is_disabled: true,
        url: None,
        prefer_small_media: false,
        prefer_large_media: false,
        show_above_text: false,
    }
}

pub async fn send_for_moderation(
    bot: &Bot,
    chat_id: ChatId,
    proposal: &Proposal,
    lang: Locale,
) -> Result<()> {
    if proposal.messages.len() == 1 {
        send_single_for_moderation(bot, chat_id, proposal.first(), lang).await
    } else {
        send_group_for_moderation(bot, chat_id, &proposal.messages, lang).await
    }
}

async fn send_single_for_moderation(
    bot: &Bot,
    chat_id: ChatId,
    msg: &ProposalMessage,
    lang: Locale,
) -> Result<()> {
    let media_type = msg.media_type.as_str();
    let file_id = msg.media_file_id.as_str();
    let caption = &msg.message_text;

    if media_type == "text" || file_id.is_empty() {
        bot.send_message(chat_id, l10n!(lang, "text_proposal", caption = caption))
            .await?;
        return Ok(());
    }

    let file = input_file(file_id);
    match media_type {
        "photo" => bot.send_photo(chat_id, file).caption(caption).await?,
        "document" => bot.send_document(chat_id, file).caption(caption).await?,
        "video" => bot.send_video(chat_id, file).caption(caption).await?,
        "audio" => bot.send_audio(chat_id, file).caption(caption).await?,
        "voice" => bot.send_voice(chat_id, file).caption(caption).await?,
        "sticker" => bot.send_sticker(chat_id, file).await?,
        "video_note" => bot.send_video_note(chat_id, file).await?,
        _ => {
            bot.send_message(
                chat_id,
                l10n!(
                    lang,
                    "media_proposal_text",
                    media_type = media_type,
                    caption = caption
                ),
            )
            .await?
        }
    };
    Ok(())
}

async fn send_group_for_moderation(
    bot: &Bot,
    chat_id: ChatId,
    messages: &[ProposalMessage],
    lang: Locale,
) -> Result<()> {
    let all_media = messages
        .iter()
        .all(|m| matches!(m.media_type.as_str(), "photo" | "video" | "audio"));
    if all_media {
        let media: Vec<InputMedia> = messages
            .iter()
            .enumerate()
            .map(|(i, m)| build_input_media(m, i == 0, false))
            .collect();
        bot.send_media_group(chat_id, media).await?;
    } else {
        for m in messages {
            send_single_for_moderation(bot, chat_id, m, lang).await?;
        }
    }
    Ok(())
}

fn build_input_media(msg: &ProposalMessage, is_first: bool, with_html: bool) -> InputMedia {
    let file = input_file(&msg.media_file_id);
    let caption = if is_first && !msg.message_text.is_empty() {
        Some(msg.message_text.clone())
    } else {
        None
    };
    let mut builder = match msg.media_type.as_str() {
        "video" => InputMediaBuilder::Video(InputMediaVideo::new(file)),
        "audio" => InputMediaBuilder::Audio(InputMediaAudio::new(file)),
        _ => InputMediaBuilder::Photo(InputMediaPhoto::new(file)),
    };
    if let Some(cap) = caption {
        builder = builder.caption(cap);
        if with_html {
            builder = builder.parse_mode(ParseMode::Html);
        }
    }
    builder.build()
}

enum InputMediaBuilder {
    Photo(InputMediaPhoto),
    Video(InputMediaVideo),
    Audio(InputMediaAudio),
}

impl InputMediaBuilder {
    fn caption(self, caption: String) -> Self {
        match self {
            Self::Photo(b) => Self::Photo(b.caption(caption)),
            Self::Video(b) => Self::Video(b.caption(caption)),
            Self::Audio(b) => Self::Audio(b.caption(caption)),
        }
    }
    fn parse_mode(self, mode: ParseMode) -> Self {
        match self {
            Self::Photo(b) => Self::Photo(b.parse_mode(mode)),
            Self::Video(b) => Self::Video(b.parse_mode(mode)),
            Self::Audio(b) => Self::Audio(b.parse_mode(mode)),
        }
    }
    fn build(self) -> InputMedia {
        match self {
            Self::Photo(b) => InputMedia::Photo(b),
            Self::Video(b) => InputMedia::Video(b),
            Self::Audio(b) => InputMedia::Audio(b),
        }
    }
}

pub async fn publish(
    bot: &Bot,
    channel_id: i64,
    proposal: &Proposal,
    bot_username: &str,
    watermark_username: &str,
    is_pro: bool,
    reply_to: Option<i32>,
    lang: Locale,
) -> Result<Option<i32>> {
    let first = proposal.first();
    let escaped_text = escape_html(&first.message_text);
    let quoted = format!("<blockquote>{escaped_text}</blockquote>");
    let is_reply = first.parent_message_id.is_some();
    let base = if is_reply {
        l10n!(lang, "reply_quote", quoted = quoted)
    } else {
        quoted
    };
    let watermark = if is_pro {
        "".to_string()
    } else {
        format!(
            "\n{}",
            l10n!(lang, "make_by", username = watermark_username).trim_start_matches('\n')
        )
    };
    let initial_caption = format!("{base}<i>{watermark}</i>");

    let channel_msg_id = if proposal.messages.len() == 1 {
        publish_single(bot, channel_id, first, &initial_caption, reply_to).await?
    } else {
        publish_group(bot, channel_id, &proposal.messages, &initial_caption).await?
    };

    if let Some(msg_id) = channel_msg_id {
        let reply_text = l10n!(lang, "reply_link_text");
        let report_text = l10n!(lang, "report_link_text");
        let links = format!(
            "\n\n<a href=\"https://t.me/{bot_username}?start=reply_{msg_id}\">{reply_text}</a> | <a href=\"https://t.me/{bot_username}?start=report_{msg_id}\">{report_text}</a>"
        );
        let final_caption = format!("{initial_caption}{links}");
        let _ = add_reply_link_to_post(bot, channel_id, msg_id, &final_caption, first).await;
    }
    Ok(channel_msg_id)
}

async fn add_reply_link_to_post(
    bot: &Bot,
    channel_id: i64,
    msg_id: i32,
    final_caption: &str,
    msg: &ProposalMessage,
) -> Result<()> {
    let chat = ChatId(channel_id);
    let media_type = msg.media_type.as_str();
    let file_id = msg.media_file_id.as_str();
    if media_type == "text" || file_id.is_empty() {
        bot.edit_message_text(chat, MessageId(msg_id), final_caption)
            .parse_mode(ParseMode::Html)
            .link_preview_options(disabled_preview_options())
            .await?;
    } else if matches!(media_type, "sticker" | "video_note") {
    } else {
        bot.edit_message_caption(chat, MessageId(msg_id))
            .caption(final_caption)
            .parse_mode(ParseMode::Html)
            .await?;
    }
    Ok(())
}

pub async fn add_reply_to_channel_post(
    bot: &Bot,
    channel_id: i64,
    msg_id: i32,
    new_caption: &str,
    forwarded_msg: &TgMessage,
) -> Result<()> {
    let chat = ChatId(channel_id);
    if forwarded_msg.sticker().is_some() || forwarded_msg.video_note().is_some() {
        anyhow::bail!("Cannot add reply button to sticker or video note posts");
    }
    if forwarded_msg.text().is_some() {
        bot.edit_message_text(chat, MessageId(msg_id), new_caption)
            .parse_mode(ParseMode::Html)
            .link_preview_options(disabled_preview_options())
            .await?;
    } else {
        bot.edit_message_caption(chat, MessageId(msg_id))
            .caption(new_caption)
            .parse_mode(ParseMode::Html)
            .await?;
    }
    Ok(())
}

async fn publish_single(
    bot: &Bot,
    channel_id: i64,
    msg: &ProposalMessage,
    caption: &str,
    reply_to: Option<i32>,
) -> Result<Option<i32>> {
    let chat = ChatId(channel_id);
    let media_type = msg.media_type.as_str();
    let file_id = msg.media_file_id.as_str();
    let sent = if media_type == "text" || file_id.is_empty() {
        let mut req = bot
            .send_message(chat, caption)
            .parse_mode(ParseMode::Html)
            .link_preview_options(disabled_preview_options());
        if let Some(id) = reply_to {
            req = req.reply_parameters(ReplyParameters::new(MessageId(id)));
        }
        req.await?
    } else {
        let file = input_file(file_id);
        match media_type {
            "photo" => {
                let mut req = bot
                    .send_photo(chat, file)
                    .caption(caption)
                    .parse_mode(ParseMode::Html);
                if let Some(id) = reply_to {
                    req = req.reply_parameters(ReplyParameters::new(MessageId(id)));
                }
                req.await?
            }
            "document" => {
                let mut req = bot
                    .send_document(chat, file)
                    .caption(caption)
                    .parse_mode(ParseMode::Html);
                if let Some(id) = reply_to {
                    req = req.reply_parameters(ReplyParameters::new(MessageId(id)));
                }
                req.await?
            }
            "video" => {
                let mut req = bot
                    .send_video(chat, file)
                    .caption(caption)
                    .parse_mode(ParseMode::Html);
                if let Some(id) = reply_to {
                    req = req.reply_parameters(ReplyParameters::new(MessageId(id)));
                }
                req.await?
            }
            "audio" => {
                let mut req = bot
                    .send_audio(chat, file)
                    .caption(caption)
                    .parse_mode(ParseMode::Html);
                if let Some(id) = reply_to {
                    req = req.reply_parameters(ReplyParameters::new(MessageId(id)));
                }
                req.await?
            }
            "voice" => {
                let mut req = bot
                    .send_voice(chat, file)
                    .caption(caption)
                    .parse_mode(ParseMode::Html);
                if let Some(id) = reply_to {
                    req = req.reply_parameters(ReplyParameters::new(MessageId(id)));
                }
                req.await?
            }
            "sticker" => {
                let mut req = bot.send_sticker(chat, file);
                if let Some(id) = reply_to {
                    req = req.reply_parameters(ReplyParameters::new(MessageId(id)));
                }
                req.await?
            }
            "video_note" => {
                let mut req = bot.send_video_note(chat, file);
                if let Some(id) = reply_to {
                    req = req.reply_parameters(ReplyParameters::new(MessageId(id)));
                }
                req.await?
            }
            _ => {
                let mut req = bot
                    .send_message(chat, caption)
                    .parse_mode(ParseMode::Html)
                    .link_preview_options(disabled_preview_options());
                if let Some(id) = reply_to {
                    req = req.reply_parameters(ReplyParameters::new(MessageId(id)));
                }
                req.await?
            }
        }
    };
    Ok(Some(sent.id.0))
}

async fn publish_group(
    bot: &Bot,
    channel_id: i64,
    messages: &[ProposalMessage],
    caption: &str,
) -> Result<Option<i32>> {
    let media: Vec<InputMedia> = messages
        .iter()
        .enumerate()
        .map(|(i, m)| {
            if i == 0 {
                let file = input_file(&m.media_file_id);
                match m.media_type.as_str() {
                    "video" => InputMedia::Video(
                        InputMediaVideo::new(file)
                            .caption(caption)
                            .parse_mode(ParseMode::Html),
                    ),
                    "audio" => InputMedia::Audio(
                        InputMediaAudio::new(file)
                            .caption(caption)
                            .parse_mode(ParseMode::Html),
                    ),
                    _ => InputMedia::Photo(
                        InputMediaPhoto::new(file)
                            .caption(caption)
                            .parse_mode(ParseMode::Html),
                    ),
                }
            } else {
                build_input_media(m, false, false)
            }
        })
        .collect();
    let sent_messages = bot.send_media_group(ChatId(channel_id), media).await?;
    Ok(sent_messages.first().map(|m| m.id.0))
}
