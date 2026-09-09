CREATE TABLE IF NOT EXISTS reports (
    id SERIAL PRIMARY KEY,
    bot_id INT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    channel_msg_id INT NOT NULL,
    reporter_hash TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status TEXT NOT NULL DEFAULT 'pending'
);

CREATE INDEX IF NOT EXISTS idx_reports_bot_status ON reports(bot_id, status);
