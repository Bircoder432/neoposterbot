use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    Ru,
}

impl Locale {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "en" => Some(Locale::En),
            "ru" => Some(Locale::Ru),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Ru => "ru",
        }
    }
}

#[derive(Deserialize)]
struct Translations {
    en: HashMap<String, String>,
    ru: HashMap<String, String>,
}

static TRANSLATIONS: OnceLock<Translations> = OnceLock::new();

fn translations() -> &'static Translations {
    TRANSLATIONS.get_or_init(|| {
        let raw = include_str!("locales.ron");
        let parsed: Translations =
            ron::from_str(raw).expect("src/locales.ron is broken - fix the syntax before shipping");

        for key in parsed.en.keys() {
            if !parsed.ru.contains_key(key) {
                tracing::error!(key = %key, "locales.ron: key is missing in `ru`");
            }
        }
        for key in parsed.ru.keys() {
            if !parsed.en.contains_key(key) {
                tracing::error!(key = %key, "locales.ron: key is missing in `en`");
            }
        }
        parsed
    })
}

pub fn raw<'a>(l: Locale, key: &'a str) -> &'a str {
    let t = translations();
    let (primary, fallback) = match l {
        Locale::En => (&t.en, &t.ru),
        Locale::Ru => (&t.ru, &t.en),
    };
    primary
        .get(key)
        .or_else(|| fallback.get(key))
        .map(|s| s.as_str())
        .unwrap_or(key)
}

pub fn t(l: Locale, key: &str, args: &[(&str, &str)]) -> String {
    let mut out = raw(l, key).to_string();
    for (name, value) in args {
        let placeholder = format!("{{{name}}}");
        out = out.replace(&placeholder, value);
    }
    out
}

pub struct L10n;

impl L10n {
    pub fn welcome(l: Locale) -> &'static str {
        raw(l, "welcome")
    }

    pub fn owner_panel(l: Locale) -> &'static str {
        raw(l, "owner_panel")
    }

    pub fn mod_panel(l: Locale) -> &'static str {
        raw(l, "mod_panel")
    }

    pub fn mod_frozen_panel(l: Locale) -> &'static str {
        raw(l, "mod_frozen_panel")
    }

    pub fn mod_frozen_action(l: Locale) -> &'static str {
        raw(l, "mod_frozen_action")
    }

    pub fn no_access(l: Locale) -> &'static str {
        raw(l, "no_access")
    }

    pub fn master_help(l: Locale) -> &'static str {
        raw(l, "master_help")
    }

    pub fn only_owner_add_admins(l: Locale) -> &'static str {
        raw(l, "only_owner_add_admins")
    }

    pub fn only_owner_remove_admins(l: Locale) -> &'static str {
        raw(l, "only_owner_remove_admins")
    }

    pub fn only_owner_list_admins(l: Locale) -> &'static str {
        raw(l, "only_owner_list_admins")
    }

    pub fn add_admin_usage(l: Locale) -> &'static str {
        raw(l, "add_admin_usage")
    }

    pub fn remove_admin_usage(l: Locale) -> &'static str {
        raw(l, "remove_admin_usage")
    }

    pub fn admin_already_exists(l: Locale, id: i64) -> String {
        let id = id.to_string();
        t(l, "admin_already_exists", &[("id", &id)])
    }

    pub fn admin_added(l: Locale, name: &str) -> String {
        t(l, "admin_added", &[("name", name)])
    }

    pub fn admin_added_notification(l: Locale) -> &'static str {
        raw(l, "admin_added_notification")
    }

    pub fn admin_added_frozen_notification(l: Locale) -> &'static str {
        raw(l, "admin_added_frozen_notification")
    }

    pub fn admin_added_frozen(l: Locale, name: &str) -> String {
        t(l, "admin_added_frozen", &[("name", name)])
    }

    pub fn admin_invite_link(l: Locale, link: &str) -> String {
        t(l, "admin_invite_link", &[("link", link)])
    }

    pub fn admin_invite_invalid(l: Locale) -> &'static str {
        raw(l, "admin_invite_invalid")
    }

    pub fn admin_invite_already_admin(l: Locale) -> &'static str {
        raw(l, "admin_invite_already_admin")
    }

    pub fn admin_not_found(l: Locale) -> &'static str {
        raw(l, "admin_not_found")
    }

    pub fn admin_removed(l: Locale, id: i64) -> String {
        let id = id.to_string();
        t(l, "admin_removed", &[("id", &id)])
    }

    pub fn admin_removed_notification(l: Locale) -> &'static str {
        raw(l, "admin_removed_notification")
    }

    pub fn no_admins(l: Locale) -> &'static str {
        raw(l, "no_admins")
    }

    pub fn no_admins_to_remove(l: Locale) -> &'static str {
        raw(l, "no_admins_to_remove")
    }

    pub fn admins_remove_hint(l: Locale) -> &'static str {
        raw(l, "admins_remove_hint")
    }

    pub fn admins_manage_hint(l: Locale) -> &'static str {
        raw(l, "admins_manage_hint")
    }

    pub fn admins_list_header(l: Locale, owner_id: i64) -> String {
        let owner_id = owner_id.to_string();
        t(l, "admins_list_header", &[("owner_id", &owner_id)])
    }

    pub fn admin_manage_title(l: Locale, name: &str, id: i64) -> String {
        let id = id.to_string();
        t(l, "admin_manage_title", &[("name", name), ("id", &id)])
    }

    pub fn confirm_remove_admin(l: Locale, name: &str, id: i64) -> String {
        let id = id.to_string();
        t(l, "confirm_remove_admin", &[("name", name), ("id", &id)])
    }

    pub fn confirm_yes(l: Locale) -> &'static str {
        raw(l, "confirm_yes")
    }

    pub fn confirm_no(l: Locale) -> &'static str {
        raw(l, "confirm_no")
    }

    pub fn back_to_list_btn(l: Locale) -> &'static str {
        raw(l, "back_to_list_btn")
    }

    pub fn back_btn(l: Locale) -> &'static str {
        raw(l, "back_btn")
    }

    pub fn freeze_btn(l: Locale) -> &'static str {
        raw(l, "freeze_btn")
    }

    pub fn unfreeze_btn(l: Locale) -> &'static str {
        raw(l, "unfreeze_btn")
    }

    pub fn remove_btn(l: Locale) -> &'static str {
        raw(l, "remove_btn")
    }

    pub fn revoke_invite_btn(l: Locale) -> &'static str {
        raw(l, "revoke_invite_btn")
    }

    pub fn invite_revoked_msg(l: Locale) -> &'static str {
        raw(l, "invite_revoked_msg")
    }

    pub fn admin_frozen_msg(l: Locale) -> &'static str {
        raw(l, "admin_frozen_msg")
    }

    pub fn admin_unfrozen_msg(l: Locale) -> &'static str {
        raw(l, "admin_unfrozen_msg")
    }

    pub fn limit_reached_unfreeze(l: Locale) -> &'static str {
        raw(l, "limit_reached_unfreeze")
    }

    pub fn free_limit_admins(l: Locale) -> &'static str {
        raw(l, "free_limit_admins")
    }

    // ===== модерация предложений =====

    pub fn no_new_proposals(l: Locale) -> &'static str {
        raw(l, "no_new_proposals")
    }

    pub fn proposal_items_count(l: Locale, count: usize) -> String {
        let count = count.to_string();
        t(l, "proposal_items_count", &[("count", &count)])
    }

    pub fn failed_display_media(l: Locale, e: &str) -> String {
        t(l, "failed_display_media", &[("e", e)])
    }

    pub fn proposal_action_header(l: Locale, id: u64, date: &str) -> String {
        let id = id.to_string();
        t(l, "proposal_action_header", &[("id", &id), ("date", date)])
    }

    pub fn approve_btn(l: Locale) -> &'static str {
        raw(l, "approve_btn")
    }

    pub fn reject_btn(l: Locale) -> &'static str {
        raw(l, "reject_btn")
    }

    pub fn published(l: Locale) -> &'static str {
        raw(l, "published")
    }

    pub fn published_no_id(l: Locale) -> &'static str {
        raw(l, "published_no_id")
    }

    pub fn failed_publish(l: Locale) -> &'static str {
        raw(l, "failed_publish")
    }

    pub fn rejected(l: Locale) -> &'static str {
        raw(l, "rejected")
    }

    pub fn already_processing(l: Locale) -> &'static str {
        raw(l, "already_processing")
    }

    pub fn proposal_gone(l: Locale) -> &'static str {
        raw(l, "proposal_gone")
    }

    pub fn returned_to_queue(l: Locale) -> &'static str {
        raw(l, "returned_to_queue")
    }

    pub fn choose_action(l: Locale) -> &'static str {
        raw(l, "choose_action")
    }

    pub fn reason_btn(l: Locale) -> &'static str {
        raw(l, "reason_btn")
    }

    pub fn next_btn(l: Locale) -> &'static str {
        raw(l, "next_btn")
    }

    pub fn ban_btn(l: Locale) -> &'static str {
        raw(l, "ban_btn")
    }

    pub fn enter_rejection_reason(l: Locale) -> &'static str {
        raw(l, "enter_rejection_reason")
    }

    pub fn enter_ban_reason(l: Locale) -> &'static str {
        raw(l, "enter_ban_reason")
    }

    pub fn enter_reason_callback(l: Locale) -> &'static str {
        raw(l, "enter_reason_callback")
    }

    pub fn reason_sent(l: Locale) -> &'static str {
        raw(l, "reason_sent")
    }

    pub fn rejected_reason(l: Locale, reason: &str) -> String {
        t(l, "rejected_reason", &[("reason", reason)])
    }

    pub fn new_proposal_notif(l: Locale, text: &str, media_type: &str) -> String {
        t(
            l,
            "new_proposal_notif",
            &[("text", text), ("media_type", media_type)],
        )
    }

    pub fn proposal_accepted(l: Locale) -> &'static str {
        raw(l, "proposal_accepted")
    }

    pub fn reply_accepted(l: Locale) -> &'static str {
        raw(l, "reply_accepted")
    }

    pub fn error_sending_reply(l: Locale) -> &'static str {
        raw(l, "error_sending_reply")
    }

    pub fn no_active_bans(l: Locale) -> &'static str {
        raw(l, "no_active_bans")
    }

    pub fn active_bans_header(l: Locale) -> &'static str {
        raw(l, "active_bans_header")
    }

    pub fn ban_record_entry(l: Locale, i: usize, ban_id: &str, reason: &str, date: &str) -> String {
        let i = i.to_string();
        t(
            l,
            "ban_record_entry",
            &[
                ("i", &i),
                ("ban_id", ban_id),
                ("reason", reason),
                ("date", date),
            ],
        )
    }

    pub fn pardon_usage(l: Locale) -> &'static str {
        raw(l, "pardon_usage")
    }

    pub fn ban_not_found(l: Locale) -> &'static str {
        raw(l, "ban_not_found")
    }

    pub fn access_restored(l: Locale) -> &'static str {
        raw(l, "access_restored")
    }

    pub fn ban_deactivated(l: Locale, ban_id: &str) -> String {
        t(l, "ban_deactivated", &[("ban_id", ban_id)])
    }

    pub fn user_banned(l: Locale) -> &'static str {
        raw(l, "user_banned")
    }

    pub fn user_banned_appeal(l: Locale, reason: &str, ban_id: &str) -> String {
        t(
            l,
            "user_banned_appeal",
            &[("reason", reason), ("ban_id", ban_id)],
        )
    }

    pub fn user_banned_success(l: Locale) -> &'static str {
        raw(l, "user_banned_success")
    }

    pub fn error_banning_user(l: Locale) -> &'static str {
        raw(l, "error_banning_user")
    }

    pub fn reply_usage(l: Locale) -> &'static str {
        raw(l, "reply_usage")
    }

    pub fn send_reply_to_post(l: Locale) -> &'static str {
        raw(l, "send_reply_to_post")
    }

    pub fn invalid_reply_link(l: Locale) -> &'static str {
        raw(l, "invalid_reply_link")
    }

    pub fn post_not_found(l: Locale) -> &'static str {
        raw(l, "post_not_found")
    }

    pub fn reply_link_text(l: Locale) -> &'static str {
        raw(l, "reply_link_text")
    }

    pub fn reply_quote(l: Locale, quoted: &str) -> String {
        t(l, "reply_quote", &[("quoted", quoted)])
    }

    pub fn mode_cancelled(l: Locale) -> &'static str {
        raw(l, "mode_cancelled")
    }

    pub fn no_active_mode(l: Locale) -> &'static str {
        raw(l, "no_active_mode")
    }

    pub fn addreplies_usage(l: Locale) -> &'static str {
        raw(l, "addreplies_usage")
    }

    pub fn addreplies_invalid_link(l: Locale) -> &'static str {
        raw(l, "addreplies_invalid_link")
    }

    pub fn addreplies_already_exists(l: Locale) -> &'static str {
        raw(l, "addreplies_already_exists")
    }

    pub fn addreplies_success(l: Locale) -> &'static str {
        raw(l, "addreplies_success")
    }

    pub fn addreplies_failed_get(l: Locale, e: &str) -> String {
        t(l, "addreplies_failed_get", &[("e", e)])
    }

    pub fn addreplies_failed_edit(l: Locale, e: &str) -> String {
        t(l, "addreplies_failed_edit", &[("e", e)])
    }

    pub fn text_proposal(l: Locale, caption: &str) -> String {
        t(l, "text_proposal", &[("caption", caption)])
    }

    pub fn media_proposal_text(l: Locale, media_type: &str, caption: &str) -> String {
        t(
            l,
            "media_proposal_text",
            &[("media_type", media_type), ("caption", caption)],
        )
    }

    pub fn photo_text(l: Locale) -> &'static str {
        raw(l, "photo_text")
    }

    pub fn document_text(l: Locale, name: &str) -> String {
        t(l, "document_text", &[("name", name)])
    }

    pub fn video_text(l: Locale) -> &'static str {
        raw(l, "video_text")
    }

    pub fn video_note_text(l: Locale) -> &'static str {
        raw(l, "video_note_text")
    }

    pub fn audio_text(l: Locale, title: &str) -> String {
        t(l, "audio_text", &[("title", title)])
    }

    pub fn voice_text(l: Locale) -> &'static str {
        raw(l, "voice_text")
    }

    pub fn sticker_text(l: Locale) -> &'static str {
        raw(l, "sticker_text")
    }

    pub fn media_content_text(l: Locale) -> &'static str {
        raw(l, "media_content_text")
    }

    pub fn make_by(l: Locale, username: &str) -> String {
        t(l, "make_by", &[("username", username)])
    }

    pub fn lang_usage(l: Locale) -> &'static str {
        raw(l, "lang_usage")
    }

    pub fn lang_invalid(l: Locale) -> &'static str {
        raw(l, "lang_invalid")
    }

    pub fn lang_updated(l: Locale) -> &'static str {
        raw(l, "lang_updated")
    }

    pub fn setup_checking(l: Locale) -> &'static str {
        raw(l, "setup_checking")
    }

    pub fn setup_not_admin(l: Locale) -> &'static str {
        raw(l, "setup_not_admin")
    }

    pub fn setup_chat_not_found(l: Locale) -> &'static str {
        raw(l, "setup_chat_not_found")
    }

    pub fn setup_success(l: Locale, channel_name: &str) -> String {
        t(l, "setup_success", &[("channel_name", channel_name)])
    }

    pub fn setup_enter_channel(l: Locale) -> &'static str {
        raw(l, "setup_enter_channel")
    }

    pub fn send_command_in_your_channel(l: Locale) -> &'static str {
        raw(l, "send_command_in_your_channel")
    }

    // ===== мастер-бот: клиенты =====

    pub fn clients_list_title(l: Locale) -> &'static str {
        raw(l, "clients_list_title")
    }

    pub fn clients_empty(l: Locale) -> &'static str {
        raw(l, "clients_empty")
    }

    pub fn client_not_found(l: Locale) -> &'static str {
        raw(l, "client_not_found")
    }

    pub fn client_details(
        l: Locale,
        tg_id: i64,
        plan: &str,
        expires: Option<&str>,
        banned: bool,
        bots_count: usize,
    ) -> String {
        let plan_str = match plan {
            "pro" => match expires {
                Some(date) => t(l, "client_plan_pro_until", &[("date", date)]),
                None => raw(l, "client_plan_pro").to_string(),
            },
            _ => raw(l, "client_plan_free").to_string(),
        };
        let status = if banned {
            raw(l, "client_status_banned")
        } else {
            raw(l, "client_status_active")
        };
        let tg_id = tg_id.to_string();
        let bots_count = bots_count.to_string();
        t(
            l,
            "client_details",
            &[
                ("id_label", raw(l, "field_tg_id")),
                ("tg_id", &tg_id),
                ("plan_label", raw(l, "field_plan")),
                ("plan", &plan_str),
                ("status_label", raw(l, "field_status")),
                ("status", status),
                ("bots_label", raw(l, "field_bots")),
                ("bots", &bots_count),
            ],
        )
    }

    pub fn ban_client_btn(l: Locale) -> &'static str {
        raw(l, "ban_client_btn")
    }

    pub fn unban_client_btn(l: Locale) -> &'static str {
        raw(l, "unban_client_btn")
    }

    pub fn change_plan_btn(l: Locale) -> &'static str {
        raw(l, "change_plan_btn")
    }

    pub fn make_free(l: Locale) -> &'static str {
        raw(l, "make_free")
    }

    pub fn make_pro(l: Locale) -> &'static str {
        raw(l, "make_pro")
    }

    pub fn client_bots_btn(l: Locale) -> &'static str {
        raw(l, "client_bots_btn")
    }

    pub fn client_bots_title(l: Locale) -> &'static str {
        raw(l, "client_bots_title")
    }

    pub fn client_banned_msg(l: Locale) -> &'static str {
        raw(l, "client_banned_msg")
    }

    pub fn client_unbanned_msg(l: Locale) -> &'static str {
        raw(l, "client_unbanned_msg")
    }

    pub fn plan_changed_pro_msg(l: Locale) -> &'static str {
        raw(l, "plan_changed_pro_msg")
    }

    pub fn plan_changed_free_msg(l: Locale) -> &'static str {
        raw(l, "plan_changed_free_msg")
    }

    pub fn no_bots_msg(l: Locale) -> &'static str {
        raw(l, "no_bots_msg")
    }

    pub fn bot_info_text(l: Locale, username: &str, channel_display: &str, active: bool) -> String {
        let status = if active {
            raw(l, "bot_status_active")
        } else {
            raw(l, "bot_status_inactive")
        };
        t(
            l,
            "bot_info",
            &[
                ("username_label", raw(l, "field_username")),
                ("username", username),
                ("channel_label", raw(l, "field_channel")),
                ("channel", channel_display),
                ("status_label", raw(l, "field_status")),
                ("status", status),
            ],
        )
    }

    pub fn channel_not_configured(l: Locale) -> &'static str {
        raw(l, "channel_not_configured")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ron_is_valid_and_languages_are_in_sync() {
        let t = translations();
        assert!(!t.en.is_empty(), "locales.ron: `en` is empty");
        assert!(!t.ru.is_empty(), "locales.ron: `ru` is empty");
        for key in t.en.keys() {
            assert!(
                t.ru.contains_key(key),
                "locales.ron: missing `ru` key: {key}"
            );
        }
        for key in t.ru.keys() {
            assert!(
                t.en.contains_key(key),
                "locales.ron: missing `en` key: {key}"
            );
        }
    }

    #[test]
    fn placeholders_are_rendered() {
        assert_eq!(
            t(Locale::En, "admin_removed", &[("id", "42")]),
            "✅ Admin 42 removed."
        );
        assert_eq!(
            t(Locale::Ru, "ban_deactivated", &[("ban_id", "BAN-ABC123")]),
            "✅ Бан BAN-ABC123 деактивирован."
        );
    }

    #[test]
    fn fallback_returns_key_when_missing() {
        assert_eq!(
            raw(Locale::En, "definitely_not_a_key"),
            "definitely_not_a_key"
        );
    }
}
