CREATE TABLE IF NOT EXISTS banned_usernames (
    id SERIAL PRIMARY KEY,
    bot_id INTEGER NOT NULL,
    username VARCHAR(255) NOT NULL,
    UNIQUE(bot_id, username)
);
