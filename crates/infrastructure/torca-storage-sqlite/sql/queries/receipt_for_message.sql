SELECT receipt_id, kind, received_at_ms
FROM receipts
WHERE message_id = ?1
ORDER BY kind, received_at_ms, receipt_id;
