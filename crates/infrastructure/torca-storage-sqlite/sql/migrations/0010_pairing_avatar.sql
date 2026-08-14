ALTER TABLE pairing_sessions ADD COLUMN remote_avatar_schema INTEGER;
ALTER TABLE pairing_sessions ADD COLUMN remote_avatar_generator_version TEXT;
ALTER TABLE pairing_sessions ADD COLUMN remote_avatar_catalog_version TEXT;
ALTER TABLE pairing_sessions ADD COLUMN remote_avatar_hash BLOB;
ALTER TABLE pairing_sessions ADD COLUMN remote_avatar_payload BLOB;
