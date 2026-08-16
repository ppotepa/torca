UPDATE runtime_battery_preferences
SET mode = ?1,
    background_sync = ?2,
    allow_delayed_background_delivery = ?3,
    metered_transfers = ?4,
    visual_activity = ?5,
    updated_at_ms = ?6
WHERE singleton = 1
