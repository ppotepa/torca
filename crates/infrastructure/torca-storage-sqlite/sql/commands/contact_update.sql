UPDATE contacts SET
    remote_identity_id = ?2,
    remote_key_id = ?3,
    remote_key_algorithm = ?4,
    remote_public_key = ?5,
    remote_key_generation = ?6,
    capability_id = ?7,
    status = ?8,
    created_at_ms = ?9,
updated_at_ms = ?10,
transport_endpoints_json = ?11,
country_code = ?12
WHERE contact_id = ?1;
