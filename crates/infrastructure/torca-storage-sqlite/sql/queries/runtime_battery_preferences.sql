SELECT mode, background_sync, allow_delayed_background_delivery, metered_transfers, visual_activity
FROM runtime_battery_preferences
WHERE singleton = 1
