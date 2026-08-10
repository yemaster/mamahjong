CREATE TABLE IF NOT EXISTS mamahjong_users (
    id TEXT PRIMARY KEY,
    version BIGINT NOT NULL CHECK (version >= 1),
    login_name TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'suspended')),
    role TEXT NOT NULL CHECK (role IN ('player', 'administrator')),
    nickname TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS mamahjong_sessions (
    token TEXT PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    user_id TEXT NOT NULL REFERENCES mamahjong_users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS mamahjong_sessions_user_id_idx
    ON mamahjong_sessions (user_id);
