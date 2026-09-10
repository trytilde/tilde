INSERT INTO connection_setup_values(setup_id,field_key,encrypted_value) VALUES($1,$2,$3) ON CONFLICT(setup_id,field_key) DO UPDATE SET encrypted_value=EXCLUDED.encrypted_value;
