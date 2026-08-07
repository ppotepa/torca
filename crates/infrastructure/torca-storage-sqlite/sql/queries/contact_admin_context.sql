SELECT c.status, v.conversation_id, p.secret_handle
FROM contacts AS c
LEFT JOIN conversations AS v ON v.contact_id = c.contact_id
LEFT JOIN peer_credentials AS p ON p.contact_id = c.contact_id
WHERE c.contact_id = ?1;
