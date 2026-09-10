INSERT INTO connection_setup_values(setup_id,field_key,encrypted_value)
SELECT $1,field_key,encrypted_value FROM connection_values WHERE connection_id=$2;
