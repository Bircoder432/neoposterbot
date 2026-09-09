CREATE TABLE IF NOT EXISTS audit_logs (
    id SERIAL PRIMARY KEY,
    bot_id INT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    admin_id BIGINT NOT NULL,
    admin_name TEXT NOT NULL DEFAULT '',
    action TEXT NOT NULL,
    target TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_bot ON audit_logs(bot_id, created_at DESC);
