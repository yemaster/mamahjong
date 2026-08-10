ALTER TABLE mamahjong_users
    ADD COLUMN IF NOT EXISTS selected_character_id TEXT
        REFERENCES mamahjong_characters(id) ON DELETE SET NULL;

ALTER TABLE mamahjong_users
    ADD COLUMN IF NOT EXISTS avatar_path TEXT;
