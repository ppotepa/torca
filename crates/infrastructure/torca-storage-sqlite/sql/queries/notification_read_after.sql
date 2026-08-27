SELECT cursor,payload
FROM notification_outbox
WHERE cursor > ?1
ORDER BY cursor
LIMIT ?2
