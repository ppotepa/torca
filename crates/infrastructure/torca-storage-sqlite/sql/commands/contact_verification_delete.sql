DELETE FROM contact_verification
WHERE contact_id = ?1
  AND remote_identity_id = (
    SELECT remote_identity_id
    FROM contacts
    WHERE contact_id = ?1
  );
