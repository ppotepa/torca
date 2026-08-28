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
    remote_capability_id = ?14,
    remote_avatar_schema = ?15,
    remote_avatar_generator_version = ?16,
remote_avatar_catalog_version = ?17,
remote_avatar_hash = ?18,
remote_avatar_payload = ?19,
remote_transport_endpoints_json = ?20
WHERE session_id = ?1;
