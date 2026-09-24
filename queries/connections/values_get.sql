--: Record()

--! run (p1) : Record
SELECT field_key,encrypted_value FROM connection_values WHERE connection_id=:p1;
