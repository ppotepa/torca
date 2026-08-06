UPDATE contacts SET
    remote_identity_id = ?2,
    remote_key_id = ?3,
    remote_key_algorithm = ?4,
    remote_public_key = ?5,
    remote_key_generation = ?6,
    onion_address = ?7,
    capability_id = ?8,
    status = ?9,
    created_at_ms = ?10,
    updated_at_ms = ?11
WHERE contact_id = ?1;
