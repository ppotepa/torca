CREATE TABLE IF NOT EXISTS schema_metadata (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    version INTEGER NOT NULL
);
INSERT OR IGNORE INTO schema_metadata(singleton, version) VALUES (1, 0);

CREATE TABLE IF NOT EXISTS processed_commands (
    command_id BLOB PRIMARY KEY NOT NULL,
    completed_at_ms INTEGER NOT NULL,
    result_kind TEXT NOT NULL,
    result_payload BLOB
);
