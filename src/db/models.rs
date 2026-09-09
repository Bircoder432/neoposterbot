use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct Client {
    pub id: i32,
    pub tg_user_id: i64,
    pub plan: String,
    pub pro_expires_at: Option<DateTime<Utc>>,
    pub banned: bool,
    pub lang: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct BotConfig {
    pub id: i32,
    pub client_id: i32,
    pub token: String,
    pub bot_username: String,
    pub channel_id: i64,
    pub lang: String,
    pub active: bool,
    pub setup_complete: bool,
    pub setup_code: Option<String>,
    pub channel_username: Option<String>,
    pub client_tg_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ProposalMessage {
    pub id: u64,
    pub sender_id: i64,
    pub message_text: String,
    pub media_type: String,
    pub media_file_id: String,
    pub media_group_id: Option<String>,
    pub proposal_group_id: String,
    pub parent_message_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct IncomingProposal {
    pub chat_id: i64,
    pub telegram_message_id: i32,
    pub sender_id: i64,
    pub message_text: String,
    pub media_type: String,
    pub media_file_id: String,
    pub media_group_id: Option<String>,
    pub proposal_group_id: String,
    pub parent_message_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct Proposal {
    pub group_id: String,
    pub messages: Vec<ProposalMessage>,
}

impl Proposal {
    pub fn first(&self) -> &ProposalMessage {
        &self.messages[0]
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct Admin {
    pub id: i32,
    pub bot_id: i32,
    pub user_id: i64,
    pub user_name: String,
    pub frozen: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct BanRecord {
    pub ban_id: String,
    pub user_hash: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Report {
    pub id: i32,
    pub bot_id: i32,
    pub channel_msg_id: i32,
    pub reporter_hash: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub status: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct AuditLog {
    pub id: i32,
    pub bot_id: i32,
    pub admin_id: i64,
    pub admin_name: String,
    pub action: String,
    pub target: String,
    pub created_at: DateTime<Utc>,
}
