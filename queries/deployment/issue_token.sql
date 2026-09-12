UPDATE agent_deployments SET token_hash=$2,encrypted_secrets=COALESCE(encrypted_secrets,$3) WHERE agent_id=$1;
