--: Record()

--! run (p1) : Record
SELECT field_key,encrypted_value FROM connection_setup_values WHERE setup_id=:p1;
