ALTER TABLE messages ADD COLUMN sent_at_ms INTEGER;
ALTER TABLE messages ADD COLUMN delivered_at_ms INTEGER;
ALTER TABLE messages ADD COLUMN read_at_ms INTEGER;

UPDATE messages SET
  sent_at_ms = CASE WHEN status >= 2 THEN updated_at_ms END,
  delivered_at_ms = CASE WHEN status >= 3 THEN updated_at_ms END,
  read_at_ms = CASE WHEN status = 4 THEN updated_at_ms END;
