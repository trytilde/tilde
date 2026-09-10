UPDATE connection_setups SET claimed_at=now(),updated_at=now() WHERE id=$1 AND action_id=$2 AND step=$3 AND claimed_at IS NULL AND expires_at>now() RETURNING id;
