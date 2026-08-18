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

pub struct L10n;

impl L10n {
    pub fn welcome(l: Locale) -> &'static str {
        match l {
            Locale::En => {
                "🤖 Welcome to the anonymous proposal bot!\n\nJust send your proposal, idea, or message here, and it will be anonymously reviewed by moderators.\n\nYour identity will be hidden - moderators will only see the content of your message.\n\n❓ What you can send:\n• Text proposals\n• Photos\n• Documents\n• Videos\n• Video notes\n• Audio and voice messages\n• Stickers\n\nYour proposal will be reviewed shortly!"
            }
            Locale::Ru => {
                "🤖 Добро пожаловать в анонимную предложку!\n\nПросто отправьте сюда ваше предложение, идею или сообщение, и оно будет анонимно рассмотрено модераторами.\n\nВаша личность будет скрыта - модераторы увидят только содержание вашего сообщения.\n\n❓ Что можно отправлять:\n• Текстовые предложения\n• Фотографии\n• Документы\n• Видео\n• Кружочки (видеосообщения)\n• Аудио и голосовые сообщения\n• Стикеры\n\nВаше предложение будет рассмотрено в ближайшее время!"
            }
        }
    }

    pub fn only_owner_add_admins(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Only the owner can add admins.",
            Locale::Ru => "❌ Только владелец бота может добавлять администраторов.",
        }
    }

    pub fn add_admin_usage(l: Locale) -> &'static str {
        match l {
            Locale::En => "📝 Usage: /addadmin\n\nGenerates an invite link for a new admin.",
            Locale::Ru => {
                "📝 Использование: /addadmin\n\nГенерирует пригласительную ссылку для нового администратора."
            }
        }
    }

    pub fn admin_already_exists(l: Locale, id: i64) -> String {
        match l {
            Locale::En => format!("❌ User {id} is already an admin."),
            Locale::Ru => format!("❌ Пользователь {id} уже является администратором."),
        }
    }

    pub fn admin_added(l: Locale, name: &str) -> String {
        match l {
            Locale::En => format!("✅ User {name} added as admin!"),
            Locale::Ru => format!("✅ Пользователь {name} добавлен как администратор!"),
        }
    }

    pub fn admin_added_notification(l: Locale) -> &'static str {
        match l {
            Locale::En => {
                "🎉 You have been added as a moderator!\n\nUse /start to access the moderation panel."
            }
            Locale::Ru => {
                "🎉 Вы были добавлены как модератор бота-предложки!\n\nИспользуйте команду /start для доступа к панели модерации."
            }
        }
    }

    pub fn admin_added_frozen_notification(l: Locale) -> &'static str {
        match l {
            Locale::En => {
                "🎉 You have been added as a moderator, but you are currently ❄️ FROZEN.\n\nThe bot owner needs to upgrade to Pro to unfreeze you."
            }
            Locale::Ru => {
                "🎉 Вы были добавлены как модератор, но сейчас вы ❄️ ЗАМОРОЖЕНЫ.\n\nВладельцу бота нужно обновиться до Pro, чтобы разморозить вас."
            }
        }
    }

    pub fn admin_invite_link(l: Locale, link: &str) -> String {
        match l {
            Locale::En => format!(
                "🔗 Send this link to the person you want to add as admin:\n\n{link}\n\nThe link is valid for 24 hours and can only be used once."
            ),
            Locale::Ru => format!(
                "🔗 Отправьте эту ссылку человеку, которого хотите добавить как администратора:\n\n{link}\n\nСсылка действительна 24 часа и может быть использована только один раз."
            ),
        }
    }

    pub fn admin_invite_invalid(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ The invitation link is invalid, expired, or already used.",
            Locale::Ru => {
                "❌ Пригласительная ссылка недействительна, истекла или уже использована."
            }
        }
    }

    pub fn admin_invite_already_admin(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ You are already an admin of this bot.",
            Locale::Ru => "❌ Вы уже являетесь администратором этого бота.",
        }
    }

    pub fn only_owner_remove_admins(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Only the owner can remove admins.",
            Locale::Ru => "❌ Только владелец может удалять администраторов.",
        }
    }

    pub fn remove_admin_usage(l: Locale) -> &'static str {
        match l {
            Locale::En => "📝 Usage: /removeadmin <user_ID>",
            Locale::Ru => "📝 Использование: /removeadmin <ID_пользователя>",
        }
    }

    pub fn admin_not_found(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ User is not an admin.",
            Locale::Ru => "❌ Пользователь не является администратором.",
        }
    }

    pub fn admin_removed(l: Locale, id: i64) -> String {
        match l {
            Locale::En => format!("✅ Admin {id} removed."),
            Locale::Ru => format!("✅ Администратор {id} удалён."),
        }
    }

    pub fn admin_removed_notification(l: Locale) -> &'static str {
        match l {
            Locale::En => "⚠️ You are no longer a moderator.",
            Locale::Ru => "⚠️ Вы больше не являетесь модератором бота.",
        }
    }

    pub fn only_owner_list_admins(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Only the owner can list admins.",
            Locale::Ru => "❌ Только владелец может просматривать список администраторов.",
        }
    }

    pub fn no_admins(l: Locale) -> &'static str {
        match l {
            Locale::En => "📋 No moderators.",
            Locale::Ru => "📋 Список модераторов пуст.",
        }
    }

    pub fn no_admins_to_remove(l: Locale) -> &'static str {
        match l {
            Locale::En => "No admins to remove. Only you (the owner) are in the list.",
            Locale::Ru => "Нет администраторов для удаления. В списке только вы (владелец).",
        }
    }

    pub fn admins_remove_hint(l: Locale) -> &'static str {
        match l {
            Locale::En => "Tap a moderator to remove:",
            Locale::Ru => "Нажмите на модератора, чтобы удалить:",
        }
    }

    pub fn admins_list_header(l: Locale, owner_id: i64) -> String {
        match l {
            Locale::En => format!("📋 Moderators:\n\n👑 Owner: ID {owner_id}\n\n"),
            Locale::Ru => format!("📋 Список модераторов:\n\n👑 Владелец: ID {owner_id}\n\n"),
        }
    }

    pub fn confirm_remove_admin(l: Locale, name: &str, id: i64) -> String {
        match l {
            Locale::En => format!("⚠️ Remove admin {name} (ID: {id})?"),
            Locale::Ru => format!("⚠️ Удалить администратора {name} (ID: {id})?"),
        }
    }

    pub fn confirm_yes(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ Yes, remove",
            Locale::Ru => "✅ Да, удалить",
        }
    }

    pub fn confirm_no(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Cancel",
            Locale::Ru => "❌ Отмена",
        }
    }

    pub fn back_to_list_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "⬅️ Back to list",
            Locale::Ru => "⬅️ К списку",
        }
    }

    pub fn back_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "⬅️ Back",
            Locale::Ru => "⬅️ Назад",
        }
    }

    pub fn no_access(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ No access.",
            Locale::Ru => "❌ У вас нет доступа к этой функции.",
        }
    }

    pub fn no_active_bans(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ No active bans.",
            Locale::Ru => "✅ Список банов пуст.",
        }
    }

    pub fn active_bans_header(l: Locale) -> &'static str {
        match l {
            Locale::En => "🚫 Active bans:\n\n",
            Locale::Ru => "🚫 Активные блокировки:\n\n",
        }
    }

    pub fn ban_record_entry(l: Locale, i: usize, ban_id: &str, reason: &str, date: &str) -> String {
        match l {
            Locale::En => format!("{i}. {ban_id}\n   Reason: {reason}\n   Date: {date}\n"),
            Locale::Ru => format!("{i}. {ban_id}\n   Причина: {reason}\n   Дата: {date}\n"),
        }
    }

    pub fn pardon_usage(l: Locale) -> &'static str {
        match l {
            Locale::En => "📝 Usage: /pardon <BAN-ID>",
            Locale::Ru => "📝 Использование: /pardon <BAN-ID>",
        }
    }

    pub fn ban_not_found(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Ban not found or already pardoned.",
            Locale::Ru => "❌ Бан не найден или уже снят.",
        }
    }

    pub fn access_restored(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ Your access has been restored.",
            Locale::Ru => "✅ Ваш доступ восстановлен.",
        }
    }

    pub fn ban_deactivated(l: Locale, ban_id: &str) -> String {
        match l {
            Locale::En => format!("✅ Ban {ban_id} deactivated."),
            Locale::Ru => format!("✅ Бан {ban_id} деактивирован."),
        }
    }

    pub fn no_new_proposals(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ No new proposals.",
            Locale::Ru => "✅ Нет новых предложений.",
        }
    }

    pub fn proposal_items_count(l: Locale, count: usize) -> String {
        match l {
            Locale::En => format!("📨 Proposal ({count} item(s)):",),
            Locale::Ru => format!("📨 Предложение ({count} элем.):"),
        }
    }

    pub fn failed_display_media(l: Locale, e: &str) -> String {
        match l {
            Locale::En => format!("❌ Failed to display media: {e}"),
            Locale::Ru => format!("❌ Ошибка отображения медиа: {e}"),
        }
    }

    pub fn proposal_action_header(l: Locale, id: i64, date: &str) -> String {
        match l {
            Locale::En => format!("📨 Proposal #{id}\n⏰ {date}\n\nChoose action:"),
            Locale::Ru => format!("📨 Предложение #{id}\n⏰ {date}\n\nВыберите действие:"),
        }
    }

    pub fn approve_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ APPROVE",
            Locale::Ru => "✅ ОДОБРИТЬ",
        }
    }

    pub fn reject_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ REJECT",
            Locale::Ru => "❌ ОТКЛОНИТЬ",
        }
    }

    pub fn published(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ Published!",
            Locale::Ru => "✅ Опубликовано!",
        }
    }

    pub fn published_no_id(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Published but failed to retrieve message ID",
            Locale::Ru => "❌ Опубликовано, но не удалось получить ID сообщения",
        }
    }

    pub fn failed_publish(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Failed to publish",
            Locale::Ru => "❌ Ошибка публикации",
        }
    }

    pub fn rejected(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ Rejected!",
            Locale::Ru => "✅ Отклонено!",
        }
    }

    pub fn reason_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "Reason",
            Locale::Ru => "Причина",
        }
    }

    pub fn next_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "Next",
            Locale::Ru => "Далее",
        }
    }

    pub fn ban_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "Ban",
            Locale::Ru => "Бан",
        }
    }

    pub fn choose_action(l: Locale) -> &'static str {
        match l {
            Locale::En => "Choose action:",
            Locale::Ru => "Выберите действие:",
        }
    }

    pub fn enter_rejection_reason(l: Locale) -> &'static str {
        match l {
            Locale::En => "Enter rejection reason:",
            Locale::Ru => "Введите причину отказа:",
        }
    }

    pub fn enter_reason_callback(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ Enter reason",
            Locale::Ru => "✅ Введите причину",
        }
    }

    pub fn enter_ban_reason(l: Locale) -> &'static str {
        match l {
            Locale::En => "Enter ban reason:",
            Locale::Ru => "Введите причину блокировки:",
        }
    }

    pub fn send_reply_to_post(l: Locale) -> &'static str {
        match l {
            Locale::En => "✍️ Send your reply to the post.",
            Locale::Ru => "✍️ Отправьте ваш ответ на пост.",
        }
    }

    pub fn invalid_reply_link(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Invalid reply link.",
            Locale::Ru => "❌ Неверная ссылка для ответа.",
        }
    }

    pub fn user_banned(l: Locale) -> &'static str {
        match l {
            Locale::En => "🚫 You are banned.",
            Locale::Ru => "🚫 Вы заблокированы.",
        }
    }

    pub fn owner_panel(l: Locale) -> &'static str {
        match l {
            Locale::En => {
                "👑 Owner Panel\n\nCommands:\n/addadmin — generate invite link\n/admins — manage admins\n/banned\n/proposals\n/pardon <BAN-ID>\n/reply <ID>\n/lang <en|ru>"
            }
            Locale::Ru => {
                "👑 Панель владельца\n\nКоманды:\n/addadmin — создать пригласительную ссылку\n/admins — управление администраторами\n/banned\n/proposals\n/pardon <BAN-ID>\n/reply <ID>\n/lang <en|ru>"
            }
        }
    }

    pub fn mod_panel(l: Locale) -> &'static str {
        match l {
            Locale::En => {
                "🛠️ Moderator Panel\n\nCommands:\n/proposals\n/banned\n/pardon <BAN-ID>\n/reply <ID>\n/lang <en|ru>"
            }
            Locale::Ru => {
                "🛠️ Панель модератора\n\nКоманды:\n/proposals\n/banned\n/pardon <BAN-ID>\n/reply <ID>\n/lang <en|ru>"
            }
        }
    }

    pub fn reply_usage(l: Locale) -> &'static str {
        match l {
            Locale::En => "📝 Usage: /reply <post_ID>",
            Locale::Ru => "📝 Использование: /reply <ID_поста>",
        }
    }

    pub fn post_not_found(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Post not found.",
            Locale::Ru => "❌ Пост не найден.",
        }
    }

    pub fn proposal_accepted(l: Locale) -> &'static str {
        match l {
            Locale::En => {
                "✅ Your proposal has been accepted! It will be reviewed by moderators anonymously."
            }
            Locale::Ru => {
                "✅ Ваше предложение принято! Оно будет рассмотрено модераторами анонимно."
            }
        }
    }

    pub fn reply_accepted(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ Your reply has been accepted!",
            Locale::Ru => "✅ Ваш ответ принят!",
        }
    }

    pub fn error_sending_reply(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Error sending reply.",
            Locale::Ru => "❌ Ошибка при отправке ответа.",
        }
    }

    pub fn rejected_reason(l: Locale, reason: &str) -> String {
        match l {
            Locale::En => format!("Your message was rejected. Reason: {reason}"),
            Locale::Ru => format!("Ваше сообщение отклонено по причине: {reason}"),
        }
    }

    pub fn reason_sent(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ Reason sent.",
            Locale::Ru => "✅ Причина отправлена.",
        }
    }

    pub fn user_banned_appeal(l: Locale, ban_id: &str) -> String {
        match l {
            Locale::En => format!("🚫 You are banned. Appeal code: {ban_id}"),
            Locale::Ru => format!("🚫 Вы заблокированы. Код обращения: {ban_id}"),
        }
    }

    pub fn user_banned_success(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ User banned.",
            Locale::Ru => "✅ Пользователь заблокирован.",
        }
    }

    pub fn error_banning_user(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Error banning user.",
            Locale::Ru => "❌ Ошибка при блокировке.",
        }
    }

    pub fn new_proposal_notif(l: Locale, text: &str, media_type: &str) -> String {
        match l {
            Locale::En => {
                format!("📨 New proposal!\n\n💬 {text}\n📁 Type: {media_type}\n\n/proposals")
            }
            Locale::Ru => {
                format!("📨 Новое предложение!\n\n💬 {text}\n📁 Тип: {media_type}\n\n/proposals")
            }
        }
    }

    pub fn text_proposal(l: Locale, caption: &str) -> String {
        match l {
            Locale::En => format!("💬 Text proposal:\n{caption}"),
            Locale::Ru => format!("💬 Текст предложения:\n{caption}"),
        }
    }

    pub fn media_proposal_text(l: Locale, media_type: &str, caption: &str) -> String {
        match l {
            Locale::En => format!("💬 {media_type}\n{caption}"),
            Locale::Ru => format!("💬 {media_type}\n{caption}"),
        }
    }

    pub fn photo_text(l: Locale) -> &'static str {
        match l {
            Locale::En => "🖼️ Photo",
            Locale::Ru => "🖼️ Фото",
        }
    }

    pub fn document_text(l: Locale, name: &str) -> String {
        match l {
            Locale::En => format!("📄 Document: {name}"),
            Locale::Ru => format!("📄 Документ: {name}"),
        }
    }

    pub fn video_text(l: Locale) -> &'static str {
        match l {
            Locale::En => "🎥 Video",
            Locale::Ru => "🎥 Видео",
        }
    }

    pub fn video_note_text(l: Locale) -> &'static str {
        match l {
            Locale::En => "📹 Video note",
            Locale::Ru => "📹 Кружочек (видеосообщение)",
        }
    }

    pub fn audio_text(l: Locale, title: &str) -> String {
        match l {
            Locale::En => format!("🎵 {title}"),
            Locale::Ru => format!("🎵 {title}"),
        }
    }

    pub fn voice_text(l: Locale) -> &'static str {
        match l {
            Locale::En => "🎤 Voice message",
            Locale::Ru => "🎤 Голосовое сообщение",
        }
    }

    pub fn sticker_text(l: Locale) -> &'static str {
        match l {
            Locale::En => "😊 Sticker",
            Locale::Ru => "😊 Стикер",
        }
    }

    pub fn media_content_text(l: Locale) -> &'static str {
        match l {
            Locale::En => "📦 Media content",
            Locale::Ru => "📦 Медиа-контент",
        }
    }

    pub fn reply_quote(l: Locale, quoted: &str) -> String {
        match l {
            Locale::En => format!("💬 Reply:\n\n{quoted}"),
            Locale::Ru => format!("💬 Ответ:\n\n{quoted}"),
        }
    }

    pub fn lang_usage(l: Locale) -> &'static str {
        match l {
            Locale::En => "📝 Usage: /lang <en|ru>",
            Locale::Ru => "📝 Использование: /lang <en|ru>",
        }
    }

    pub fn lang_invalid(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Invalid language. Use 'en' or 'ru'.",
            Locale::Ru => "❌ Неверный язык. Используйте 'en' или 'ru'.",
        }
    }

    pub fn lang_updated(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ Language updated to English.",
            Locale::Ru => "✅ Язык изменен на Русский.",
        }
    }

    pub fn reply_link_text(l: Locale) -> &'static str {
        match l {
            Locale::En => "💬 Reply",
            Locale::Ru => "💬 Ответить",
        }
    }

    pub fn free_limit_admins(l: Locale) -> &'static str {
        match l {
            Locale::En => {
                "❌ Free plan limit reached: you can only add 1 moderator. Please upgrade your plan."
            }
            Locale::Ru => {
                "❌ Достигнут лимит бесплатного тарифа: можно добавить только 1 модератора. Пожалуйста, улучшите тариф."
            }
        }
    }

    pub fn make_by(l: Locale, username: &str) -> String {
        match l {
            Locale::En => format!("\nmake by {username}"),
            Locale::Ru => format!("\nсделано с помощью {username}"),
        }
    }

    pub fn setup_checking(l: Locale) -> &'static str {
        match l {
            Locale::En => "🔍 Checking admin rights...",
            Locale::Ru => "🔍 Проверка прав администратора...",
        }
    }

    pub fn setup_not_admin(l: Locale) -> &'static str {
        match l {
            Locale::En => {
                "❌ I am not an administrator in this channel. Please add me and try again."
            }
            Locale::Ru => {
                "❌ Я не являюсь администратором в этом канале. Пожалуйста, добавьте меня и попробуйте снова."
            }
        }
    }

    pub fn setup_chat_not_found(l: Locale) -> &'static str {
        match l {
            Locale::En => {
                "❌ Channel not found. Make sure it is public or send the correct ID (-100...)."
            }
            Locale::Ru => {
                "❌ Канал не найден. Убедитесь, что он публичный, или отправьте корректный ID (-100...)."
            }
        }
    }

    pub fn setup_success(l: Locale, channel_name: &str) -> String {
        match l {
            Locale::En => format!(
                "✅ Setup complete! Channel '{}' linked successfully. Use /start to see the panel.",
                channel_name
            ),
            Locale::Ru => format!(
                "✅ Настройка завершена! Канал '{}' успешно привязан. Отправьте /start, чтобы открыть панель.",
                channel_name
            ),
        }
    }

    pub fn setup_enter_channel(l: Locale) -> &'static str {
        match l {
            Locale::En => {
                "➕ Now, please add the bot to your Telegram channel as an administrator."
            }
            Locale::Ru => "➕ Теперь добавьте бота в ваш Telegram-канал как администратора.",
        }
    }

    pub fn admin_added_frozen(l: Locale, name: &str) -> String {
        match l {
            Locale::En => format!(
                "✅ User {name} added as moderator, but ❄️ FROZEN.\n\nYou have reached the Free plan limit. Upgrade to Pro to unfreeze moderators."
            ),
            Locale::Ru => format!(
                "✅ Пользователь {name} добавлен как модератор, но ❄️ ЗАМОРОЖЕН.\n\nВы достигли лимита бесплатного тарифа. Обновите до Pro, чтобы разморозить модераторов."
            ),
        }
    }

    // ── Master bot: clients management ──

    pub fn clients_list_title(l: Locale) -> &'static str {
        match l {
            Locale::En => "👥 Clients list",
            Locale::Ru => "👥 Список клиентов",
        }
    }

    pub fn clients_empty(l: Locale) -> &'static str {
        match l {
            Locale::En => "No clients yet.",
            Locale::Ru => "Клиентов пока нет.",
        }
    }

    pub fn client_not_found(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Client not found.",
            Locale::Ru => "❌ Клиент не найден.",
        }
    }

    pub fn client_details(
        l: Locale,
        tg_id: i64,
        plan: &str,
        expires: Option<&str>,
        banned: bool,
        bots_count: usize,
    ) -> String {
        let plan_str = match l {
            Locale::En => match plan {
                "pro" => match expires {
                    Some(d) => format!("Pro (until {d})"),
                    None => "Pro".to_string(),
                },
                _ => "Free".to_string(),
            },
            Locale::Ru => match plan {
                "pro" => match expires {
                    Some(d) => format!("Pro (до {d})"),
                    None => "Pro".to_string(),
                },
                _ => "Free".to_string(),
            },
        };
        let status_str = if banned {
            match l {
                Locale::En => "🚫 Banned",
                Locale::Ru => "🚫 Забанен",
            }
        } else {
            match l {
                Locale::En => "✅ Active",
                Locale::Ru => "✅ Активен",
            }
        };
        let bots_label = match l {
            Locale::En => "Bots",
            Locale::Ru => "Ботов",
        };
        let id_label = match l {
            Locale::En => "Telegram ID",
            Locale::Ru => "Telegram ID",
        };
        let plan_label = match l {
            Locale::En => "Plan",
            Locale::Ru => "Тариф",
        };
        let status_label = match l {
            Locale::En => "Status",
            Locale::Ru => "Статус",
        };
        format!(
            "👤 <b>Client</b>\n\n{id_label}: <code>{tg_id}</code>\n{plan_label}: {plan_str}\n{status_label}: {status_str}\n{bots_label}: {bots_count}"
        )
    }

    pub fn ban_client_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "🚫 Ban",
            Locale::Ru => "🚫 Бан",
        }
    }

    pub fn unban_client_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ Unban",
            Locale::Ru => "✅ Разбан",
        }
    }

    pub fn change_plan_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "🔄 Change plan",
            Locale::Ru => "🔄 Изменить план",
        }
    }

    pub fn client_bots_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "🤖 Bots",
            Locale::Ru => "🤖 Боты",
        }
    }

    pub fn client_banned_msg(l: Locale) -> &'static str {
        match l {
            Locale::En => "🚫 Client banned. Bots stopped.",
            Locale::Ru => "🚫 Клиент забанен. Боты остановлены.",
        }
    }

    pub fn client_unbanned_msg(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ Client unbanned. Bots restarted.",
            Locale::Ru => "✅ Клиент разбанен. Боты перезапущены.",
        }
    }

    pub fn plan_changed_pro_msg(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ Plan changed to Pro (30 days).",
            Locale::Ru => "✅ План изменён на Pro (30 дней).",
        }
    }

    pub fn plan_changed_free_msg(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ Plan changed to Free.",
            Locale::Ru => "✅ План изменён на Free.",
        }
    }

    pub fn client_bots_title(l: Locale) -> &'static str {
        match l {
            Locale::En => "🤖 Client's bots:",
            Locale::Ru => "🤖 Боты клиента:",
        }
    }

    pub fn no_bots_msg(l: Locale) -> &'static str {
        match l {
            Locale::En => "This client has no bots.",
            Locale::Ru => "У этого клиента пока нет ботов.",
        }
    }

    pub fn bot_info_text(l: Locale, username: &str, channel_display: &str, active: bool) -> String {
        let active_str = if active {
            match l {
                Locale::En => "✅ Active",
                Locale::Ru => "✅ Активен",
            }
        } else {
            match l {
                Locale::En => "❌ Inactive",
                Locale::Ru => "❌ Неактивен",
            }
        };
        let username_label = match l {
            Locale::En => "Username",
            Locale::Ru => "Юзернейм",
        };
        let channel_label = match l {
            Locale::En => "Channel",
            Locale::Ru => "Канал",
        };
        let status_label = match l {
            Locale::En => "Status",
            Locale::Ru => "Статус",
        };
        format!(
            "🤖 <b>Bot info</b>\n\n{username_label}: @{username}\n{channel_label}: {channel_display}\n{status_label}: {active_str}"
        )
    }

    pub fn channel_not_configured(l: Locale) -> &'static str {
        match l {
            Locale::En => "Not configured",
            Locale::Ru => "Не настроен",
        }
    }

    pub fn master_help(l: Locale) -> &'static str {
        match l {
            Locale::En => {
                "👑 Admin panel:\n\n/stats — service statistics\n/clients — clients list (with inline management)\n/setplan <user_id> <free|pro> — set plan manually\n/banclient <user_id> — ban a client"
            }
            Locale::Ru => {
                "👑 Панель администратора:\n\n/stats — статистика сервиса\n/clients — список клиентов (с управлением через кнопки)\n/setplan <user_id> <free|pro> — выдать план вручную\n/banclient <user_id> — забанить клиента"
            }
        }
    }

    pub fn make_free(l: Locale) -> &'static str {
        match l {
            Locale::En => "🔄 Set Free",
            Locale::Ru => "🔄 Сделать Free",
        }
    }

    pub fn make_pro(l: Locale) -> &'static str {
        match l {
            Locale::En => "🔄 Set Pro",
            Locale::Ru => "🔄 Сделать Pro",
        }
    }

    pub fn revoke_invite_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "🗑 Revoke link",
            Locale::Ru => "🗑 Отозвать ссылку",
        }
    }

    pub fn invite_revoked_msg(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Invitation link revoked.",
            Locale::Ru => "❌ Пригласительная ссылка отозвана.",
        }
    }

    pub fn admins_manage_hint(l: Locale) -> &'static str {
        match l {
            Locale::En => "Tap a moderator to manage:",
            Locale::Ru => "Нажмите на модератора, чтобы управлять им:",
        }
    }

    pub fn admin_manage_title(l: Locale, name: &str, id: i64) -> String {
        match l {
            Locale::En => format!("⚙️ Manage moderator:\n\n{name} (ID: {id})"),
            Locale::Ru => format!("⚙️ Управление модератором:\n\n{name} (ID: {id})"),
        }
    }

    pub fn freeze_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "❄️ Freeze",
            Locale::Ru => "❄️ Заморозить",
        }
    }

    pub fn unfreeze_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "🔥 Unfreeze",
            Locale::Ru => "🔥 Разморозить",
        }
    }

    pub fn remove_btn(l: Locale) -> &'static str {
        match l {
            Locale::En => "🗑 Remove",
            Locale::Ru => "🗑 Удалить",
        }
    }

    pub fn limit_reached_unfreeze(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ Free plan limit reached. Freeze another moderator or upgrade to Pro.",
            Locale::Ru => {
                "❌ Достигнут лимит бесплатного тарифа. Заморозьте другого модератора или обновитесь до Pro."
            }
        }
    }

    pub fn admin_frozen_msg(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ Moderator frozen.",
            Locale::Ru => "✅ Модератор заморожен.",
        }
    }

    pub fn admin_unfrozen_msg(l: Locale) -> &'static str {
        match l {
            Locale::En => "✅ Moderator unfrozen.",
            Locale::Ru => "✅ Модератор разморожен.",
        }
    }

    // Добавьте эти методы в конец impl L10n
    pub fn mod_frozen_panel(l: Locale) -> &'static str {
        match l {
            Locale::En => {
                "🛠️ Moderator Panel\n\n⚠️ You are currently ❄️ FROZEN. You cannot perform moderation actions until the bot owner unfreezes you."
            }
            Locale::Ru => {
                "🛠️ Панель модератора\n\n⚠️ Вы сейчас ❄️ ЗАМОРОЖЕНЫ. Вы не можете выполнять модераторские действия, пока владелец бота вас не разморозит."
            }
        }
    }

    pub fn mod_frozen_action(l: Locale) -> &'static str {
        match l {
            Locale::En => "❌ You are frozen and cannot perform this action.",
            Locale::Ru => "❌ Вы заморожены и не можете выполнять это действие.",
        }
    }

    pub fn already_processing(l: Locale) -> &'static str {
        match l {
            Locale::En => "⏳ This proposal is already being processed.",
            Locale::Ru => "⏳ Это предложение уже обрабатывается.",
        }
    }
}
