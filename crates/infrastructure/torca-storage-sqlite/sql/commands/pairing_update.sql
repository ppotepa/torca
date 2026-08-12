UPDATE pairing_sessions
SET
    code = ?2,
    role = ?3,
    state = ?4,
    expires_at_ms = ?5,
    local_approved = ?6,
    remote_approved = ?7,
    remote_identity_id = ?8,
    remote_key_id = ?9,
    remote_key_algorithm = ?10,
    remote_public_key = ?11,
    remote_key_generation = ?12,
    remote_display_name = ?13,
    remote_onion_address = ?14,
    remote_capability_id = ?15
WHERE session_id = ?1;
