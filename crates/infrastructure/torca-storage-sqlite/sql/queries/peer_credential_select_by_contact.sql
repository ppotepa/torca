SELECT local_capability_id, secret_handle
FROM peer_credentials
WHERE contact_id = ?1;
