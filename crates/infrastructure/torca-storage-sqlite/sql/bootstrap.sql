PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
PRAGMA trusted_schema = OFF;
-- Torca intentionally uses several narrow SQLCipher connections against one
-- local database. SQLite still serializes writers in WAL mode, so wait for a
-- short bounded interval instead of surfacing transient SQLITE_BUSY/LOCKED
-- as an application failure during delivery, receipts or attachment updates.
PRAGMA busy_timeout = 5000;
