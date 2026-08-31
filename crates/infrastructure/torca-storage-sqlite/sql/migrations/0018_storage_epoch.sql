CREATE TABLE IF NOT EXISTS torca_storage_metadata (
    singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
    storage_epoch INTEGER NOT NULL CHECK (storage_epoch > 0)
);

INSERT INTO torca_storage_metadata (singleton, storage_epoch)
VALUES (1, 3)
ON CONFLICT(singleton) DO NOTHING;
