-- Stage 2: promo codes (free PRO / discount on PRO purchase)
CREATE TABLE IF NOT EXISTS promo_codes (
    id SERIAL PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL,                -- 'free_pro' | 'discount'
    param INT NOT NULL,                -- days for free_pro, percent for discount
    max_uses INT NOT NULL DEFAULT 0,   -- 0 = unlimited
    used_count INT NOT NULL DEFAULT 0,
    created_by BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    active BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE IF NOT EXISTS promo_redemptions (
    id SERIAL PRIMARY KEY,
    promo_id INT NOT NULL REFERENCES promo_codes(id) ON DELETE CASCADE,
    client_tg_id BIGINT NOT NULL,
    used_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (promo_id, client_tg_id)
);

-- Pending discount received via promo code, applied automatically at PRO purchase
CREATE TABLE IF NOT EXISTS client_discounts (
    client_tg_id BIGINT PRIMARY KEY,
    discount_percent INT NOT NULL,
    promo_id INT REFERENCES promo_codes(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Owner-configurable service settings (pro_days, pro_price_stars)
CREATE TABLE IF NOT EXISTS service_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO service_settings (key, value)
VALUES ('pro_days', '30'), ('pro_price_stars', '100')
ON CONFLICT (key) DO NOTHING;
