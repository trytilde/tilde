--! run (p1, p2, p3)
INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES(:p1,:p2,:p3) ON CONFLICT(connection_id,field_key) DO UPDATE SET encrypted_value=EXCLUDED.encrypted_value;
