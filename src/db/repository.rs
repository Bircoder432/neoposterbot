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

    pub fn new_ban_id() -> String {
        format!(
            "BAN-{}",
            &uuid::Uuid::new_v4().simple().to_string().to_uppercase()[..6]
        )
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
             FROM clients WHERE banned = FALSE",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
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

    pub async fn find_active_bot_by_username(&self, username: &str) -> Result<Option<BotConfig>> {
        let username = username.trim_start_matches('@');
        Ok(sqlx::query_as(
            "SELECT b.*, c.tg_user_id as client_tg_id FROM bots b \
             JOIN clients c ON b.client_id = c.id \
             WHERE LOWER(b.bot_username) = LOWER($1) AND b.active = TRUE",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn deactivate_bot(&self, bot_id: i32) -> Result<()> {
        sqlx::query("UPDATE bots SET active = FALSE WHERE id = $1")
            .bind(bot_id)
            .execute(&self.pool)
            .await?;
        Ok(())
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

    pub async fn get_language(&self, bot_id: i32) -> Result<Locale> {
        let row: Option<(String,)> = sqlx::query_as("SELECT lang FROM bots WHERE id = $1")
            .bind(bot_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.and_then(|(v,)| Locale::parse(&v)).unwrap_or(Locale::En))
    }

    pub async fn set_language(&self, bot_id: i32, lang: Locale) -> Result<()> {
        sqlx::query("UPDATE bots SET lang = $1 WHERE id = $2")
            .bind(lang.as_str())
            .bind(bot_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn is_setup_complete(&self, bot_id: i32) -> Result<bool> {
        let row: Option<(bool,)> = sqlx::query_as("SELECT setup_complete FROM bots WHERE id = $1")
            .bind(bot_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(v,)| v).unwrap_or(false))
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

    pub async fn ban_user(
        &self,
        bot_id: i32,
        user_hash: &str,
        reason: &str,
        ban_id: &str,
    ) -> Result<()> {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO bans (bot_id, ban_id, user_hash, reason, created_at, active)
             VALUES ($1, $2, $3, $4, $5, TRUE)
             ON CONFLICT (bot_id, user_hash) DO UPDATE
             SET ban_id = EXCLUDED.ban_id,
                 reason = EXCLUDED.reason,
                 created_at = EXCLUDED.created_at,
                 active = TRUE",
        )
        .bind(bot_id)
        .bind(ban_id)
        .bind(user_hash)
        .bind(reason)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn is_banned(&self, bot_id: i32, user_hash: &str) -> Result<bool> {
        let row: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM bans WHERE bot_id = $1 AND user_hash = $2 AND active = TRUE",
        )
        .bind(bot_id)
        .bind(user_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    pub async fn pardon_by_ban_id(&self, bot_id: i32, ban_id: &str) -> Result<Option<BanRecord>> {
        Ok(sqlx::query_as(
            "UPDATE bans SET active = FALSE
             WHERE bot_id = $1 AND ban_id = $2 AND active = TRUE
             RETURNING ban_id, user_hash, reason, created_at",
        )
        .bind(bot_id)
        .bind(ban_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn get_active_ban_records(&self, bot_id: i32) -> Result<Vec<BanRecord>> {
        Ok(sqlx::query_as(
            "SELECT ban_id, user_hash, reason, created_at FROM bans
             WHERE bot_id = $1 AND active = TRUE ORDER BY created_at DESC",
        )
        .bind(bot_id)
        .fetch_all(&self.pool)
        .await?)
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

    pub async fn create_report(
        &self,
        bot_id: i32,
        channel_msg_id: i32,
        reporter_hash: &str,
        reason: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO reports (bot_id, channel_msg_id, reporter_hash, reason)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(bot_id)
        .bind(channel_msg_id)
        .bind(reporter_hash)
        .bind(reason)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_pending_reports(&self, bot_id: i32) -> Result<Vec<Report>> {
        Ok(sqlx::query_as(
            "SELECT * FROM reports WHERE bot_id = $1 AND status = 'pending' ORDER BY created_at ASC",
        )
        .bind(bot_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn get_report_by_id(&self, bot_id: i32, report_id: i32) -> Result<Option<Report>> {
        Ok(sqlx::query_as(
            "SELECT * FROM reports WHERE id = $1 AND bot_id = $2 AND status = 'pending'",
        )
        .bind(report_id)
        .bind(bot_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn dismiss_report(&self, bot_id: i32, report_id: i32) -> Result<()> {
        sqlx::query("UPDATE reports SET status = 'dismissed' WHERE id = $1 AND bot_id = $2")
            .bind(report_id)
            .bind(bot_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn resolve_report(&self, bot_id: i32, report_id: i32) -> Result<()> {
        sqlx::query("UPDATE reports SET status = 'resolved' WHERE id = $1 AND bot_id = $2")
            .bind(report_id)
            .bind(bot_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn count_pending_reports(&self, bot_id: i32) -> Result<i64> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM reports WHERE bot_id = $1 AND status = 'pending'")
                .bind(bot_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }

    pub async fn log_action(
        &self,
        bot_id: i32,
        admin_id: i64,
        admin_name: &str,
        action: &str,
        target: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO audit_logs (bot_id, admin_id, admin_name, action, target)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(bot_id)
        .bind(admin_id)
        .bind(admin_name)
        .bind(action)
        .bind(target)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_audit_logs(&self, bot_id: i32, limit: i64) -> Result<Vec<AuditLog>> {
        Ok(sqlx::query_as(
            "SELECT * FROM audit_logs WHERE bot_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(bot_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }
}
