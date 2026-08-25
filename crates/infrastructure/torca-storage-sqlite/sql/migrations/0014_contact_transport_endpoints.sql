ALTER TABLE contacts
    ADD COLUMN transport_endpoints_json TEXT NOT NULL DEFAULT '{}';
