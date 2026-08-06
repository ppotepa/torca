SELECT identity_id, key_id, key_algorithm, public_key, key_generation,
       display_name, avatar_reference, created_at_ms, updated_at_ms
FROM local_identity
WHERE singleton = 1;
