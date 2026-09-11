UPDATE chat_identity_verifications SET expires_at=NOW() WHERE identity_id=$1 AND agent_id=$2 AND status<>'approved';
