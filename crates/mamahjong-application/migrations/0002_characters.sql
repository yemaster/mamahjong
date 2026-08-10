CREATE TABLE IF NOT EXISTS mamahjong_characters (
    id TEXT PRIMARY KEY,
    version BIGINT NOT NULL CHECK (version >= 1),
    name TEXT NOT NULL,
    illustration_path TEXT NOT NULL,
    emotes_json TEXT NOT NULL,
    voices_json TEXT NOT NULL,
    outfits_json TEXT NOT NULL,
    enabled BOOLEAN NOT NULL,
    is_default BOOLEAN NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS mamahjong_characters_one_default_idx
    ON mamahjong_characters (is_default)
    WHERE is_default;
