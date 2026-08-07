SELECT a.attachment_id
FROM attachments AS a
JOIN messages AS m ON m.message_id = a.message_id
WHERE m.conversation_id = ?1
ORDER BY a.attachment_id;
