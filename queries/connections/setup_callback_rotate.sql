UPDATE connection_setups SET callback_token=$3,callback_hash=$4 WHERE id=$1 AND action_id=$2 AND claimed_at IS NOT NULL AND step NOT IN ('complete','failed','cancelled') RETURNING id;
