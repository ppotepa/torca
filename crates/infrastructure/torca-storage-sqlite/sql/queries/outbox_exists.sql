SELECT EXISTS(SELECT 1 FROM outbox WHERE message_id = ?1);
