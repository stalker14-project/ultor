CREATE TABLE IF NOT EXISTS sponsorships (
    discord_id TEXT NOT NULL,
    role_id    TEXT NOT NULL,
    expires_at INTEGER,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (discord_id, role_id)
);
