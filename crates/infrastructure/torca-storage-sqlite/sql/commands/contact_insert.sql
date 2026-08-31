INSERT INTO contacts (
    contact_id, remote_identity_id, remote_key_id, remote_key_algorithm,
remote_public_key, remote_key_generation, capability_id,
status, created_at_ms, updated_at_ms, transport_endpoints_json, country_code
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12);
