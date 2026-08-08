SELECT CASE
         WHEN v.contact_id IS NULL THEN 0
         WHEN v.remote_identity_id = c.remote_identity_id THEN 1
         ELSE 2
       END AS security_state
FROM conversations conversation
JOIN contacts c ON c.contact_id = conversation.contact_id
LEFT JOIN contact_verification v ON v.contact_id = c.contact_id
WHERE conversation.conversation_id = ?1;
