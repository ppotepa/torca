INSERT INTO contacts (
    contact_id, remote_identity_id, remote_key_id, remote_key_algorithm,
    remote_public_key, remote_key_generation, onion_address, capability_id,
    status, created_at_ms, updated_at_ms
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11);
