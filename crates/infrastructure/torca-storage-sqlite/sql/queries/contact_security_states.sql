SELECT c.contact_id,
       CASE
         WHEN v.contact_id IS NULL THEN 0
         WHEN v.remote_identity_id = c.remote_identity_id THEN 1
         ELSE 2
       END AS security_state,
       CASE
         WHEN v.remote_identity_id = c.remote_identity_id THEN v.verified_at_ms
         ELSE NULL
       END AS verified_at_ms
FROM contacts c
LEFT JOIN contact_verification v ON v.contact_id = c.contact_id;
