UPDATE chat_identity_verifications SET status='approved' WHERE id=$1 AND token_hash=$2 AND status='delivered' AND expires_at>NOW() RETURNING connection_id,agent_id,identity_id;
