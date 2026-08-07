SELECT c.contact_id, m.display_name
FROM contacts AS c
LEFT JOIN contact_metadata AS m ON m.contact_id = c.contact_id
WHERE c.status != 2
ORDER BY c.created_at_ms, c.contact_id;
