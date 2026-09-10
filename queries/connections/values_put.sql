INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3) ON CONFLICT(connection_id,field_key) DO UPDATE SET encrypted_value=EXCLUDED.encrypted_value;
