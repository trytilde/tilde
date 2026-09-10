UPDATE connection_setups SET step=$3,action_id=$4,claimed_at=NULL,error_code=NULL,updated_at=now() WHERE id=$1 AND action_id=$2 AND claimed_at IS NOT NULL RETURNING id;
