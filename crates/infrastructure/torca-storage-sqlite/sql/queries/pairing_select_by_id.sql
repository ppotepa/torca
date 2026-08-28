SELECT
    session_id,
    code,
    role,
    state,
    expires_at_ms,
    local_approved,
    remote_approved,
    remote_identity_id,
    remote_key_id,
    remote_key_algorithm,
    remote_public_key,
    remote_key_generation,
    remote_display_name,
    remote_capability_id,
    remote_avatar_schema,
    remote_avatar_generator_version,
    remote_avatar_catalog_version,
remote_avatar_hash,
remote_avatar_payload,
remote_transport_endpoints_json
FROM pairing_sessions
WHERE session_id = ?1;
