--! run (p1, p2)
INSERT INTO connection_setup_values(setup_id,field_key,encrypted_value)
SELECT :p1,field_key,encrypted_value FROM connection_values WHERE connection_id=:p2;
