use super::models::*;
use crate::locales::Locale;
use anyhow::Result;
use chrono::Utc;
use sqlx::PgPool;

#[derive(Clone)]
pub struct Database {
    pub pool: PgPool,
}

impl Database {
    pub async fn connect(url: &str) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    pub async fn get_all_clients(&self) -> Result<Vec<Client>> {
        Ok(sqlx::query_as("SELECT * FROM clients WHERE banned = FALSE")
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn get_clients_paginated(&self, offset: i64, limit: i64) -> Result<Vec<Client>> {
        Ok(
            sqlx::query_as("SELECT * FROM clients ORDER BY created_at DESC OFFSET $1 LIMIT $2")
                .bind(offset)
                .bind(limit)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn count_clients(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM clients")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn get_client_by_tg_id(&self, tg_user_id: i64) -> Result<Option<Client>> {
        Ok(
            sqlx::query_as("SELECT * FROM clients WHERE tg_user_id = $1")
                .bind(tg_user_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn get_client_lang(&self, tg_user_id: i64) -> Result<Locale> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT lang FROM clients WHERE tg_user_id = $1")
                .bind(tg_user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|(v,)| Locale::parse(&v)).unwrap_or(Locale::Ru))
    }

    pub async fn set_client_lang(&self, tg_user_id: i64, lang: Locale) -> Result<()> {
        sqlx::query("UPDATE clients SET lang = $1 WHERE tg_user_id = $2")
            .bind(lang.as_str())
            .bind(tg_user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_all_bots(&self) -> Result<Vec<BotConfig>> {
        Ok(sqlx::query_as("SELECT b.*, c.tg_user_id as client_tg_id FROM bots b JOIN clients c ON b.client_id = c.id WHERE b.active = TRUE")
            .fetch_all(&self.pool).await?)
    }

    pub async fn get_bots_by_client_tg_id(&self, tg_user_id: i64) -> Result<Vec<BotConfig>> {
        Ok(sqlx::query_as("SELECT b.*, c.tg_user_id as client_tg_id FROM bots b JOIN clients c ON b.client_id = c.id WHERE c.tg_user_id = $1 AND b.active = TRUE")
            .bind(tg_user_id).fetch_all(&self.pool).await?)
    }

    pub async fn get_all_bots_by_client_tg_id(&self, tg_user_id: i64) -> Result<Vec<BotConfig>> {
        Ok(sqlx::query_as("SELECT b.*, c.tg_user_id as client_tg_id FROM bots b JOIN clients c ON b.client_id = c.id WHERE c.tg_user_id = $1 ORDER BY b.created_at DESC")
            .bind(tg_user_id).fetch_all(&self.pool).await?)
    }

    pub async fn toggle_client_plan(&self, tg_user_id: i64) -> Result<String> {
        let client = self.get_client_by_tg_id(tg_user_id).await?;
        if let Some(client) = client {
            let is_pro = client.pro_expires_at.map_or(false, |d| d > Utc::now());
            if is_pro {
                self.set_client_plan(tg_user_id, "free").await?;
                Ok("free".to_string())
            } else {
                self.set_client_plan(tg_user_id, "pro").await?;
                Ok("pro".to_string())
            }
        } else {
            anyhow::bail!("Client not found");
        }
    }

    pub async fn ban_client(&self, tg_user_id: i64) -> Result<Vec<i32>> {
        sqlx::query("UPDATE clients SET banned = TRUE WHERE tg_user_id = $1")
            .bind(tg_user_id)
            .execute(&self.pool)
            .await?;

        let bot_ids: Vec<(i32,)> = sqlx::query_as(
            "UPDATE bots SET active = FALSE WHERE client_id = (SELECT id FROM clients WHERE tg_user_id = $1) RETURNING id"
        )
        .bind(tg_user_id).fetch_all(&self.pool).await?;

        Ok(bot_ids.into_iter().map(|(id,)| id).collect())
    }

    pub async fn unban_client(&self, tg_user_id: i64) -> Result<Vec<BotConfig>> {
        sqlx::query("UPDATE clients SET banned = FALSE WHERE tg_user_id = $1")
            .bind(tg_user_id)
            .execute(&self.pool)
            .await?;

        let bots: Vec<BotConfig> = sqlx::query_as(
            "UPDATE bots SET active = TRUE \
             WHERE client_id = (SELECT id FROM clients WHERE tg_user_id = $1) \
             AND setup_complete = TRUE \
             RETURNING *, NULL as client_tg_id",
        )
        .bind(tg_user_id)
        .fetch_all(&self.pool)
        .await?;

        let bots = bots
            .into_iter()
            .map(|mut b| {
                b.client_tg_id = Some(tg_user_id);
                b
            })
            .collect();

        Ok(bots)
    }

    pub async fn deactivate_bot(&self, bot_id: i32) -> Result<()> {
        sqlx::query("UPDATE bots SET active = FALSE WHERE id = $1")
            .bind(bot_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_language(&self, bot_id: i32) -> Result<Locale> {
        let row: Option<(String,)> = sqlx::query_as("SELECT lang FROM bots WHERE id = $1")
            .bind(bot_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.and_then(|(v,)| Locale::parse(&v)).unwrap_or(Locale::En))
    }

    pub async fn save_message(&self, msg: &NewMessage) -> Result<bool> {
        let now = Utc::now();
        let res = sqlx::query(
            "INSERT INTO messages (bot_id, chat_id, telegram_message_id, sender_id, message_text, media_type, media_file_id, media_group_id, proposal_group_id, created_at, status, parent_message_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'pending', $11)
             ON CONFLICT (bot_id, chat_id, telegram_message_id) DO NOTHING"
        )
        .bind(msg.bot_id).bind(msg.chat_id).bind(msg.telegram_message_id).bind(msg.sender_id)
        .bind(&msg.message_text).bind(&msg.media_type).bind(&msg.media_file_id)
        .bind(&msg.media_group_id).bind(&msg.proposal_group_id).bind(now).bind(msg.parent_message_id)
        .execute(&self.pool).await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn message_exists(
        &self,
        bot_id: i32,
        chat_id: i64,
        telegram_message_id: i32,
    ) -> Result<bool> {
        let row: Option<(i32,)> = sqlx::query_as("SELECT 1 FROM messages WHERE bot_id = $1 AND chat_id = $2 AND telegram_message_id = $3")
            .bind(bot_id).bind(chat_id).bind(telegram_message_id).fetch_optional(&self.pool).await?;
        Ok(row.is_some())
    }

    pub async fn get_message_by_id(&self, bot_id: i32, id: i64) -> Result<Option<Message>> {
        Ok(
            sqlx::query_as("SELECT * FROM messages WHERE bot_id = $1 AND id = $2")
                .bind(bot_id)
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn get_next_pending_proposal(&self, bot_id: i32) -> Result<Option<(String,)>> {
        Ok(sqlx::query_as("SELECT proposal_group_id FROM messages WHERE bot_id = $1 AND status = 'pending' GROUP BY proposal_group_id ORDER BY MIN(created_at) ASC LIMIT 1")
            .bind(bot_id).fetch_optional(&self.pool).await?)
    }

    pub async fn get_proposal_by_group_id(
        &self,
        bot_id: i32,
        group_id: &str,
    ) -> Result<Vec<Message>> {
        Ok(sqlx::query_as(
            "SELECT * FROM messages WHERE bot_id = $1 AND proposal_group_id = $2 ORDER BY id ASC",
        )
        .bind(bot_id)
        .bind(group_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update_proposal_status(
        &self,
        bot_id: i32,
        group_id: &str,
        status: &str,
    ) -> Result<()> {
        sqlx::query("UPDATE messages SET status = $1 WHERE bot_id = $2 AND proposal_group_id = $3")
            .bind(status)
            .bind(bot_id)
            .bind(group_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_proposal(&self, bot_id: i32, group_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM messages WHERE bot_id = $1 AND proposal_group_id = $2")
            .bind(bot_id)
            .bind(group_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_channel_message_id(
        &self,
        bot_id: i32,
        group_id: &str,
        msg_id: i32,
    ) -> Result<()> {
        sqlx::query("UPDATE messages SET channel_message_id = $1 WHERE bot_id = $2 AND proposal_group_id = $3")
            .bind(msg_id).bind(bot_id).bind(group_id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn remove_admin(&self, bot_id: i32, user_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM admins WHERE bot_id = $1 AND user_id = $2")
            .bind(bot_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_admins(&self, bot_id: i32) -> Result<Vec<Admin>> {
        Ok(
            sqlx::query_as("SELECT * FROM admins WHERE bot_id = $1 ORDER BY id ASC")
                .bind(bot_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn count_admins(&self, bot_id: i32) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM admins WHERE bot_id = $1")
            .bind(bot_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn ban_user(&self, bot_id: i32, user_id: i64) -> Result<()> {
        sqlx::query(
            "INSERT INTO banned_users (bot_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(bot_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn is_banned(&self, bot_id: i32, user_id: i64) -> Result<bool> {
        let row: Option<(i32,)> =
            sqlx::query_as("SELECT 1 FROM banned_users WHERE bot_id = $1 AND user_id = $2")
                .bind(bot_id)
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
    }

    pub async fn pardon_user(&self, bot_id: i32, user_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM banned_users WHERE bot_id = $1 AND user_id = $2")
            .bind(bot_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn create_ban_record(
        &self,
        bot_id: i32,
        user_id: i64,
        reason: &str,
    ) -> Result<String> {
        let ban_id = format!(
            "BAN-{}",
            &uuid::Uuid::new_v4().simple().to_string().to_uppercase()[..6]
        );
        let now = Utc::now();
        sqlx::query("INSERT INTO ban_records (bot_id, ban_id, user_id, reason, created_at) VALUES ($1, $2, $3, $4, $5)")
            .bind(bot_id).bind(&ban_id).bind(user_id).bind(reason).bind(now).execute(&self.pool).await?;
        Ok(ban_id)
    }

    pub async fn get_ban_record(&self, bot_id: i32, ban_id: &str) -> Result<Option<BanRecord>> {
        Ok(sqlx::query_as("SELECT ban_id, user_id, reason, created_at FROM ban_records WHERE bot_id = $1 AND ban_id = $2 AND active = TRUE")
            .bind(bot_id).bind(ban_id).fetch_optional(&self.pool).await?)
    }

    pub async fn get_active_ban_records(&self, bot_id: i32) -> Result<Vec<BanRecord>> {
        Ok(sqlx::query_as("SELECT ban_id, user_id, reason, created_at FROM ban_records WHERE bot_id = $1 AND active = TRUE ORDER BY created_at DESC")
            .bind(bot_id).fetch_all(&self.pool).await?)
    }

    pub async fn deactivate_ban(&self, bot_id: i32, ban_id: &str) -> Result<()> {
        sqlx::query("UPDATE ban_records SET active = FALSE WHERE bot_id = $1 AND ban_id = $2")
            .bind(bot_id)
            .bind(ban_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_user_state(
        &self,
        bot_id: i32,
        user_id: i64,
        state: &str,
        target_id: i64,
    ) -> Result<()> {
        sqlx::query("INSERT INTO user_states (bot_id, user_id, state, temp_target_id) VALUES ($1, $2, $3, $4) ON CONFLICT (bot_id, user_id) DO UPDATE SET state = EXCLUDED.state, temp_target_id = EXCLUDED.temp_target_id")
            .bind(bot_id).bind(user_id).bind(state).bind(target_id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_user_state(&self, bot_id: i32, user_id: i64) -> Result<Option<UserState>> {
        Ok(sqlx::query_as(
            "SELECT state, temp_target_id FROM user_states WHERE bot_id = $1 AND user_id = $2",
        )
        .bind(bot_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn clear_user_state(&self, bot_id: i32, user_id: i64) -> Result<()> {
        sqlx::query("UPDATE user_states SET state = 'none', temp_target_id = 0 WHERE bot_id = $1 AND user_id = $2")
            .bind(bot_id).bind(user_id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn ensure_client(&self, tg_user_id: i64) -> Result<Client> {
        let client: Client = sqlx::query_as(
            "INSERT INTO clients (tg_user_id) VALUES ($1)
             ON CONFLICT (tg_user_id) DO UPDATE SET banned = FALSE
             RETURNING *",
        )
        .bind(tg_user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(client)
    }

    pub async fn upgrade_to_pro(&self, tg_user_id: i64) -> Result<()> {
        sqlx::query("UPDATE clients SET plan = 'pro', pro_expires_at = NOW() + INTERVAL '30 days' WHERE tg_user_id = $1")
            .bind(tg_user_id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_stats(&self) -> Result<(i64, i64)> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT
                COUNT(*) FILTER (WHERE pro_expires_at > NOW()) as pro_count,
                COUNT(*) FILTER (WHERE pro_expires_at IS NULL OR pro_expires_at <= NOW()) as free_count
             FROM clients WHERE banned = FALSE"
        ).fetch_one(&self.pool).await?;
        Ok(row)
    }

    pub async fn get_client_plan_by_bot_id(&self, bot_id: i32) -> Result<String> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT CASE WHEN c.pro_expires_at > NOW() THEN 'pro' ELSE 'free' END as plan
             FROM clients c JOIN bots b ON c.id = b.client_id WHERE b.id = $1",
        )
        .bind(bot_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(p,)| p).unwrap_or_else(|| "free".to_string()))
    }

    pub async fn add_bot(&self, tg_user_id: i64, token: &str, username: &str) -> Result<BotConfig> {
        let existing: Option<BotConfig> = sqlx::query_as(
            "SELECT b.*, c.tg_user_id as client_tg_id FROM bots b JOIN clients c ON b.client_id = c.id WHERE b.token = $1"
        )
        .bind(token).fetch_optional(&self.pool).await?;

        if let Some(bot) = existing {
            if bot.client_tg_id != Some(tg_user_id) {
                anyhow::bail!("Этот токен уже привязан к другому аккаунту.");
            }

            sqlx::query("UPDATE bots SET active = TRUE, bot_username = $1, setup_complete = FALSE, channel_id = 0, channel_username = NULL, setup_code = NULL WHERE id = $2")
                .bind(username).bind(bot.id).execute(&self.pool).await?;

            let updated_bot: BotConfig = sqlx::query_as(
                "SELECT b.*, c.tg_user_id as client_tg_id FROM bots b JOIN clients c ON b.client_id = c.id WHERE b.id = $1"
            )
            .bind(bot.id).fetch_one(&self.pool).await?;

            Ok(updated_bot)
        } else {
            let bot: BotConfig = sqlx::query_as(
                "INSERT INTO bots (client_id, token, bot_username, channel_id)
                 VALUES ((SELECT id FROM clients WHERE tg_user_id = $1), $2, $3, 0)
                 RETURNING *, NULL as client_tg_id",
            )
            .bind(tg_user_id)
            .bind(token)
            .bind(username)
            .fetch_one(&self.pool)
            .await?;
            Ok(bot)
        }
    }

    pub async fn is_admin(&self, bot_id: i32, user_id: i64) -> Result<bool> {
        let row: Option<(i32,)> =
            sqlx::query_as("SELECT 1 FROM admins WHERE bot_id = $1 AND user_id = $2")
                .bind(bot_id)
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
    }

    pub async fn add_admin(
        &self,
        bot_id: i32,
        user_id: i64,
        user_name: &str,
        frozen: bool,
    ) -> Result<()> {
        sqlx::query("INSERT INTO admins (bot_id, user_id, user_name, frozen) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING")
            .bind(bot_id).bind(user_id).bind(user_name).bind(frozen).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn is_setup_complete(&self, bot_id: i32) -> Result<bool> {
        let row: Option<(bool,)> = sqlx::query_as("SELECT setup_complete FROM bots WHERE id = $1")
            .bind(bot_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(v,)| v).unwrap_or(false))
    }

    pub async fn set_language(&self, bot_id: i32, lang: Locale) -> Result<()> {
        sqlx::query("UPDATE bots SET lang = $1 WHERE id = $2")
            .bind(lang.as_str())
            .bind(bot_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_setup_code(&self, bot_id: i32, code: &str) -> Result<()> {
        sqlx::query("UPDATE bots SET setup_code = $1 WHERE id = $2")
            .bind(code)
            .bind(bot_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_setup_code(&self, bot_id: i32) -> Result<Option<String>> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT setup_code FROM bots WHERE id = $1")
                .bind(bot_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|(v,)| v))
    }

    pub async fn complete_setup(
        &self,
        bot_id: i32,
        channel_id: i64,
        channel_username: Option<&str>,
    ) -> Result<()> {
        sqlx::query("UPDATE bots SET channel_id = $1, channel_username = $2, setup_complete = TRUE, setup_code = NULL WHERE id = $3")
            .bind(channel_id).bind(channel_username).bind(bot_id).execute(&self.pool).await?;
        Ok(())
    }

    // ── Admin invite methods ──

    pub async fn create_admin_invite(&self, bot_id: i32) -> Result<String> {
        let code = uuid::Uuid::new_v4().simple().to_string()[..8].to_uppercase();
        sqlx::query("INSERT INTO admin_invites (bot_id, code) VALUES ($1, $2)")
            .bind(bot_id)
            .bind(&code)
            .execute(&self.pool)
            .await?;
        Ok(code)
    }

    pub async fn use_admin_invite(&self, bot_id: i32, code: &str) -> Result<bool> {
        let row: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM admin_invites \
             WHERE bot_id = $1 AND code = $2 AND used = FALSE AND expires_at > NOW()",
        )
        .bind(bot_id)
        .bind(code)
        .fetch_optional(&self.pool)
        .await?;

        if row.is_some() {
            sqlx::query("UPDATE admin_invites SET used = TRUE WHERE bot_id = $1 AND code = $2")
                .bind(bot_id)
                .bind(code)
                .execute(&self.pool)
                .await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn revoke_admin_invite(&self, bot_id: i32, code: &str) -> Result<()> {
        sqlx::query("DELETE FROM admin_invites WHERE bot_id = $1 AND code = $2")
            .bind(bot_id)
            .bind(code)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_admin_frozen(&self, bot_id: i32, user_id: i64, frozen: bool) -> Result<()> {
        sqlx::query("UPDATE admins SET frozen = $1 WHERE bot_id = $2 AND user_id = $3")
            .bind(frozen)
            .bind(bot_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_admin_status(&self, bot_id: i32, user_id: i64) -> Result<Option<bool>> {
        let row: Option<(bool,)> =
            sqlx::query_as("SELECT frozen FROM admins WHERE bot_id = $1 AND user_id = $2")
                .bind(bot_id)
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(f,)| f))
    }

    pub async fn count_active_admins(&self, bot_id: i32, owner_id: i64) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM admins WHERE bot_id = $1 AND frozen = FALSE AND user_id != $2",
        )
        .bind(bot_id)
        .bind(owner_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn check_plan_limits(&self, bot_id: i32, owner_id: i64) -> Result<()> {
        let plan = self.get_client_plan_by_bot_id(bot_id).await?;
        if plan == "free" {
            let active_count = self.count_active_admins(bot_id, owner_id).await?;
            if active_count > 1 {
                self.enforce_free_plan_limit(bot_id, owner_id).await?;
            }
        }
        Ok(())
    }

    pub async fn set_client_plan(&self, tg_user_id: i64, plan: &str) -> Result<()> {
        if plan == "pro" {
            sqlx::query("UPDATE clients SET plan = 'pro', pro_expires_at = NOW() + INTERVAL '30 days' WHERE tg_user_id = $1")
                .bind(tg_user_id).execute(&self.pool).await?;
        } else {
            sqlx::query(
                "UPDATE clients SET plan = 'free', pro_expires_at = NULL WHERE tg_user_id = $1",
            )
            .bind(tg_user_id)
            .execute(&self.pool)
            .await?;

            let bot_ids: Vec<(i32,)> = sqlx::query_as(
                "SELECT id FROM bots WHERE client_id = (SELECT id FROM clients WHERE tg_user_id = $1) AND active = TRUE"
            )
            .bind(tg_user_id)
            .fetch_all(&self.pool)
            .await?;

            for (bot_id,) in bot_ids {
                self.enforce_free_plan_limit(bot_id, tg_user_id).await?;
            }
        }
        Ok(())
    }

    pub async fn enforce_free_plan_limit(&self, bot_id: i32, owner_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE admins SET frozen = TRUE
             WHERE bot_id = $1
             AND user_id != $2
             AND id NOT IN (
                 SELECT id FROM admins
                 WHERE bot_id = $1
                 AND user_id != $2
                 ORDER BY id ASC
                 LIMIT 1
             )",
        )
        .bind(bot_id)
        .bind(owner_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Atomic publishing helpers ──

    /// Atomically claim a proposal for publishing.
    /// Returns true if this caller won the claim (rows_affected > 0).
    pub async fn claim_proposal_for_publishing(&self, bot_id: i32, group_id: &str) -> Result<bool> {
        let res = sqlx::query(
            "UPDATE messages SET status = 'publishing' \
             WHERE bot_id = $1 AND proposal_group_id = $2 AND status = 'pending'",
        )
        .bind(bot_id)
        .bind(group_id)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Finalize a successfully published proposal: set status='approved'
    /// and store the native channel message id.
    pub async fn finalize_published(
        &self,
        bot_id: i32,
        group_id: &str,
        channel_msg_id: i32,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE messages \
             SET status = 'approved', channel_message_id = $1 \
             WHERE bot_id = $2 AND proposal_group_id = $3",
        )
        .bind(channel_msg_id)
        .bind(bot_id)
        .bind(group_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Revert a failed publish back to pending so it can be retried.
    pub async fn revert_to_pending(&self, bot_id: i32, group_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE messages SET status = 'pending' \
             WHERE bot_id = $1 AND proposal_group_id = $2 AND status = 'publishing'",
        )
        .bind(bot_id)
        .bind(group_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
