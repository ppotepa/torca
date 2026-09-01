INSERT INTO conversation_read_state (conversation_id, read_through_ms)
VALUES (?1, ?2)
ON CONFLICT(conversation_id) DO UPDATE SET
    read_through_ms = MAX(conversation_read_state.read_through_ms, excluded.read_through_ms);
