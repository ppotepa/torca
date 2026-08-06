INSERT INTO local_identity(
    singleton, identity_id, key_id, key_algorithm, public_key, key_generation,
    display_name, avatar_reference, created_at_ms, updated_at_ms
) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);
