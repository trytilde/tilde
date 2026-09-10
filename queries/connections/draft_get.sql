SELECT field_key,encrypted_value FROM connection_setup_drafts WHERE setup_id=$1 ORDER BY field_key;
