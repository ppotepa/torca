CREATE TABLE IF NOT EXISTS local_identity (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    identity_id BLOB NOT NULL,
    key_id BLOB NOT NULL,
    key_algorithm INTEGER NOT NULL,
    public_key BLOB NOT NULL,
    key_generation INTEGER NOT NULL,
    display_name TEXT NOT NULL,
    avatar_reference TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
