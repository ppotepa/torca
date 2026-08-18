CREATE TABLE IF NOT EXISTS contact_connectivity_preferences (
    contact_id BLOB PRIMARY KEY REFERENCES contacts(contact_id) ON DELETE CASCADE,
    availability_mode TEXT NOT NULL CHECK (availability_mode IN ('adaptive', 'instant')),
    updated_at_ms INTEGER NOT NULL
);
