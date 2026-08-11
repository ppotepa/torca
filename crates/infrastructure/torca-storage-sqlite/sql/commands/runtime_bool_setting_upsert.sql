INSERT INTO runtime_settings(setting_key, bool_value, updated_at_ms)
VALUES (?1, ?2, ?3)
ON CONFLICT(setting_key) DO UPDATE SET
    bool_value = excluded.bool_value,
    updated_at_ms = excluded.updated_at_ms

