UPDATE connection_setups SET step=$2 WHERE id=$1 AND step='fields' AND claimed_at IS NULL;
