ALTER TABLE mamahjong_users
    ADD COLUMN IF NOT EXISTS selected_riichi_music_id TEXT
        REFERENCES mamahjong_music_tracks(id) ON DELETE SET NULL;
