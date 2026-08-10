CREATE TABLE IF NOT EXISTS mamahjong_music_tracks (
    id TEXT PRIMARY KEY,
    version BIGINT NOT NULL CHECK (version >= 1),
    name TEXT NOT NULL,
    scene TEXT NOT NULL,
    audio_path TEXT NOT NULL,
    duration_ms BIGINT NOT NULL CHECK (duration_ms >= 0),
    enabled BOOLEAN NOT NULL,
    is_default BOOLEAN NOT NULL
);

-- 大厅和对局各有一首默认曲目，互不相干。
CREATE UNIQUE INDEX IF NOT EXISTS mamahjong_music_tracks_one_default_idx
    ON mamahjong_music_tracks (scene)
    WHERE is_default;

ALTER TABLE mamahjong_users
    ADD COLUMN IF NOT EXISTS selected_lobby_music_id TEXT
        REFERENCES mamahjong_music_tracks(id) ON DELETE SET NULL;

ALTER TABLE mamahjong_users
    ADD COLUMN IF NOT EXISTS selected_match_music_id TEXT
        REFERENCES mamahjong_music_tracks(id) ON DELETE SET NULL;
