UPDATE chat_identity_verifications SET status=$2 WHERE id=$1 AND status='pending';
