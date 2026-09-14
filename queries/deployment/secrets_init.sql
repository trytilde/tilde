UPDATE agent_deployment_settings SET encrypted_secrets=COALESCE(encrypted_secrets,$2) WHERE agent_id=$1;
