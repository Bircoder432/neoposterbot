use anyhow::{Context, Result};
use std::env;
use url::Url;

#[derive(Clone, Debug)]
pub struct Config {
    pub master_bot_token: String,
    pub owner_id: i64,
    pub database_url: String,
    pub watermark_username: String,
    pub instruction_url: Url,
    pub deploy_server: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let deploy_server = env::var("DEPLOY_SERVER").unwrap_or_else(|_| "production".to_string());

        // Читаем ссылку на инструкцию из .env, по умолчанию заглушка
        let instruction_url_str =
            env::var("INSTRUCTION_URL").unwrap_or_else(|_| "https://telegra.ph/".to_string());
        let instruction_url =
            Url::parse(&instruction_url_str).context("Invalid INSTRUCTION_URL")?;

        Ok(Self {
            master_bot_token: env::var("MASTER_BOT_TOKEN").context("MASTER_BOT_TOKEN not set")?,
            owner_id: env::var("OWNER_ID")
                .context("OWNER_ID not set")?
                .parse()
                .context("OWNER_ID must be a number")?,
            database_url: env::var("DATABASE_URL").context("DATABASE_URL not set")?,
            watermark_username: env::var("WATERMARK_USERNAME")
                .unwrap_or_else(|_| "@superduperbot".into()),
            deploy_server,
            instruction_url,
        })
    }
}
