CREATE TABLE IF NOT EXISTS mamahjong_tablecloths (
    id TEXT PRIMARY KEY,
    version BIGINT NOT NULL CHECK (version >= 1),
    name TEXT NOT NULL,
    texture_path TEXT NOT NULL,
    enabled BOOLEAN NOT NULL,
    is_default BOOLEAN NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS mamahjong_tablecloths_one_default_idx
    ON mamahjong_tablecloths (is_default)
    WHERE is_default;

ALTER TABLE mamahjong_users
    ADD COLUMN IF NOT EXISTS selected_tablecloth_id TEXT
        REFERENCES mamahjong_tablecloths(id) ON DELETE SET NULL;
