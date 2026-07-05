CREATE TABLE IF NOT EXISTS clients (
    id SERIAL PRIMARY KEY,
    tg_user_id BIGINT NOT NULL UNIQUE,
    plan TEXT NOT NULL DEFAULT 'free',
    pro_expires_at TIMESTAMPTZ,
    banned BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS bots (
    id SERIAL PRIMARY KEY,
    client_id INT NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    token TEXT NOT NULL UNIQUE,
    bot_username TEXT NOT NULL,
    channel_id BIGINT NOT NULL,
    lang TEXT NOT NULL DEFAULT 'en',
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS messages (
    id BIGSERIAL PRIMARY KEY,
    bot_id INT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    chat_id BIGINT NOT NULL,
    telegram_message_id INT NOT NULL,
    sender_id BIGINT NOT NULL,
    message_text TEXT NOT NULL DEFAULT '',
    media_type TEXT NOT NULL DEFAULT 'text',
    media_file_id TEXT NOT NULL DEFAULT '',
    media_group_id TEXT,
    proposal_group_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    parent_message_id BIGINT,
    channel_message_id INT,
    UNIQUE(bot_id, chat_id, telegram_message_id)
);

CREATE INDEX IF NOT EXISTS idx_messages_status ON messages(bot_id, status);
CREATE INDEX IF NOT EXISTS idx_messages_group ON messages(bot_id, proposal_group_id);

CREATE TABLE IF NOT EXISTS admins (
    id SERIAL PRIMARY KEY,
    bot_id INT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL,
    user_name TEXT NOT NULL DEFAULT '',
    UNIQUE(bot_id, user_id)
);

CREATE TABLE IF NOT EXISTS banned_users (
    bot_id INT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL,
    PRIMARY KEY (bot_id, user_id)
);

CREATE TABLE IF NOT EXISTS ban_records (
    id SERIAL PRIMARY KEY,
    bot_id INT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    ban_id TEXT NOT NULL UNIQUE,
    user_id BIGINT NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL,
    active BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE IF NOT EXISTS user_states (
    bot_id INT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL,
    state TEXT NOT NULL DEFAULT 'none',
    temp_target_id BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (bot_id, user_id)
);
