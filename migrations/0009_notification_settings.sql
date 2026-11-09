CREATE TABLE IF NOT EXISTS notification_settings (
    bot_id INT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL,
    proposals_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    reports_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (bot_id, user_id)
);
