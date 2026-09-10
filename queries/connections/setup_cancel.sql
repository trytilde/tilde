UPDATE connection_setups SET step='cancelled',claimed_at=NULL,updated_at=now() WHERE connection_id=$1 AND step NOT IN ('complete','failed','cancelled');
