CREATE TABLE IF NOT EXISTS runtime_battery_preferences (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    mode TEXT NOT NULL,
    background_sync TEXT NOT NULL,
    allow_delayed_background_delivery INTEGER NOT NULL CHECK (allow_delayed_background_delivery IN (0, 1)),
    metered_transfers TEXT NOT NULL,
    visual_activity TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

INSERT OR IGNORE INTO runtime_battery_preferences(
    singleton, mode, background_sync, allow_delayed_background_delivery,
    metered_transfers, visual_activity, updated_at_ms
)
VALUES (1, 'automatic', 'instant', 0, 'pause_large', 'follow_system', 0);
