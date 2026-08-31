SELECT contact_id, remote_identity_id, remote_key_id, remote_key_algorithm, remote_public_key,
remote_key_generation, capability_id, status, created_at_ms, updated_at_ms,
transport_endpoints_json, country_code
FROM contacts
ORDER BY created_at_ms, contact_id;
