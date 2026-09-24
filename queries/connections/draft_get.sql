--: Record()

--! run (p1) : Record
SELECT field_key,encrypted_value FROM connection_setup_drafts WHERE setup_id=:p1 ORDER BY field_key;
