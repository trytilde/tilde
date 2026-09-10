UPDATE connection_setups SET provider_redirect_url=$3 WHERE id=$1 AND action_id=$2 AND claimed_at IS NOT NULL AND step NOT IN ('complete','failed','cancelled');
