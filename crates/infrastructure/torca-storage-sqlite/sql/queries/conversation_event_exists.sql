SELECT EXISTS(SELECT 1 FROM conversation_events WHERE event_id = ?1);
