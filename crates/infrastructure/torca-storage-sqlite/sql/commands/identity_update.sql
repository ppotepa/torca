UPDATE local_identity
SET key_id = ?1,
    key_algorithm = ?2,
    public_key = ?3,
    key_generation = ?4,
    display_name = ?5,
    avatar_reference = ?6,
    country_code = ?7,
    updated_at_ms = ?8
WHERE singleton = 1 AND key_generation = ?9;
