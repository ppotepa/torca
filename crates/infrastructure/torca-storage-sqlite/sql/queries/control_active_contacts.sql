SELECT DISTINCT contact_id
FROM control_outbox
WHERE state IN (0, 1)
ORDER BY contact_id;
