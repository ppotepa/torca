SELECT c.contact_id,
       CASE WHEN v.remote_identity_id = c.remote_identity_id THEN 1 ELSE 0 END AS verified,
       CASE WHEN v.remote_identity_id = c.remote_identity_id THEN v.verified_at_ms ELSE NULL END AS verified_at_ms
FROM contacts c
LEFT JOIN contact_verification v ON v.contact_id = c.contact_id;
